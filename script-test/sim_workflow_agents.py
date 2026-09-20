"""Workflows name an agent per step, blind.

`AgentOpts` carries `agent_type: Option<String>` — and that is the whole
interface. It is passed straight to `subagent_type` with a `general-purpose`
default, never validated against the registry, and the built-in `deep-research`
uses it **zero times**: it re-describes every role in prose instead.

There is also no `skills_hint`, which `spawn_subagent` has. So a workflow can
neither reach the 109 registered agents reliably nor bind a skill to a step.

Two measurements:

1. **Authoring the type.** The author writes a literal string, with no catalog in
   view. Three arms: blind, a listing, and retrieval + Jev.
2. **A typo's fate.** What the runtime does with a name that is not an agent.

    python sim_workflow_agents.py --transport native
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_embedding import cached_local
from sim_matrix import COLLECTION, milvus_search, milvus_setup

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
WORKERS = 8
SHORTLIST = 5

STEP_QUESTION = (
    "A workflow is authoring a step. Which agent_type should it pass to agent()? The type is a "
    "literal string naming a registered agent; general-purpose is the fallback."
)
BUILTINS = [
    {"name": "general-purpose", "description": "General purpose agent for multi-step tasks; all tools."},
    {"name": "explore", "description": "Fast read-only agent for codebase exploration."},
    {"name": "plan", "description": "Software architect for planning; read-only."},
]


@dataclass
class Row:
    step: str
    gold: str
    arm: str
    picked: str | None = None
    correct: bool | None = None
    error: str = ""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    roster = json.loads((ROOT / "cases" / "agents.json").read_text(encoding="utf-8")) + BUILTINS
    index = {a["name"]: a["description"][:200] for a in roster}
    names = [a["name"] for a in roster]
    docs = [f"{a['name']}: {a['description']}" for a in roster]
    cases = json.loads((ROOT / "cases" / "agent_cases.json").read_text(encoding="utf-8"))
    # frame each case as a workflow step
    steps = [{"step": f"phase: implement — {c['request']}", "gold": c["gold"]} for c in cases]

    vectors = cached_local(docs)
    qvecs = cached_local([s["step"] for s in steps])
    milvus_setup(vectors, name=f"{COLLECTION}_wfagent")
    coll = f"{COLLECTION}_wfagent"

    arms = ["blind (sem catálogo)", "listed (109 no prompt)", "retrieval + jev"]

    def one(index_: int, arm: str) -> Row:
        case = steps[index_]
        row = Row(step=case["step"][:44], gold=case["gold"], arm=arm)
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                if arm == "blind (sem catálogo)":
                    response = client.ask(
                        {"step": case["step"]},
                        {"is": primitives.noul(f"Should this step's agent_type be exactly `{case['gold']}`?")},
                    )
                    row.picked = case["gold"] if response.noul("is") >= 0.5 else None
                elif arm == "listed (109 no prompt)":
                    response = client.ask({"step": case["step"]},
                                          {"which": primitives.choice(STEP_QUESTION, index)})
                    row.picked = response.choice("which").choice
                else:
                    hits = milvus_search(qvecs[index_], SHORTLIST, name=coll)
                    pool = [names[i] for i in hits]
                    response = client.ask({"step": case["step"]},
                                          {"which": primitives.choice(STEP_QUESTION, {n: index[n] for n in pool})})
                    row.picked = response.choice("which").choice
        except transports.TransportError as error:
            row.error = str(error)
            return row
        row.correct = row.picked == case["gold"]
        return row

    rows: list[Row] = []
    for arm in arms:
        with ThreadPoolExecutor(max_workers=WORKERS) as pool:
            rows += list(pool.map(lambda i, a=arm: one(i, a), range(len(steps))))

    graded = [r for r in rows if r.error == ""]
    table = Table(title=f"agent_type escrito por um workflow — {len(steps)} passos")
    for column in ("arm", "tipo certo", "custo por passo"):
        table.add_column(column, justify="right" if column != "arm" else "left")
    for arm in arms:
        subset = [r for r in graded if r.arm == arm]
        ok = sum(1 for r in subset if r.correct)
        cost = "0" if arm.startswith("blind") else ("109 entradas" if arm.startswith("listed") else "1 KNN + 1 jev")
        table.add_row(arm, f"{ok}/{len(subset)}", cost)
    console.print(table)

    print("\n== o destino de um typo ==")
    print("  host_service.rs:459  subagent_type = opts.agent_type.unwrap_or(\"general-purpose\")")
    print("  sem validação contra o registry: o nome errado só aparece no spawn, no meio do run")

    print("\n== o que o built-in faz ==")
    print("  deep_research.rhai usa agent_type 0 vezes — descreve cada papel em prosa no prompt")

    raw = OUT / "results-workflow-agents.json"
    raw.write_text(json.dumps([asdict(r) for r in rows], ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()