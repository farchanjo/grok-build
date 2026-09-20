"""Simulate the two-stage skill selector the cookbook prescribes.

Stage 1 ranks the whole roster on a short index and gates whether any skill is
wanted. Stage 2 re-reads only the shortlist, now with each candidate's full
description and the opening of its own SKILL.md, and is free to reject all of
them.

The earlier single-stage run showed why this matters: 75% of the misses already
had the right skill inside the top three. That number is the ceiling stage 2 can
reach, so this script measures how much of the ceiling the second read converts.

Also fixes the gate: the previous version averaged three nouls, which compresses
toward the middle and almost never fires. Here the gate is one question, and it
is scored separately from the pick.

    python sim_skills_stage2.py --transport native
"""

from __future__ import annotations

import argparse
import json
import statistics
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports

ROOT = Path(__file__).parent
OUT = ROOT / "out"
SKILLS_DIR = Path("~/.grok/skills").expanduser()
PRICE_IN = 0.042
WORKERS = 8
SHORTLIST = 3
GATE_THRESHOLD = 0.5
# The cookbook tunes this at 0.30 for the same question shape; a noul answering
# "does this one specifically fit" sits lower than a yes/no about urgency.
FITS_THRESHOLD = 0.30
BODY_EXCERPT = 700

console = Console()

GATE_QUESTION = (
    "Would a careful expert answering this request consult a specific documented procedure or "
    "set of commands, rather than answering from general knowledge alone?"
)

STAGE1_QUESTION = (
    "Which of these skills is the right one to load for this request? Read the descriptions "
    "as an index; the winner will be re-read in full afterwards."
)

# The roster carries umbrella skills (`dns`, `ospf`, `metrics`, `archive`) next to
# specific siblings (`bind9-dns`, `frr-ospf`, `prometheus-metrics`, `zstd-archive`).
# Both match; the specific one is the useful load. Saying so is the cheapest test of
# whether the confusion is a wording problem or a taxonomy problem.
SPECIFICITY_HINT = (
    " When a broad umbrella skill and a specific skill both cover the request, prefer the "
    "specific one: loading `dns` when `bind9-dns` fits wastes the detail."
)

STAGE2_QUESTION = (
    "Exactly one of these skills is the right one to load for this request. Read what each "
    "actually does, not just its name."
)


@dataclass
class Call:
    case_id: str
    gold: str | None
    gate: float | None = None
    stage1: str | None = None
    stage1_rank: int | None = None
    final: str | None = None
    correct: bool | None = None
    stage2_ran: bool = False
    latency_ms: float = 0.0
    input_tokens: int = 0
    error: str = ""
    extra: dict[str, Any] = field(default_factory=dict)


