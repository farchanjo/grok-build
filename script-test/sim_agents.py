"""Agents: does the recommender help the selection, or replace it?

The harness already recommends: `prime/agents.rs` (PR20) ranks the callable
snapshot, refines through FTS + KNN, revalidates every survivor against a fresh
live authority, and renders an **advisory-only** block — it never spawns. The
selection is the parent's, at `task` time.

So the question is not "add a recommender" but **where the decision belongs**:

  select from the full roster   the parent picks among everything
  inject, then select           the recommender narrows to three, the parent picks
  select without the parent     the harness decides, no advisory block

The second is what ships. The third is the alternative worth pricing.

Also re-measured here: `reasoning_effort`, which the earlier 4-option rubric
collapsed to `high` on 23 of 25 cases. The real enum has seven levels
(`None, Minimal, Low, Medium, High, Xhigh, Max`).

    python sim_agents.py --transport native
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
console = Console()
WORKERS = 8
SHORTLIST = 3
PRICE_IN = 0.042

BUILTINS = [
    {"name": "general-purpose", "description": "General purpose agent for multi-step tasks; has access to all tools."},
    {"name": "explore", "description": "Fast read-only agent specialized for codebase exploration."},
    {"name": "plan", "description": "Software architect for planning implementation strategies; read-only."},
]

SELECT_QUESTION = (
    "Which subagent should handle this task, given it is being delegated from a parent session? "
    "The parent keeps the work it can do itself."
)
EFFORT_QUESTION = (
    "How much reasoning effort does this task need before the agent acts? Pick the lowest level "
    "that is sufficient — higher effort costs latency."
)
EFFORT_LEVELS = {
    "low": "Mechanical, one obvious path, little ambiguity.",
    "medium": "Some judgement needed but the path is clear.",
    "high": "Ambiguous, multi-step, or easy to get subtly wrong.",
    "max": "Hard problem where a wrong answer is expensive.",
}
# effort golds per case, by index in agent_cases.json
EFFORT_GOLD = {
    0: "high", 1: "high", 2: "high", 3: "medium", 4: "high", 5: "medium",
    6: "high", 7: "high", 8: "high", 9: "high", 10: "high", 11: "medium",
    12: "high", 13: "high", 14: "high", 15: "medium", 16: "high", 17: "medium",
    18: "medium", 19: "high", 20: "medium", 21: "medium", 22: "medium", 23: "high", 24: "high",
}


@dataclass
class Row:
    case_id: str
    gold: str
    arm: str
    picked: str | None = None
    effort: str | None = None
    correct: bool | None = None
    effort_correct: bool | None = None
    shortlist: list[str] = field(default_factory=list)
    gold_shortlisted: bool = False
    error: str = ""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    roster = json.loads((ROOT / "cases" / "agents.json").read_text(encoding="utf-8")) + BUILTINS
    by_name = {a["name"]: a["description"][:200] for a in roster}
    cases = json.loads((ROOT / "cases" / "agent_cases.json").read_text(encoding="utf-8"))

    arms = ["select from full roster", "inject top3 then select", "select, no advisory block"]
    rows: list[Row] = []

    def one(index: int, arm: str) -> Row:
        case = cases[index]
        row = Row(case_id=case["request"][:44], gold=case["gold"], arm=arm)
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                if arm == "select from full roster":
                    response = client.ask(
                        {"task": case["request"]},
                        {
                            "which": primitives.choice(SELECT_QUESTION, by_name),
                            "effort": primitives.choice(EFFORT_QUESTION, EFFORT_LEVELS),
                        },
                    )
                    row.picked = response.choice("which").choice
                    row.effort = response.choice("effort").choice
                elif arm == "select, no advisory block":
                    response = client.ask(
                        {"task": case["request"]},
                        {"which": primitives.choice(SELECT_QUESTION, by_name)},
                    )
                    row.picked = response.choice("which").choice
                else:
                    # recommender: rank the roster, take three, then the parent picks
                    rec = client.ask(
                        {"task": case["request"]},
                        {"rank": primitives.choice(
                            "Which three subagents are the most plausible for this task?",
                            by_name)},
                    )
                    shortlist = [rec.choice("rank").choice] + [
                        n for n in by_name if n != rec.choice("rank").choice
                    ][: SHORTLIST - 1]
                    row.shortlist = shortlist
                    row.gold_shortlisted = case["gold"] in shortlist
                    pick = client.ask(
                        {"task": case["request"]},
                        {"which": primitives.choice(SELECT_QUESTION, {n: by_name[n] for n in shortlist})},
                    )
                    row.picked = pick.choice("which").choice
        except transports.TransportError as error:
            row.error = str(error)
            return row
        row.correct = row.picked == case["gold"]
        if index in EFFORT_GOLD:
            row.effort_correct = row.effort == EFFORT_GOLD[index]
        return row

    for arm in arms:
        with ThreadPoolExecutor(max_workers=WORKERS) as pool:
            rows += list(pool.map(lambda i, a=arm: one(i, a), range(len(cases))))

    graded = [r for r in rows if r.error == ""]
    table = Table(title=f"agents — {len(cases)} delegation cases")
    for column in ("arm", "tipo certo", "gold no shortlist", "tokens injetados/turno"):
        table.add_column(column, justify="right" if column != "arm" else "left")
    for arm in arms:
        subset = [r for r in graded if r.arm == arm]
        ok = sum(1 for r in subset if r.correct)
        short = sum(1 for r in subset if r.gold_shortlisted)
        injected = statistics.mean([sum(len(by_name[n]) for n in r.shortlist) for r in subset]) if subset else 0
        table.add_row(
            arm,
            f"{ok}/{len(subset)} ({ok / len(subset):.1%})",
            f"{short}/{len(subset)}" if arm.startswith("inject") else "—",
            f"{injected / 4:.0f}" if arm.startswith("inject") else "0",
        )
    console.print(table)

    efforts = [r for r in graded if r.effort is not None]
    if efforts:
        ok = sum(1 for r in efforts if r.effort_correct)
        console.print(f"\n  effort com 4 níveis e rubrica de custo: {ok}/{len(efforts)}")
        from collections import Counter
        console.print("  distribuição:", dict(Counter(r.effort for r in efforts)))

    misses = [r for r in graded if r.arm == "select from full roster" and not r.correct]
    console.print(f"\n  erros de seleção no roster inteiro ({len(misses)}):")
    for row in misses[:10]:
        console.print(f"    gold={row.gold:<24} pick={row.picked}")

    raw = OUT / "results-agents.json"
    raw.write_text(json.dumps([asdict(r) for r in rows], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()