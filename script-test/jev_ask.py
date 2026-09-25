#!/usr/bin/env python3
"""Shell entry point to the Jev decision endpoint.

A long run is monitored from the shell; when an observed state needs a decision,
the answer should come from Jev instead of a guess. This wraps the harness in
``script-test/jev/`` so one command is enough:

    python3 jev_ask.py probe
    python3 jev_ask.py decide --state ctx.txt --question "Which fix first?" \\
        --options "revert|drop the commit,apply|fix the call site"
    python3 jev_ask.py gate --state ctx.txt --question "Is the tree consistent?"
    python3 jev_ask.py ask --spec spec.json

``decide`` prints the options ranked by Jev's probability; ``gate`` prints the
yes probability. Both print the model and the latency, so a report can cite the
call that produced a decision.

Transport defaults to ``openrouter``, which is what ``[compaction.jev]`` uses.
Credentials resolve from the environment first, then ``~/.llm-key``.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from jev import primitives  # noqa: E402
from jev.tier import TieredClient  # noqa: E402
from jev.transports import Client, TRANSPORTS, TransportError  # noqa: E402

LOG_PATH = Path(__file__).resolve().parent / "jev-decisions.jsonl"


def _client(transport: str, timeout_s: float) -> Client | TieredClient:
    """``tiered`` is the Laya-first policy; anything else is a direct transport."""
    if transport == "tiered":
        return TieredClient.build(timeout_s=timeout_s)
    return Client.build(transport, timeout_s=timeout_s)


def _render(key: str, answer: primitives.Answer) -> list[str]:
    """One answer as printable lines, ordered by what a reader needs first."""
    if isinstance(answer, primitives.NoulAnswer):
        verdict = "yes" if answer.noul >= 0.5 else "no"
        return [f"{key}: {verdict} (p_yes={answer.noul:.3f})"]
    if isinstance(answer, primitives.ChoiceAnswer):
        lines = [f"{key}: {answer.choice} (confidence={answer.confidence:.3f})"]
        for option, probability in answer.ranked():
            lines.append(f"  {probability:.3f}  {option}")
        return lines
    if isinstance(answer, primitives.ScoreAnswer):
        return [f"{key}: score={answer.score:.3f} (confidence={answer.confidence:.3f})"]
    return [f"{key}: {answer!r}"]


def _log(extra: dict[str, object] | None, response: primitives.Response) -> None:
    """Append one line per call: a decision must stay auditable after the run."""
    record: dict[str, object] = {
        "ts": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "argv": sys.argv[1:],
        "model": response.model,
        "latency_ms": round(response.latency_ms, 1),
        "answers": {
            key: _render(key, response.answers[key])[0] for key in sorted(response.answers)
        },
    }
    if response.source:
        record["source"] = response.source
        record["escalated"] = response.escalated
        record["agreement"] = response.agreement
        record["primary_confidence"] = round(response.primary_confidence, 4)
    if response.primary_error:
        record["primary_error"] = response.primary_error
    if extra:
        record.update({key: value for key, value in extra.items() if key != "argv"})
    try:
        with LOG_PATH.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(record, ensure_ascii=False) + "\n")
    except OSError:
        pass


def _emit(response: primitives.Response, extra: dict[str, object] | None = None) -> None:
    header: dict[str, object] = {
        "model": response.model,
        "latency_ms": round(response.latency_ms, 1),
        "input_tokens": response.input_tokens,
        "output_tokens": response.output_tokens,
    }
    if response.source:
        header["source"] = response.source
        header["primary_confidence"] = round(response.primary_confidence, 4)
        header["agreement"] = response.agreement
        header["escalated"] = response.escalated
    if response.primary_error:
        header["primary_error"] = response.primary_error
    if extra:
        header.update(extra)
    print(json.dumps(header, ensure_ascii=False))
    for key in sorted(response.answers):
        for line in _render(key, response.answers[key]):
            print(line)
    _log(extra, response)


def cmd_probe(args: argparse.Namespace) -> int:
    names = sorted(TRANSPORTS) if args.transport == "both" else [args.transport]
    failures = 0
    for name in names:
        try:
            with _client(name, args.timeout) as client:
                result = client.ask(
                    "The build failed twice on the same test; the third attempt is still running.",
                    {
                        "is_stuck": primitives.noul(
                            "Does the state describe a job that is stuck?",
                            true="Repeated identical failure",
                            false="Normal retry in flight",
                        ),
                        "next": primitives.choice(
                            "What should happen next?",
                            {"inspect": "Read the failure first", "retry": "Retry as-is"},
                        ),
                        "severity": primitives.score(
                            "How severe is the state?", ["Cosmetic", "Annoying", "Blocking"]
                        ),
                    },
                )
            print(
                json.dumps(
                    {
                        "transport": name,
                        "endpoint": client.transport.endpoint,
                        "key_source": client.key_source,
                        "model": result.model,
                        "latency_ms": round(result.latency_ms, 1),
                        "is_stuck": round(result.noul("is_stuck"), 3),
                        "next": result.choice("next").choice,
                        "severity": result.score("severity").score,
                    },
                    ensure_ascii=False,
                )
            )
        except Exception as error:  # noqa: BLE001 - a probe reports, it does not raise
            failures += 1
            print(json.dumps({"transport": name, "error": f"{type(error).__name__}: {error}"}))
    return 1 if failures == len(names) else 0


def _state_text(args: argparse.Namespace) -> str:
    if args.state_file:
        return Path(args.state_file).read_text(encoding="utf-8")
    if not sys.stdin.isatty():
        return sys.stdin.read()
    raise SystemExit("no state: pass --state-file or pipe it on stdin")


def _parse_options(raw: str) -> dict[str, str]:
    """Parse ``label|description`` entries, newline- or comma-separated.

    A comma inside a description would split it, so a part without a ``|`` is
    folded back into the previous entry.
    """
    parts = raw.split("\n") if "\n" in raw else raw.split(",")
    entries: list[str] = []
    for part in parts:
        part = part.strip()
        if not part:
            continue
        if entries and "|" not in part:
            entries[-1] = f"{entries[-1]}, {part}"
        else:
            entries.append(part)
    options = {}
    for entry in entries:
        label, _, description = entry.partition("|")
        label = label.strip()
        if label:
            options[label] = description.strip() or label
    return options


def cmd_decide(args: argparse.Namespace) -> int:
    options = _parse_options(args.options)
    if len(options) < 2:
        raise SystemExit("--options needs at least two 'label|description' entries")
    questions = {args.key: primitives.choice(args.question, options)}
    if args.gate:
        questions[f"{args.key}_sound"] = primitives.noul(
            args.gate, true="Yes", false="No"
        )
    with _client(args.transport, args.timeout) as client:
        response = client.ask(_state_text(args), questions)
    _emit(
        response,
        {
            "transport": args.transport,
            "question": args.question,
            "options": getattr(args, "options", None),
            "gate_question": getattr(args, "gate", None),
        },
    )
    if args.gate:
        sound = response.noul(f"{args.key}_sound")
        print(f"gate: {'pass' if sound >= 0.5 else 'fail'} (p={sound:.3f})")
    return 0


def cmd_gate(args: argparse.Namespace) -> int:
    questions = {
        args.key: primitives.noul(args.question, true=args.true, false=args.false)
    }
    with _client(args.transport, args.timeout) as client:
        response = client.ask(_state_text(args), questions)
    _emit(
        response,
        {
            "transport": args.transport,
            "question": args.question,
            "options": getattr(args, "options", None),
            "gate_question": getattr(args, "gate", None),
        },
    )
    return 0


def cmd_ask(args: argparse.Namespace) -> int:
    spec = json.loads(Path(args.spec).read_text(encoding="utf-8"))
    with _client(spec.get("transport", args.transport), args.timeout) as client:
        response = client.ask(spec["state"], spec["questions"])
    _emit(response, {"transport": spec.get("transport", args.transport)})
    return 0


def build_parser() -> argparse.ArgumentParser:
    # `--transport`/`--timeout` are accepted before and after the subcommand, so
    # the shared copy suppresses its defaults and the root parser owns them.
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument(
        "--transport",
        default=argparse.SUPPRESS,
        choices=sorted(TRANSPORTS) + ["both", "tiered"],
    )
    common.add_argument("--timeout", type=float, default=argparse.SUPPRESS)

    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0], parents=[common])
    parser.set_defaults(transport="openrouter", timeout=30.0)
    sub = parser.add_subparsers(dest="command", required=True)

    probe = sub.add_parser("probe", parents=[common], help="one cheap call per transport; proves it answers")
    probe.set_defaults(func=cmd_probe)

    for name, func, help_text in (
        ("decide", cmd_decide, "rank candidate actions for a state"),
        ("gate", cmd_gate, "one yes/no validation question"),
        ("ask", cmd_ask, "raw state + questions spec"),
    ):
        sub_parser = sub.add_parser(name, parents=[common], help=help_text)
        sub_parser.set_defaults(func=func)
        if name == "ask":
            sub_parser.add_argument("--spec", required=True)
            continue
        sub_parser.add_argument("--state-file")
        sub_parser.add_argument("--question", required=True)
        sub_parser.add_argument("--key", default="answer")
        if name == "decide":
            sub_parser.add_argument("--options", required=True, help="label|description,label|description")
            sub_parser.add_argument("--gate", help="extra yes/no question validating the pick")
        else:
            sub_parser.add_argument("--true", default="Yes")
            sub_parser.add_argument("--false", default="No")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return int(args.func(args))
    except TransportError as error:
        print(json.dumps({"error": f"transport: {error}", "status": error.status}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())