def body_excerpt(name: str, limit: int = BODY_EXCERPT) -> str:
    """The opening of a skill's SKILL.md, past the frontmatter."""
    path = SKILLS_DIR / name / "SKILL.md"
    if not path.is_file():
        return ""
    text = path.read_text(encoding="utf-8", errors="replace")
    if text.startswith("---"):
        end = text.find("\n---", 3)
        if end != -1:
            text = text[end + 4 :]
    return " ".join(text.split())[:limit]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--hint", action="store_true", help="append the specificity hint")
    parser.add_argument("--no-stage2", action="store_true", help="stage 1 only")
    parser.add_argument("--width", type=int, default=60,
                        help="roster description width; 60 = cookbook, 400 = the real MAX_LISTING_COMBINED_BYTES")
    args = parser.parse_args()

    OUT.mkdir(exist_ok=True)
    roster = json.loads((ROOT / "cases" / "roster.json").read_text(encoding="utf-8"))
    by_name = {s["name"]: s for s in roster}
    cases = json.loads((ROOT / "cases" / "skill_cases.json").read_text(encoding="utf-8"))
    if args.limit:
        cases = cases[: args.limit]

    short_index = {s["name"]: s["description"][: args.width] for s in roster}
    full_index = {s["name"]: s["description"] for s in roster}
    bodies = {s["name"]: body_excerpt(s["name"]) for s in roster}

    def one(index: int) -> tuple[int, Call]:
        case = cases[index]
        call = Call(case_id=case["request"][:48], gold=case["gold"])
        state = {"request": case["request"]}
        try:
            wide = client.ask(
                state,
                {
                    "gate": primitives.noul(GATE_QUESTION),
                    "which": primitives.choice(
                        STAGE1_QUESTION + (SPECIFICITY_HINT if args.hint else ""), short_index
                    ),
                },
            )
        except transports.TransportError as error:
            call.error = str(error)
            return index, call
        call.latency_ms = wide.latency_ms
        call.input_tokens = wide.input_tokens
        call.gate = wide.noul("gate")
        ranked = wide.choice("which").ranked()
        call.stage1 = ranked[0][0]
        call.extra["shortlist"] = [n for n, _ in ranked[:SHORTLIST]]

        if call.gate < GATE_THRESHOLD or args.no_stage2:
            call.final = None if call.gate < GATE_THRESHOLD else call.stage1
            call.correct = call.final == case["gold"]
            return index, call

        if case["gold"] and case["gold"] in dict(ranked[:SHORTLIST]):
            call.stage1_rank = [n for n, _ in ranked].index(case["gold"])
        shortlist = [n for n, _ in ranked[:SHORTLIST]]
        criteria = {
            name: f"{full_index[name]} — {bodies[name]}".strip(" —") for name in shortlist
        }
        # One absolute noul per candidate, each answered in isolation, each with
        # its own true/false criteria. A single noul asking "does the winner fit"
        # conflates the candidates and lands mid-scale.
        questions: dict[str, Any] = {"which": primitives.choice(STAGE2_QUESTION, criteria)}
        for name in shortlist:
            questions[f"fits::{name}"] = primitives.noul(
                f"Does the skill '{name}' do the specific thing this request asks for? "
                f"It is described as: {full_index[name][:200]}",
                true="It does exactly what the request needs",
                false="It is only nearby: same area, different specific job",
            )
        try:
            fine = client.ask(state, questions)
        except transports.TransportError as error:
            call.error = str(error)
            call.final = call.stage1
            call.correct = call.final == case["gold"]
            return index, call
        call.stage2_ran = True
        call.latency_ms += fine.latency_ms
        call.input_tokens += fine.input_tokens
        fits = {name: fine.noul(f"fits::{name}") for name in shortlist}
        call.extra["fits"] = fits
        call.extra["fits_best"] = max(fits.values())
        call.final = fine.choice("which").choice if max(fits.values()) >= FITS_THRESHOLD else None
        call.correct = call.final == case["gold"]
        return index, call

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        results: list[Call | None] = [None] * len(cases)
        with ThreadPoolExecutor(max_workers=WORKERS) as pool:
            for index, call in pool.map(one, range(len(cases))):
                results[index] = call
        calls = [c for c in results if c is not None]

    graded = [c for c in calls if c.correct is not None and not c.error]
    covered = [c for c in graded if c.gold]
    uncovered = [c for c in graded if not c.gold]

    ceiling = sum(1 for c in covered if c.gold in c.extra.get("shortlist", []))
    stage1_hits = sum(1 for c in covered if c.stage1 == c.gold)
    final_hits = sum(1 for c in covered if c.final == c.gold)
    stage2_ran = sum(1 for c in covered if c.stage2_ran)
    rescued = sum(1 for c in covered if c.stage2_ran and c.final == c.gold and c.stage1 != c.gold)
    broken = sum(1 for c in covered if c.stage2_ran and c.stage1 == c.gold and c.final != c.gold)
    needless = sum(1 for c in uncovered if c.final)
    gate_fn = sum(1 for c in covered if c.gate < GATE_THRESHOLD)

    table = Table(title=f"two-stage skill selector — {args.transport}")
    for column in ("metric", "value"):
        table.add_column(column)
    table.add_row("covered cases", str(len(covered)))
    table.add_row("stage 1 top-1", f"{stage1_hits / len(covered):.1%} ({stage1_hits})")
    table.add_row("gold inside stage-1 top-3 (ceiling)", f"{ceiling / len(covered):.1%} ({ceiling})")
    table.add_row("stage 2 executed", str(stage2_ran))
    table.add_row("rescued by stage 2", f"[green]{rescued}[/green]")
    table.add_row("broken by stage 2", f"[red]{broken}[/red]")
    table.add_row("final top-1", f"{final_hits / len(covered):.1%} ({final_hits})")
    table.add_row("gate false negatives (covered)", f"{gate_fn}")
    table.add_row("needless load (uncovered)", f"{needless}/{len(uncovered)}")
    table.add_row("p50 latency", f"{statistics.median([c.latency_ms for c in graded]):.0f} ms")
    table.add_row("input tokens", f"{sum(c.input_tokens for c in graded):,}")
    table.add_row("cost", f"${sum(c.input_tokens for c in graded) / 1e6 * PRICE_IN:.4f}")
    console.print(table)

    arm = ("hint" if args.hint else "plain") + ("_s1only" if args.no_stage2 else "_s2")
    print(
        f"SUMMARY arm={arm} stage1={stage1_hits / len(covered):.3f} "
        f"final={final_hits / len(covered):.3f} rescued={rescued} broken={broken} "
        f"ceiling={ceiling / len(covered):.3f} needless={needless}/{len(uncovered)} "
        f"tokens={sum(c.input_tokens for c in graded)}"
    )
    raw = OUT / f"results-skills-{arm}-{args.transport}.jsonl"
    with raw.open("w", encoding="utf-8") as handle:
        for call in calls:
            handle.write(json.dumps(asdict(call), ensure_ascii=False) + "\n")
    console.print(f"\n-> {raw.relative_to(ROOT)}")


if __name__ == "__main__":
    main()