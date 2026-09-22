#!/usr/bin/env python3
"""Verification: does the two-tier policy beat Jev alone, and where?

Runs every experiment three times — Laya alone, Jev alone, and the tier — on the
same case sets and the same worker pool, then reports the comparison that decides
whether the integration is worth keeping:

  * accuracy per arm, so the tier can be shown not to lose any
  * escalation rate and Jev calls avoided, which is the whole point of the tier
  * p50 latency and paid-token cost, the resources the tier is meant to save
  * the decision table: cases the tier got right that Jev alone got wrong, and
    the reverse, since a tier that flips either way needs to say which

Laya is deterministic, so one run per case is a full sample. Jev is deterministic
for a resolved model build; ``--repeats`` re-runs the paid arm when a variance
estimate is wanted.

    python verify_tier.py --experiment all
    python verify_tier.py --experiment skills --repeats 2
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from jev.tier import TieredClient

ROOT = Path(__file__).parent
CASES = ROOT / "cases"
OUT = ROOT / "out" / "verify"
WORKERS = 8
PRICE_IN = 0.042  # $ per 1M input tokens, TypeSafe published rate

console = Console()


@dataclass
class Row:
    """One case through one arm."""

    arm: str
    experiment: str
    case_id: str
    gold: str | None = None
    picked: str | None = None
    confidence: float = 0.0
    latency_ms: float = 0.0
    input_tokens: int = 0
    source: str = ""
    primary_confidence: float = 0.0
    escalated: bool = False
    agreement: bool | None = None
    error: str = ""

    @property
    def correct(self) -> bool | None:
        return None if self.gold is None else self.picked == self.gold


@dataclass
class Arm:
    """One decision configuration, built lazily so arms never share a client."""

    name: str
    factory: Callable[[], Any]
    rows: list[Row] = field(default_factory=list)


def load(name: str) -> list[dict[str, Any]]:
    return json.loads((CASES / name).read_text(encoding="utf-8"))


def case_id(entry: dict[str, Any], key: str) -> str:
    return entry.get("id") or entry.get(key, "")[:48]


# ── the three experiments, written once and run by every arm ────────────────


def skill_task(roster: list[dict]) -> Callable[[Any, dict], tuple[dict, dict, str]]:
    criteria = {s["name"]: s["description"][:60] for s in roster}

    def build(client: Any, case: dict) -> tuple[dict, dict, str]:
        questions = {
            "which": primitives.choice(
                "Which of these skills, if any, is the right one to load to help with the "
                "user's latest request?",
                criteria,
            )
        }
        return {"request": case["request"], "recent_context": ""}, questions, "which"

    return build


def agent_task(roster: list[dict]) -> Callable[[Any, dict], tuple[dict, dict, str]]:
    criteria = {a["name"]: a["description"][:120] for a in roster}

    def build(client: Any, case: dict) -> tuple[dict, dict, str]:
        questions = {
            "which": primitives.choice(
                "Which subagent should handle this task, given it is being delegated from a "
                "parent session?",
                criteria,
            )
        }
        return {"task": case["request"]}, questions, "which"

    return build


def memory_task() -> Callable[[Any, dict], tuple[dict, dict, str]]:
    def build(client: Any, case: dict) -> tuple[dict, dict, str]:
        questions = {
            "should_store": primitives.noul(
                "This candidate note is being considered for long-term memory. Should it be "
                "stored, so a future session recalls it?",
                true="Durable fact, rule, preference, or verified result worth recalling later",
                false="Momentary state, trivially rediscoverable, or a duplicate",
            )
        }
        return {"candidate": case["request"]}, questions, "should_store"

    return build


def workflow_task() -> Callable[[Any, dict], tuple[dict, dict, str]]:
    def build(client: Any, case: dict) -> tuple[dict, dict, str]:
        questions = {
            "name": primitives.choice(
                "Which name is best for this workflow? Prefer a name that is short, "
                "specific, and searchable by what it produces.",
                {c: c for c in case["candidates"]},
            )
        }
        return {"workflow": case["intent"]}, questions, "name"

    return build


EXPERIMENTS: dict[str, Callable[[], Callable[[Any, dict], tuple[dict, dict, str]]]] = {
    "skills": lambda: skill_task(load("roster.json")),
    "agents": lambda: agent_task(load("agents.json")),
    "memory": memory_task,
    "workflow": workflow_task,
}

CASE_FILES = {
    "skills": ("skill_cases.json", "request"),
    "agents": ("agent_cases.json", "request"),
    "memory": ("memory_cases.json", "request"),
    "workflow": ("workflow_cases.json", "intent"),
}


def to_gold_label(experiment: str, picked: str | None, response: primitives.Response,
                  driver: str) -> str | None:
    """Map a wire answer onto the case set's own vocabulary.

    ``Response.pick`` speaks the wire (``yes``/``no`` for a noul); the memory case
    set labels the same outcome ``store``/``skip``.
    """
    if experiment == "memory":
        answer = response.answers.get(driver)
        if not isinstance(answer, primitives.NoulAnswer):
            return None
        return "store" if answer.noul >= 0.5 else "skip"
    return picked


def run_arm(arm: Arm, experiment: str, limit: int) -> list[Row]:
    """Every case of one experiment through one arm, order preserved."""
    filename, key = CASE_FILES[experiment]
    cases = load(filename)
    if limit:
        cases = cases[:limit]
    build = EXPERIMENTS[experiment]()

    def one(case: dict) -> Row:
        row = Row(arm.name, experiment, case_id(case, key), gold=case.get("gold"))
        try:
            with arm.factory() as client:
                state, questions, driver = build(client, case)
                started = time.perf_counter()
                response = client.ask(state, questions)
                if not response.latency_ms:
                    response.latency_ms = (time.perf_counter() - started) * 1000
        except Exception as error:  # noqa: BLE001 - an arm reports, it does not raise
            row.error = f"{type(error).__name__}: {error}"
            return row
        row.picked = to_gold_label(experiment, response.pick(driver), response, driver)
        row.confidence = response.confidence(driver)
        row.latency_ms = response.latency_ms
        row.input_tokens = response.input_tokens
        row.source = response.source
        row.primary_confidence = response.primary_confidence
        row.escalated = response.escalated
        row.agreement = response.agreement
        return row

    with ThreadPoolExecutor(WORKERS) as pool:
        return list(pool.map(one, cases))


# ── reporting ───────────────────────────────────────────────────────────────


def stats(rows: list[Row]) -> dict[str, Any]:
    ok = [r for r in rows if not r.error and r.correct is not None]
    lat = sorted(r.latency_ms for r in ok)
    return {
        "n": len(ok),
        "errors": len(rows) - len(ok),
        "correct": sum(1 for r in ok if r.correct),
        "accuracy": (sum(1 for r in ok if r.correct) / len(ok)) if ok else None,
        "p50": statistics.median(lat) if lat else None,
        "p95": lat[int(len(lat) * 0.95) - 1] if lat else None,
        "tokens_in": sum(r.input_tokens for r in ok),
        "cost": sum(r.input_tokens for r in ok) / 1e6 * PRICE_IN,
        "escalated": sum(1 for r in ok if r.escalated),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--experiment", default="all",
                        choices=["all", *EXPERIMENTS])
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--repeats", type=int, default=1,
                        help="repeat each arm and keep the last run (Laya is deterministic)")
    parser.add_argument("--laya-model", default="typed-decisions")
    args = parser.parse_args()

    names = list(EXPERIMENTS) if args.experiment == "all" else [args.experiment]
    arms = {
        "laya": Arm("laya", lambda: transports.Client.build("laya", model=args.laya_model, timeout_s=60.0)),
        "jev": Arm("jev", lambda: transports.Client.build("openrouter", timeout_s=60.0)),
        "tier": Arm("tier", lambda: TieredClient.build(primary_model=args.laya_model, timeout_s=60.0)),
    }

    OUT.mkdir(parents=True, exist_ok=True)
    started = time.time()
    history: dict[tuple[str, str], list[float]] = {}
    for rep in range(max(1, args.repeats)):
        for arm in arms.values():
            arm.rows = []
            for experiment in names:
                arm.rows += run_arm(arm, experiment, args.limit)
        for experiment in names:
            for arm_name, arm in arms.items():
                subset = [r for r in arm.rows if r.experiment == experiment]
                acc = stats(subset)["accuracy"]
                if acc is not None:
                    history.setdefault((experiment, arm_name), []).append(acc)
    if args.repeats > 1:
        console.print("\n[bold]per-repeat accuracy (Jev is not deterministic across runs)[/bold]")
        for (experiment, arm_name), values in sorted(history.items()):
            spread = f"{min(values):.1%}..{max(values):.1%}"
            console.print(f"  {experiment:<9} {arm_name:<5} n={len(values)} {spread}")

    table = Table(title="two-tier policy vs each engine alone")
    for column in ("experiment", "arm", "n", "acc", "p50 ms", "p95 ms", "in tok", "cost $", "paid", "avoided"):
        table.add_column(column, justify="left" if column in ("experiment", "arm") else "right")

    for experiment in names:
        for arm_name, arm in arms.items():
            subset = [r for r in arm.rows if r.experiment == experiment]
            s = stats(subset)
            paid = s["escalated"]
            table.add_row(
                experiment, arm_name, str(s["n"]),
                f"{s['accuracy']:.1%}" if s["accuracy"] is not None else "-",
                f"{s['p50']:.0f}" if s["p50"] else "-",
                f"{s['p95']:.0f}" if s["p95"] else "-",
                f"{s['tokens_in']:,}", f"{s['cost']:.4f}",
                f"{paid}/{s['n']}" if arm_name == "tier" else "-",
                f"{1 - paid / s['n']:.0%}" if arm_name == "tier" and s["n"] else "-",
            )
    console.print(table)

    # A tiered answer equals Jev's on every escalated case by construction, so the
    # only place the two can diverge is the gate-passed subset. Report that split
    # explicitly, and the splits the secondary flagged for review.
    console.print("\n[bold]gate-passed subset (the only place the tier can differ from Jev)[/bold]")
    for experiment in names:
        tier = [r for r in arms["tier"].rows if r.experiment == experiment]
        passed = [r for r in tier if not r.escalated and r.correct is not None]
        flagged = [r for r in tier if r.agreement is False]
        acc = sum(1 for r in passed if r.correct) / len(passed) if passed else 0.0
        console.print(
            f"  {experiment:<9} gate-passed {len(passed):>3}/{len(tier):<3} "
            f"accuracy_on_passed {acc:>6.1%}   split_for_review {len(flagged)}"
        )

    # The decision table: what the tier changes relative to Jev alone.
    console.print("\n[bold]tier vs jev-alone, per case[/bold]")
    for experiment in names:
        tier = {r.case_id: r for r in arms["tier"].rows if r.experiment == experiment}
        jev = {r.case_id: r for r in arms["jev"].rows if r.experiment == experiment}
        laya = {r.case_id: r for r in arms["laya"].rows if r.experiment == experiment}
        gained, lost, both_wrong = [], [], []
        for cid, t in tier.items():
            j = jev.get(cid)
            if j is None or t.correct is None or j.correct is None:
                continue
            if t.correct and not j.correct:
                gained.append(cid)
            elif j.correct and not t.correct:
                lost.append((cid, laya.get(cid)))
            elif not t.correct and not j.correct:
                both_wrong.append(cid)
        t_acc = stats(list(tier.values()))["accuracy"] or 0.0
        j_acc = stats(list(jev.values()))["accuracy"] or 0.0
        console.print(
            f"  {experiment:<9} tier {t_acc:.1%} vs jev {j_acc:.1%}  "
            f"gained [green]{len(gained)}[/green]  lost [red]{len(lost)}[/red]  both wrong {len(both_wrong)}"
        )
        for cid, laya_row in lost[:4]:
            conf = f"{laya_row.confidence:.3f}" if laya_row else "-"
            console.print(f"      lost: {cid[:56]:<56} laya_conf={conf} escalated={laya_row.escalated if laya_row else '?'}")

    raw = OUT / f"verify-{int(started)}.jsonl"
    with raw.open("w", encoding="utf-8") as handle:
        for arm in arms.values():
            for row in arm.rows:
                handle.write(json.dumps(row.__dict__, ensure_ascii=False) + "\n")
    console.print(f"\n{time.time() - started:.0f}s wall -> {raw.relative_to(ROOT)}")


if __name__ == "__main__":
    main()