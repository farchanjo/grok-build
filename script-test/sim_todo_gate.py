"""The todo gate: a deterministic backstop, and the one decision it does not make.

`evaluate_todo_gate` is four lines: if there are pending todos or in-progress ones
with no backing task, nudge. It lists **every** item, in insertion order, with no
cap, and the reminder says to "advance the next pending todo".

"Next" is insertion order. Nothing checks whether that item is *actionable* — a
todo can be blocked on a credential, on another item, or on a decision, while a
later one is ready to go. That is the decision worth testing.

Two things measured:

1. **The reminder's size** as the list grows, since it dumps everything.
2. **Whether relevance picks the actionable item** better than insertion order.

    python sim_todo_gate.py --transport native
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

QUESTION = (
    "The agent ended its turn with pending todos and no backing task. Which single pending item "
    "should it advance next? Prefer one it can act on now; skip items blocked on a credential, on "
    "another item, or on a decision that has not been made."
)
CRITERIA = (
    "Pick the item the agent can start right now with the tools it has. If an earlier item is "
    "blocked, a later one is the answer."
)


def fixture(objective: str, items: list[tuple[str, bool]]) -> dict[str, Any]:
    """items: (text, actionable). The gold is the first actionable one."""
    gold = next((t for t, ok in items if ok), None)
    return {"objective": objective, "items": [t for t, _ in items], "gold": gold}


FIXTURES = [
    fixture("Ship the export endpoint", [
        ("Add the CSV writer", False),      # blocked: needs a decision on columns
        ("Wire the route", False),          # blocked: depends on the writer
        ("Add the auth check to the route", True),
        ("Write the doc", True),
    ]),
    fixture("Fix the flaky test", [
        ("Reproduce it 50 times locally", True),
        ("Bisect the history", False),
        ("Add a retry", False),
    ]),
    fixture("Migrate the schema", [
        ("Get the prod connection string from the user", False),
        ("Write the migration", True),
        ("Run it on staging", True),
    ]),
    fixture("Cut the release", [
        ("Wait for CI", False),
        ("Write the changelog", True),
        ("Tag", False),
    ]),
    fixture("Add retry to the client", [
        ("Decide the backoff policy", False),
        ("Implement the loop", True),
        ("Add a test", True),
    ]),
    fixture("Clean the lint backlog", [
        ("Fix unused imports", True),
        ("Fix the naming violations", True),
        ("Fix the line lengths", True),
    ]),
    fixture("Ship the CLI flag", [
        ("Parse the flag", True),
        ("Wire it to the behaviour", False),
        ("Document it", False),
    ]),
    fixture("Investigate the leak", [
        ("Take a heap snapshot", True),
        ("Compare with baseline", False),
    ]),
    fixture("Set up the pipeline", [
        ("Ask which registry to publish to", False),
        ("Write the workflow file", True),
        ("Add the secrets", False),
    ]),
    fixture("Port the parser", [
        ("Port the tokenizer", True),
        ("Port the AST", False),
        ("Port the emitter", False),
    ]),
    fixture("Harden the endpoint", [
        ("Add rate limiting", True),
        ("Add request size limits", True),
        ("Add structured errors", True),
    ]),
    fixture("Reduce the build time", [
        ("Measure where the time goes", True),
        ("Split the crate", False),
    ]),
    fixture("Write the runbook", [
        ("Ask for the on-call rotation", False),
        ("Draft the sections", True),
    ]),
    fixture("Add the integration test", [
        ("Stand up a test container", True),
        ("Write the assertions", False),
    ]),
    fixture("Refactor the module", [
        ("Extract the pure core", True),
        ("Move the IO to the edge", False),
        ("Update the call sites", False),
    ]),
    fixture("Fix the docs build", [
        ("Reproduce the failure", True),
        ("Pin the dependency", False),
    ]),
    fixture("Add telemetry", [
        ("Decide the metric names", False),
        ("Emit the counters", True),
        ("Build the dashboard", False),
    ]),
    fixture("Ship the migration", [
        ("Write the up migration", True),
        ("Write the down migration", True),
        ("Test both on a copy", False),
    ]),
]


@dataclass
class Row:
    objective: str
    gold: str | None
    first: str | None
    picked: str | None = None
    correct: bool | None = None
    insertion_was_right: bool = False
    error: str = ""


def reminder_size(items: list[str], unbacked: list[str]) -> int:
    """Port of `build_todo_gate_reminder`'s size."""
    buf = "You have outstanding todos but ended your turn without a tool call.\n\n"
    if unbacked:
        buf += "In-progress (no backing background task):\n" + "".join(f"- {c}\n" for c in unbacked) + "\n"
    if items:
        buf += "Pending:\n" + "".join(f"- {c}\n" for c in items) + "\n"
    buf += ("Per <task_completion_discipline>, advance the next pending todo with the appropriate "
            "tool call NOW. If you have a genuine external blocker (missing credential, denied "
            "permission, network unreachable), state it explicitly AND mark the affected todos "
            "`cancelled` via ${{ tools.by_kind.plan }} with a reason in the same turn.")
    return len(buf)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    def one(case: dict[str, Any]) -> Row:
        row = Row(objective=case["objective"], gold=case["gold"], first=case["items"][0])
        row.insertion_was_right = row.first == row.gold
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                response = client.ask(
                    {"objective": case["objective"], "pending_todos": case["items"]},
                    {"which": primitives.choice(QUESTION + " " + CRITERIA, {t: None for t in case["items"]})},
                )
        except transports.TransportError as error:
            row.error = str(error)
            return row
        row.picked = response.choice("which").choice
        row.correct = row.picked == row.gold
        return row

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        rows = list(pool.map(one, FIXTURES))

    graded = [r for r in rows if r.error == ""]
    table = Table(title=f"todo gate — {len(graded)} blocked-list fixtures")
    table.add_column("medida", justify="left")
    table.add_column("n", justify="right")
    table.add_column("resultado", justify="right")
    table.add_row("ordem de inserção acerta o acionável", str(len(graded)),
                  f"{sum(1 for r in graded if r.insertion_was_right)}/{len(graded)}")
    table.add_row("Jev acerta o acionável", str(len(graded)),
                  f"{sum(1 for r in graded if r.correct)}/{len(graded)}")
    console.print(table)

    print("\n== custo do reminder conforme a lista cresce ==")
    for n in (1, 3, 5, 10, 20):
        items = [f"Item {i} with a reasonably descriptive todo text" for i in range(n)]
        full = reminder_size(items, [])
        head = reminder_size(items[:3], [])
        print(f"  {n:>2} pendentes: dump completo {full:>5} chars | só os 3 primeiros {head:>5} chars")

    print("\n  erros (inserção também erraria = o gate precisa de juízo):")
    for r in graded:
        if not r.correct:
            mark = "inserção também erra" if not r.insertion_was_right else "inserção acertaria"
            print(f"    {mark:<24} objetivo={r.objective[:34]:<36} gold={str(r.gold)[:38]}")
    gained = sum(1 for r in graded if r.correct and not r.insertion_was_right)
    lost = sum(1 for r in graded if not r.correct and r.insertion_was_right)
    print(f"\n  Jev vs inserção: {gained} ganhos, {lost} perdas")

    raw = OUT / "results-todo-gate.json"
    raw.write_text(json.dumps([asdict(r) for r in rows], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"  -> {raw.name}")


if __name__ == "__main__":
    main()