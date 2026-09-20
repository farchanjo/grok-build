"""Can the corpus's own delegation graph drive the selection?

108 of 109 agents carry a `## Delegates` map and nothing reads it. If it is any
good, it is a routing structure authored by whoever wrote each agent — strictly
more information than a flat choice over 109 descriptions.

Two arms:

  flat    pick the agent out of the full roster
  graph   pick an entry agent, then pick again among that agent and its declared
          delegates

The graph arm is only meaningful where the entry agent has delegates, so the
run reports how often that is and scores the two arms on the same cases.

    python sim_agents_graph.py --transport native
"""

from __future__ import annotations

import argparse
import json
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path

from rich.console import Console
from rich.table import Table

from jev import primitives, transports

ROOT = Path(__file__).parent
AGENTS = Path("~/.grok/agents").expanduser()
OUT = ROOT / "out"
console = Console()
WORKERS = 8

QUESTION = (
    "Which subagent should handle this task, given it is being delegated from a parent session?"
)


def delegates(name: str) -> list[str]:
    path = AGENTS / f"{name}.md"
    if not path.is_file():
        return []
    import re
    text = path.read_text(encoding="utf-8", errors="replace")
    match = re.search(r"^## Delegates\s*\n+([^\n#]+)", text, re.MULTILINE)
    return re.findall(r"->\s*([a-z0-9-]+)", match.group(1)) if match else []


@dataclass
class Row:
    case_id: str
    gold: str
    arm: str
    entry: str | None = None
    picked: str | None = None
    correct: bool | None = None
    had_delegates: bool = False
    error: str = ""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    roster = json.loads((ROOT / "cases" / "agents.json").read_text(encoding="utf-8"))
    index = {a["name"]: a["description"][:200] for a in roster}
    cases = json.loads((ROOT / "cases" / "agent_cases.json").read_text(encoding="utf-8"))
    graph = {a["name"]: [d for d in delegates(a["name"]) if d in index] for a in roster}

    def one(index_: int, arm: str) -> Row:
        case = cases[index_]
        row = Row(case_id=case["request"][:44], gold=case["gold"], arm=arm)
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                if arm == "flat":
                    response = client.ask(
                        {"task": case["request"]},
                        {"which": primitives.choice(QUESTION, index)},
                    )
                    row.picked = response.choice("which").choice
                else:
                    entry = client.ask(
                        {"task": case["request"]},
                        {"which": primitives.choice(
                            "Which agent is the best entry point for this task? Its own delegates will "
                            "be considered next.", index)},
                    )
                    row.entry = entry.choice("which").choice
                    candidates = [row.entry] + [d for d in graph.get(row.entry, []) if d != row.entry]
                    row.had_delegates = len(candidates) > 1
                    if len(candidates) >= 2:
                        second = client.ask(
                            {"task": case["request"]},
                            {"which": primitives.choice(
                                "Given this entry point, which agent should actually do the work?",
                                {n: index[n] for n in candidates})},
                        )
                        row.picked = second.choice("which").choice
                    else:
                        row.picked = row.entry
        except transports.TransportError as error:
            row.error = str(error)
            return row
        row.correct = row.picked == case["gold"]
        return row

    rows: list[Row] = []
    for arm in ("flat", "graph"):
        with ThreadPoolExecutor(max_workers=WORKERS) as pool:
            rows += list(pool.map(lambda i, a=arm: one(i, a), range(len(cases))))

    table = Table(title=f"delegation graph as a router — {len(cases)} cases")
    for column in ("arm", "tipo certo", "entrada com delegates", "quanto o grafo estreitou"):
        table.add_column(column, justify="right" if column != "arm" else "left")
    for arm in ("flat", "graph"):
        subset = [r for r in rows if r.arm == arm and r.error == ""]
        ok = sum(1 for r in subset if r.correct)
        with_del = sum(1 for r in subset if r.had_delegates)
        avg = sum(len(graph.get(r.entry or "", [])) for r in subset) / max(len(subset), 1)
        table.add_row(
            arm,
            f"{ok}/{len(subset)} ({ok / len(subset):.1%})",
            f"{with_del}/{len(subset)}",
            f"{avg:.1f} candidatos em média" if arm == "graph" else "109",
        )
    console.print(table)

    graph_rows = [r for r in rows if r.arm == "graph" and r.had_delegates and r.error == ""]
    if graph_rows:
        ok = sum(1 for r in graph_rows if r.correct)
        console.print(f"\n  só onde o grafo agiu ({len(graph_rows)} casos): {ok}/{len(graph_rows)} = {ok / len(graph_rows):.0%}")
        helped = sum(1 for r in graph_rows if r.correct and r.entry != r.gold)
        console.print(f"  casos em que a segunda pergunta corrigiu a entrada: {helped}")

    raw = OUT / "results-agents-graph.json"
    raw.write_text(json.dumps([asdict(r) for r in rows], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()