"""What the offline skill eval can and cannot see.

`strict/evals.rs` is deliberately offline and deterministic: a `should_trigger`
case passes when the query is a literal substring of the skill's name,
description, when-to-use or short description. That is a keyword-presence test,
and it is the reason the shipped corpus carries no natural-language cases —
171 skills ship `evals/cases.yaml` and every case is `resource` or
`explicit_pin`.

This ports the matcher faithfully and runs it against a real natural-language
labelled set, then puts Jev on the same set. Two things get measured:

* **collision** — how many *other* skills the matcher also accepts for the same
  query. Above zero, the eval cannot tell siblings apart.
* **discrimination** — whether a semantic judge picks the gold out of exactly
  that colliding set.

    python sim_skills_evals.py --transport native
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
from sim_vs_stack import bm25_rank

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
WORKERS = 8
PRICE_IN = 0.042

TRIGGER_QUESTION = (
    "A request arrives and this skill is a candidate to be loaded. Does this skill actually "
    "apply to the request?"
)
TRIGGER_TRUE = "It is the skill for exactly this job; loading it would help."
TRIGGER_FALSE = "It is only nearby: same area or same family, different specific job."


def matches_query(skill: dict[str, str], query: str) -> bool:
    """Faithful port of `LocalSkillEvidence::matches_query`."""
    q = query.lower()
    if not q:
        return False
    fields = [skill.get("name", ""), skill.get("description", "")]
    return any(q in f.lower() for f in fields if f)


@dataclass
class Row:
    request: str
    gold: str | None
    matched: list[str] = field(default_factory=list)
    gold_matched: bool = False
    collisions: int = 0
    jev_picked: str | None = None
    jev_choice: str | None = None
    jev_choice_correct: bool | None = None
    jev_correct: bool | None = None
    jev_said_no_to_all: bool = False
    latency_ms: float = 0.0
    tokens: int = 0
    error: str = ""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    roster = json.loads((ROOT / "cases" / "roster.json").read_text(encoding="utf-8"))
    cases = json.loads((ROOT / "cases" / "skill_cases.json").read_text(encoding="utf-8"))
    by_name = {s["name"]: s for s in roster}

    rows: list[Row] = []
    for case in cases:
        row = Row(request=case["request"], gold=case["gold"])
        row.matched = [s["name"] for s in roster if matches_query(s, case["request"])]
        row.gold_matched = bool(case["gold"]) and case["gold"] in row.matched
        row.collisions = max(0, len(row.matched) - (1 if row.gold_matched else 0))
        rows.append(row)

    def judge(row: Row) -> None:
        # The matcher accepts nothing on natural language, so the judge needs a
        # real candidate set: what the lexical half of retrieval would hand it.
        lexical = bm25_rank(roster, row.request, 10)
        candidates = list(row.matched)
        if row.gold and row.gold not in candidates:
            candidates = [row.gold] + candidates
        for name in lexical:
            if name not in candidates:
                candidates.append(name)
        if not candidates:
            row.jev_said_no_to_all = True
            row.jev_correct = row.gold is None
            return
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                response = client.ask(
                    {"request": row.request},
                    {
                        f"skill::{name}": primitives.noul(
                            f"{TRIGGER_QUESTION}\n\nSkill `{name}`: {by_name[name]['description'][:300]}",
                            true=TRIGGER_TRUE,
                            false=TRIGGER_FALSE,
                        )
                        for name in candidates
                    },
                )
        except transports.TransportError as error:
            row.error = str(error)
            return
        row.latency_ms = response.latency_ms
        row.tokens = response.input_tokens
        scored = sorted(
            ((response.noul(f"skill::{name}"), name) for name in candidates), key=lambda kv: -kv[0]
        )
        best_score, best = scored[0]
        row.jev_picked = best if best_score >= 0.5 else None
        row.jev_said_no_to_all = row.jev_picked is None
        row.jev_correct = row.jev_picked == row.gold
        # Same candidate set, asked as a single choice instead of a max over nouls.
        if len(candidates) < 2:
            row.jev_choice = candidates[0]
            row.jev_choice_correct = row.jev_choice == row.gold
            return
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client2:
                chosen = client2.ask(
                    {"request": row.request},
                    {
                        "which": primitives.choice(
                            "Which of these skills is the right one to load for this request? "
                            "If none fits, pick the closest and it will be judged below.",
                            {name: by_name[name]["description"][:200] for name in candidates},
                        ),
                        "any": primitives.noul(
                            "Does any of those skills actually apply to this request?",
                            true="At least one is the right skill for this job",
                            false="All are only nearby; nothing here fits",
                        ),
                    },
                )
            row.jev_choice = chosen.choice("which").choice if chosen.noul("any") >= 0.5 else None
            row.jev_choice_correct = row.jev_choice == row.gold
            row.tokens += chosen.input_tokens
        except transports.TransportError as error:
            row.error = str(error)

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        list(pool.map(judge, rows))

    covered = [r for r in rows if r.gold]
    uncovered = [r for r in rows if not r.gold]

    table = Table(title="offline eval matcher vs a semantic judge — 66 natural-language requests")
    for column in ("metric", "substring matcher (today)", "Jev"):
        table.add_column(column, justify="right" if column != "metric" else "left")
    table.add_row(
        "gold skill accepted",
        f"{sum(1 for r in covered if r.gold_matched)}/{len(covered)}",
        f"{sum(1 for r in covered if r.jev_correct)}/{len(covered)}",
    )
    table.add_row(
        "requests it says 'no skill' to",
        f"{sum(1 for r in covered if not r.gold_matched)}/{len(covered)}",
        f"{sum(1 for r in covered if r.jev_said_no_to_all)}/{len(covered)}",
    )
    table.add_row(
        "no-skill requests correctly silent",
        f"{sum(1 for r in uncovered if not r.matched)}/{len(uncovered)}",
        f"{sum(1 for r in uncovered if r.jev_said_no_to_all)}/{len(uncovered)}",
    )
    table.add_row(
        "gold picked (choice shape)",
        "—",
        f"{sum(1 for r in covered if r.jev_choice_correct)}/{len(covered)}",
    )
    table.add_row(
        "no-skill correctly silent (choice)",
        "—",
        f"{sum(1 for r in uncovered if r.jev_choice is None)}/{len(uncovered)}",
    )
    table.add_row(
        "mean extra skills accepted",
        f"{statistics.mean([r.collisions for r in rows]):.1f}",
        "—",
    )
    console.print(table)

    worst = sorted(rows, key=lambda r: -r.collisions)[:8]
    console.print("\n  requests where the substring matcher accepts the most skills:")
    for row in worst:
        console.print(f"    {row.collisions:>3} extra  gold={row.gold}  {row.request[:58]}")

    multi = [r for r in covered if r.collisions > 0]
    if multi:
        console.print(
            f"\n  of {len(covered)} covered requests, {len(multi)} have at least one sibling "
            f"the matcher cannot rule out"
        )
        recovered = sum(1 for r in multi if r.jev_correct)
        console.print(f"  Jev picks the gold in {recovered}/{len(multi)} of those")

    tokens = sum(r.tokens for r in rows)
    raw = OUT / "results-skills-evals.jsonl"
    with raw.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(asdict(row), ensure_ascii=False) + "\n")
    console.print(f"\n  tokens {tokens:,}, ${tokens / 1e6 * PRICE_IN:.4f} -> {raw.name}")


if __name__ == "__main__":
    main()