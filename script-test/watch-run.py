#!/usr/bin/env python3
"""Watch a workflow run and print one line per thing worth acting on.

The run is driven from the shell, so the shell needs the events: a status flip
(paused, blocked, stopped, failed), a child agent finishing, a new journal entry,
or a stall where nothing moved for minutes. Each stdout line becomes one chat
notification, so the output stays selective on purpose.

    python3 watch-run.py <run-dir> [--interval 15] [--stall-minutes 10]

Exit is not required: the watcher keeps reporting across a pause and a resume.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

SESSIONS = Path.home() / ".grok" / "sessions"
TERMINAL = {"completed", "failed", "cancelled", "interrupted", "stopped"}
ALERT = {"paused", "blocked", "stopped", "failed", "cancelled", "interrupted"}


def _stamp() -> str:
    return time.strftime("%H:%M:%S")


def _read_state(path: Path) -> dict:
    """The run state, which the host wraps in a top-level `state` object."""
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    if not isinstance(payload, dict):
        return {}
    state = payload.get("state")
    return state if isinstance(state, dict) else payload


def _agent_files(agent_id: str) -> list[Path]:
    """The session files that grow while a child agent works."""
    files: list[Path] = []
    try:
        for session_dir in SESSIONS.glob(f"*/{agent_id}"):
            for name in ("chat_history.jsonl", "events.jsonl"):
                candidate = session_dir / name
                if candidate.is_file():
                    files.append(candidate)
    except OSError:
        pass
    return files


def _newest_mtime(paths: list[Path]) -> float:
    newest = 0.0
    for path in paths:
        try:
            newest = max(newest, path.stat().st_mtime)
        except OSError:
            continue
    return newest


class Watcher:
    def __init__(self, run_dir: Path, stall_minutes: float, agent_quiet_minutes: float) -> None:
        self.run_dir = run_dir
        self.state_path = run_dir / "state.json"
        self.journal_path = run_dir / "journal.jsonl"
        self.stall_seconds = stall_minutes * 60.0
        self.quiet_seconds = agent_quiet_minutes * 60.0
        self.status = ""
        self.agent_states: dict[str, str] = {}
        self.quiet_reported: dict[str, bool] = {}
        self.quiet_since: dict[str, float] = {}
        self.journal_offset = 0
        self.stall_reported = False

    # ── emitters ──────────────────────────────────────────────────────────
    def status_event(self, state: dict) -> None:
        status = str(state.get("status", "?"))
        if status == self.status:
            return
        previous, self.status = self.status, status
        history = state.get("history") or []
        detail = str(history[-1].get("detail", ""))[:220] if history else ""
        marker = "ALERT" if status in ALERT else "STATUS"
        print(
            f"{_stamp()} {marker} {previous or '?'} -> {status} "
            f"| phase={state.get('current_phase')} "
            f"| agents={state.get('agents_used')}/{state.get('agent_budget')} "
            f"| last={detail}"
        )

    def agent_events(self, state: dict) -> None:
        for agent in state.get("agents") or []:
            label = str(agent.get("label", "?"))
            current = str(agent.get("state", "?"))
            if self.agent_states.get(label) == current:
                continue
            previous = self.agent_states.get(label)
            self.agent_states[label] = current
            if previous is None and current == "running":
                print(f"{_stamp()} AGENT started {label} ({agent.get('agent_type') or 'inherit'})")
                continue
            if current == "running":
                continue
            print(
                f"{_stamp()} AGENT {label} -> {current} "
                f"| tokens={agent.get('tokens_used')} "
                f"| {round((agent.get('duration_ms') or 0) / 1000)}s"
            )

    def journal_events(self) -> None:
        try:
            size = self.journal_path.stat().st_size
        except OSError:
            return
        if size < self.journal_offset:  # a fresh run in the same dir
            self.journal_offset = 0
        if size == self.journal_offset:
            return
        with self.journal_path.open("r", encoding="utf-8") as handle:
            handle.seek(self.journal_offset)
            for line in handle:
                if not line.endswith("\n"):
                    break  # a half-written line waits for the next poll
                self.journal_offset += len(line.encode("utf-8"))
                try:
                    entry = json.loads(line)
                except ValueError:
                    continue
                self.journal_entry(entry)

    def journal_entry(self, entry: dict) -> None:
        result = entry.get("result")
        result = result if isinstance(result, dict) else {}
        output = result.get("output")
        if isinstance(output, str):  # a failed or cancelled job reports text
            output = {"status": "failed", "summary": output}
        output = output if isinstance(output, dict) else {}
        status = output.get("status", "")
        flags = []
        if output.get("needs_human"):
            flags.append("needs_human")
        if status == "blocked":
            flags.append(f"blocked_on={str(output.get('blocked_on', ''))[:120]}")
        if result.get("cancelled"):
            flags.append("cancelled")
        if not result.get("success", True):
            flags.append("success=false")
        marker = (
            "BLOCKED"
            if flags or status in ("blocked", "failed", "cancelled")
            else "JOURNAL"
        )
        summary = str(output.get("summary", ""))[:200].replace("\n", " ")
        print(
            f"{_stamp()} {marker} seq={entry.get('seq')} {entry.get('kind')} "
            f"status={status or '-'} {' '.join(flags)} | {summary}"
        )

    def stall_event(self, state: dict) -> None:
        running = [
            str(agent.get("agent_id"))
            for agent in state.get("agents") or []
            if agent.get("state") == "running"
        ]
        if not running:
            return
        paths = [self.state_path, self.journal_path]
        for agent_id in running:
            paths.extend(_agent_files(agent_id))
        idle_for = time.time() - _newest_mtime(paths)
        if idle_for >= self.stall_seconds:
            if not self.stall_reported:
                self.stall_reported = True
                print(
                    f"{_stamp()} STALL nothing written for {round(idle_for / 60)}m "
                    f"| running={len(running)} | phase={state.get('current_phase')}"
                )
        else:
            self.stall_reported = False

    def quiet_event(self, state: dict) -> None:
        """Per-agent silence: one stalled agent hides behind active peers.

        A long build looks the same as a hang, so the threshold is higher than
        the global one and the line says what to check before acting.
        """
        for agent in state.get("agents") or []:
            if agent.get("state") != "running":
                continue
            label = str(agent.get("label", "?"))
            paths = _agent_files(str(agent.get("agent_id")))
            if not paths:
                continue
            idle_for = time.time() - _newest_mtime(paths)
            if idle_for < self.quiet_seconds:
                if self.quiet_reported.pop(label, False):
                    quiet_for = round((time.time() - self.quiet_since.pop(label, time.time())) / 60)
                    print(
                        f"{_stamp()} AGENT_RESUME {label} wrote again after {quiet_for}m quiet "
                        f"| phase={state.get('current_phase')}"
                    )
            elif not self.quiet_reported.get(label):
                self.quiet_reported[label] = True
                self.quiet_since[label] = time.time()
                print(
                    f"{_stamp()} AGENT_QUIET {label} no writes for {round(idle_for / 60)}m "
                    f"| phase={state.get('current_phase')} | a long build looks the same, "
                    f"check cargo/rustc processes before acting"
                )

    # ── loop ──────────────────────────────────────────────────────────────
    def poll(self) -> None:
        state = _read_state(self.state_path)
        if state:
            self.status_event(state)
            self.agent_events(state)
            self.stall_event(state)
            self.quiet_event(state)
        self.journal_events()

    def run(self, interval: float) -> None:
        print(
            f"{_stamp()} WATCH {self.run_dir.name} interval={interval}s "
            f"stall={self.stall_seconds / 60}m quiet={self.quiet_seconds / 60}m"
        )
        while True:
            try:
                self.poll()
            except Exception as error:  # noqa: BLE001 - the watcher reports and keeps going
                print(f"{_stamp()} WATCH error {type(error).__name__}: {error}")
            time.sleep(interval)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run_dir")
    parser.add_argument("--interval", type=float, default=15.0)
    parser.add_argument("--stall-minutes", type=float, default=10.0)
    parser.add_argument("--agent-quiet-minutes", type=float, default=15.0)
    args = parser.parse_args(argv)
    run_dir = Path(args.run_dir).expanduser()
    if not (run_dir / "state.json").is_file():
        print(f"no state.json under {run_dir}", file=sys.stderr)
        return 2
    Watcher(run_dir, args.stall_minutes, args.agent_quiet_minutes).run(args.interval)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())