"""Efficacy harness for Jev over four decision points.

Runs the same labelled case sets the integration would face, on one or both
transports, and reports what actually matters for a decision layer: does it
choose right, is it calibrated, how long does it take, what does it cost.

    python run_eval.py --transport native --experiment all
    python run_eval.py --transport openrouter --experiment skills --variants short

Case sets live in ``cases/`` and every gold label is validated against the live
profile rosters before a single call goes out.
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Callable

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from jev.tier import TieredClient

ROOT = Path(__file__).parent
CASES = ROOT / "cases"
OUT = ROOT / "out"

# $ per 1M tokens (input, output) as published by TypeSafe.
PRICE_IN, PRICE_OUT = 0.042, 0.0
WORKERS = 8

# Thresholds, kept in one place because they are the thing a human reviews.
GATE_THRESHOLD = 0.30
SHORTLIST = 3
STORE_THRESHOLD = 0.5

console = Console()


@dataclass
class Call:
    """One measured request."""

    experiment: str
    variant: str
    case_id: str
    latency_ms: float = 0.0
    input_tokens: int = 0
    output_tokens: int = 0
    picked: str | None = None
    gold: str | None = None
    value: float | None = None
    correct: bool | None = None
    error: str = ""
    extra: dict[str, Any] = field(default_factory=dict)


def load(name: str) -> list[dict[str, Any]]:
    return json.loads((CASES / name).read_text(encoding="utf-8"))


def label(entry: dict[str, Any], fallback: str) -> dict[str, Any]:
    """A stable case id, so a raw log can be read back against the case set."""
    entry.setdefault("id", entry.get(fallback, "")[:48])
    return entry


def run_cases(
    client: transports.Client | TieredClient,
    cases: list[dict[str, Any]],
    experiment: str,
    variant: str,
    fn: Callable[[dict[str, Any], int], Call],
) -> list[Call]:
    """Run every case through ``fn`` in a small pool, preserving case order."""
    results: list[Call | None] = [None] * len(cases)

    def one(index: int) -> tuple[int, Call]:
        return index, fn(cases[index], index)

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        for index, call in pool.map(one, range(len(cases))):
            results[index] = call
    return [c for c in results if c is not None]


def measure(call: Call, response: primitives.Response) -> Call:
    call.latency_ms = response.latency_ms
    call.input_tokens = response.input_tokens
    call.output_tokens = response.output_tokens
    if response.source:
        call.extra["source"] = response.source
        call.extra["primary_confidence"] = round(response.primary_confidence, 4)
        call.extra["agreement"] = response.agreement
        call.extra["escalated"] = response.escalated
    return call


def build_client(name: str, timeout_s: float = 60.0) -> tuple[Any, str]:
    """A direct client, or the two-tier policy when ``name`` is ``tiered``.

    Returns the client and a banner line. The tiered client mirrors ``Client.ask``
    so every experiment below runs unchanged against either.
    """
    if name != "tiered":
        client = transports.Client.build(name, timeout_s=timeout_s)
        banner = (
            f"transport=[bold]{client.transport.name}[/bold] endpoint={client.transport.endpoint} "
            f"model={client.transport.model} key={client.key_source}"
        )
        return client, banner
    tier = TieredClient.build(timeout_s=timeout_s)
    banner = (
        f"transport=[bold]tiered[/bold] {tier.primary.transport.name} "
        f"({tier.primary.transport.model}) -> gate -> {tier.secondary.transport.name} "
        f"gates={tier.gates} default={tier.default_gate}"
    )
    return tier, banner


# ── experiments ────────────────────────────────────────────────────────────


def skill_questions(roster: list[dict[str, str]], width: int | None) -> dict[str, Any]:
    criteria = {
        skill["name"]: (skill["description"] if width is None else skill["description"][:width])
        for skill in roster
    }
    return {
        "which": primitives.choice(
            "Which of these skills, if any, is the right one to load to help with the "
            "user's latest request?",
            criteria,
        ),
        "gate::acts": primitives.noul(
            "Is the assistant being asked to act on the user's files, accounts, devices, or "
            "online services, rather than only to explain or advise?"
        ),
        "gate::procedure": primitives.noul(
            "Would a careful expert answering this consult a specific documented procedure or "
            "set of commands, rather than answering from general understanding?"
        ),
        "gate::prose": primitives.noul(
            "Could a knowledgeable generalist fully satisfy this request in prose, with no "
            "tools, no documentation, and no access to the user's files or accounts?"
        ),
    }


def experiment_skills(client: transports.Client | TieredClient, width: int | None, variant: str) -> list[Call]:
    roster = load("roster.json")
    cases = [label(c, "request") for c in load("skill_cases.json")]

    def one(case: dict[str, Any], index: int) -> Call:
        call = Call("skills", variant, case["id"], gold=case["gold"])
        try:
            response = client.ask(
                {"request": case["request"], "recent_context": ""},
                skill_questions(roster, width),
            )
        except transports.TransportError as error:
            call.error = str(error)
            return call
        measure(call, response)
        gate_values = [response.noul(f"gate::{k}") for k in ("acts", "procedure", "prose")]
        oriented = [gate_values[0], gate_values[1], 1.0 - gate_values[2]]
        call.value = sum(oriented) / len(oriented)
        if call.value < GATE_THRESHOLD:
            call.picked = None
        else:
            ranked = response.choice("which").ranked()[:SHORTLIST]
            call.picked = ranked[0][0] if ranked else None
            call.extra["shortlist"] = [name for name, _ in ranked]
            call.extra["choice_confidence"] = response.choice("which").confidence
        call.correct = call.picked == case["gold"]
        return call

    return run_cases(client, cases, "skills", variant, one)


def experiment_agents(client: transports.Client) -> list[Call]:
    roster = load("agents.json") + [
        {"name": n, "description": d}
        for n, d in (
            ("general-purpose", "General purpose agent for multi-step tasks; all tools."),
            ("explore", "Fast read-only agent specialized for codebase exploration."),
            ("plan", "Software architect for planning implementation strategies; read-only."),
        )
    ]
    cases = [label(c, "request") for c in load("agent_cases.json")]
    criteria = {a["name"]: a["description"] for a in roster}

    def one(case: dict[str, Any], index: int) -> Call:
        call = Call("agents", "full", case["id"], gold=case["gold"])
        questions = {
            "which": primitives.choice(
                "Which subagent should handle this task, given it is being delegated from a "
                "parent session?",
                criteria,
            ),
            "effort": primitives.choice(
                "How much reasoning effort does this task need before acting?",
                {
                    "low": "Mechanical, one obvious path, little ambiguity",
                    "medium": "Some judgement needed but the path is clear",
                    "high": "Ambiguous, multi-step, or easy to get subtly wrong",
                    "max": "Hard problem where a wrong answer is expensive",
                },
            ),
        }
        try:
            response = client.ask({"task": case["request"]}, questions)
        except transports.TransportError as error:
            call.error = str(error)
            return call
        measure(call, response)
        call.picked = response.choice("which").choice
        call.extra["effort"] = response.choice("effort").choice
        call.correct = call.picked == case["gold"]
        return call

    return run_cases(client, cases, "agents", "full", one)


def experiment_memory(client: transports.Client) -> list[Call]:
    cases = [label(c, "request") for c in load("memory_cases.json")]

    def one(case: dict[str, Any], index: int) -> Call:
        call = Call("memory", "full", case["id"], gold=case["gold"])
        question = {
            "should_store": primitives.noul(
                "This candidate note is being considered for long-term memory. Should it be "
                "stored, so a future session recalls it?",
                true="Durable fact, rule, preference, or verified result worth recalling later",
                false="Momentary state, trivially rediscoverable, or a duplicate",
            )
        }
        try:
            response = client.ask({"candidate": case["request"]}, question)
        except transports.TransportError as error:
            call.error = str(error)
            return call
        measure(call, response)
        call.value = response.noul("should_store")
        call.picked = "store" if call.value >= STORE_THRESHOLD else "skip"
        call.correct = call.picked == case["gold"]
        return call

    return run_cases(client, cases, "memory", "full", one)


def experiment_workflow(client: transports.Client) -> list[Call]:
    cases = [label(c, "intent") for c in load("workflow_cases.json")]

    def one(case: dict[str, Any], index: int) -> Call:
        call = Call("workflow", "full", case["id"], gold=case["gold"])
        questions = {
            "name": primitives.choice(
                "Which name is best for this workflow? Prefer a name that is short, "
                "specific, and searchable by what it produces.",
                {candidate: None for candidate in case["candidates"]},
            )
        }
        try:
            response = client.ask({"workflow": case["intent"]}, questions)
        except transports.TransportError as error:
            call.error = str(error)
            return call
        measure(call, response)
        call.picked = response.choice("name").choice
        call.correct = call.picked == case["gold"]
        return call

    return run_cases(client, cases, "workflow", "full", one)


# ── reporting ──────────────────────────────────────────────────────────────


def pct(part: int, whole: int) -> str:
    return f"{part / whole:.1%}" if whole else "-"


def calibration(values: list[tuple[float, bool]]) -> list[tuple[str, int, float, float]]:
    """Bucket confidence against observed accuracy: a calibrated model is diagonal."""
    buckets = [(0.0, 0.2), (0.2, 0.4), (0.4, 0.6), (0.6, 0.8), (0.8, 1.01)]
    rows: list[tuple[str, int, float, float]] = []
    for low, high in buckets:
        inside = [(v, ok) for v, ok in values if low <= v < high]
        if not inside:
            continue
        rows.append(
            (
                f"{low:.1f}-{high:.1f}",
                len(inside),
                sum(v for v, _ in inside) / len(inside),
                sum(1 for _, ok in inside if ok) / len(inside),
            )
        )
    return rows


def summarise(calls: list[Call]) -> dict[str, Any]:
    ok = [c for c in calls if not c.error]
    graded = [c for c in ok if c.correct is not None]
    latencies = sorted(c.latency_ms for c in ok)
    tokens_in = sum(c.input_tokens for c in ok)
    tokens_out = sum(c.output_tokens for c in ok)
    return {
        "calls": len(calls),
        "errors": len(calls) - len(ok),
        "graded": len(graded),
        "correct": sum(1 for c in graded if c.correct),
        "accuracy": (sum(1 for c in graded if c.correct) / len(graded)) if graded else None,
        "latency_p50": statistics.median(latencies) if latencies else None,
        "latency_p95": latencies[int(len(latencies) * 0.95) - 1] if latencies else None,
        "tokens_in": tokens_in,
        "tokens_out": tokens_out,
        "cost_usd": tokens_in / 1e6 * PRICE_IN + tokens_out / 1e6 * PRICE_OUT,
    }


def report(transport: str, all_calls: list[Call]) -> None:
    console.rule(f"[bold]{transport}")
    table = Table(show_lines=False)
    for column in ("experiment", "variant", "n", "acc", "p50 ms", "p95 ms", "in tok", "cost $"):
        table.add_column(column, justify="right" if column not in ("experiment", "variant") else "left")

    grouped: dict[tuple[str, str], list[Call]] = {}
    for call in all_calls:
        grouped.setdefault((call.experiment, call.variant), []).append(call)
    for (experiment, variant), calls in sorted(grouped.items()):
        stats = summarise(calls)
        table.add_row(
            experiment,
            variant,
            str(stats["graded"]),
            pct(stats["correct"], stats["graded"]) if stats["graded"] else "-",
            f"{stats['latency_p50']:.0f}" if stats["latency_p50"] else "-",
            f"{stats['latency_p95']:.0f}" if stats["latency_p95"] else "-",
            f"{stats['tokens_in']:,}",
            f"{stats['cost_usd']:.4f}",
        )
    console.print(table)

    # Skill routing is the case with published external baselines, so it gets
    # the extra split between wrong picks and needless ones.
    for variant in sorted({c.variant for c in all_calls if c.experiment == "skills"}):
        subset = [c for c in all_calls if c.experiment == "skills" and c.variant == variant and not c.error]
        covered = [c for c in subset if c.gold]
        uncovered = [c for c in subset if not c.gold]
        wrong = sum(1 for c in covered if c.picked != c.gold)
        needless = sum(1 for c in uncovered if c.picked)
        console.print(
            f"  skills/{variant}: wrong load [red]{pct(wrong, len(covered))}[/red] "
            f"({wrong}/{len(covered)})   needless load [red]{pct(needless, len(uncovered))}[/red] "
            f"({needless}/{len(uncovered)})"
        )

    for experiment in ("memory", "skills"):
        for variant in sorted({c.variant for c in all_calls if c.experiment == experiment}):
            pairs = [
                (c.value, bool(c.correct))
                for c in all_calls
                if c.experiment == experiment and c.variant == variant and c.value is not None
            ]
            rows = calibration(pairs)
            if not rows:
                continue
            console.print(f"  calibration {experiment}/{variant}:")
            for bucket, n, mean_value, observed in rows:
                console.print(
                    f"    {bucket}  n={n:<3} mean={mean_value:.2f}  observed={observed:.1%}"
                )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--transport", default="native", choices=sorted(transports.TRANSPORTS) + ["tiered"])
    parser.add_argument(
        "--experiment",
        default="all",
        choices=["all", "skills", "agents", "memory", "workflow"],
    )
    parser.add_argument(
        "--variants",
        default="both",
        choices=["both", "short", "full"],
        help="roster description width for the skills experiment",
    )
    parser.add_argument("--limit", type=int, default=0, help="cap cases per experiment (0 = all)")
    args = parser.parse_args()

    OUT.mkdir(exist_ok=True)
    started = time.time()
    all_calls: list[Call] = []

    client, banner = build_client(args.transport)
    with client:
        console.print(banner)
        if args.experiment in ("all", "skills"):
            widths: list[tuple[int | None, str]] = (
                [(60, "short"), (None, "full")] if args.variants == "both" else [(60, "short") if args.variants == "short" else (None, "full")]
            )
            for width, variant in widths:
                all_calls += experiment_skills(client, width, variant)
        if args.experiment in ("all", "agents"):
            all_calls += experiment_agents(client)
        if args.experiment in ("all", "memory"):
            all_calls += experiment_memory(client)
        if args.experiment in ("all", "workflow"):
            all_calls += experiment_workflow(client)

    report(args.transport, all_calls)
    if isinstance(client, TieredClient):
        stats = client.stats
        # `primary_failures` first: a dead cheap leg makes every call escalate,
        # which otherwise reads as a gate that never fires.
        console.print(
            f"\ntier: {stats.calls} calls, primary failed {stats.primary_failures}, "
            f"gate accepted {stats.accepted}, "
            f"escalated {stats.escalated} ({stats.escalation_rate:.1%}), "
            f"agreed {stats.agreed}, disagreed {stats.disagreed}"
        )

    raw = OUT / f"results-{args.transport}.jsonl"
    with raw.open("w", encoding="utf-8") as handle:
        for call in all_calls:
            handle.write(json.dumps(asdict(call), ensure_ascii=False) + "\n")
    total = summarise(all_calls)
    console.print(
        f"\n{total['calls']} calls, {total['errors']} errors, "
        f"{total['tokens_in']:,} input tokens, ${total['cost_usd']:.4f}, "
        f"{time.time() - started:.0f}s wall -> {raw.relative_to(ROOT)}"
    )


if __name__ == "__main__":
    main()