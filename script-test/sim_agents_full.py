"""The complete agent space: retrieval × reranker × Jev × delegation graph.

Same shape as the skill factorial, plus the axis the agent corpus uniquely has:
108 agents declare a `## Delegates` map that nothing reads, and wiring it as a
second-stage narrow scored 88% against 80% flat in isolation.

  retrieve  native   FTS5/BM25 alone
            milvus   dense KNN served by Milvus
            hybrid   FTS + dense + weighted RRF
  reranker  none     vs  bge-reranker-v2-m3 on the LAN
  jev       off      vs  on (gate, then one choice over the shortlist)
  graph     off      vs  on (choice an entry agent, then choose among it and its
                          declared delegates)

24 arms, 25 delegation cases. Only solaris models.

    python sim_agents_full.py --transport native
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_embedding import cached_local
from sim_matrix import COLLECTION, milvus_search, milvus_setup, real_rerank
from sim_vs_stack import bm25_rank, dense_rank, rrf

ROOT = Path(__file__).parent
AGENTS = Path("~/.grok/agents").expanduser()
OUT = ROOT / "out"
console = Console()
SHORTLIST = 10
PRICE_IN = 0.042

QUESTION = (
    "Which subagent should handle this task, given it is being delegated from a parent session?"
)
ENTRY_QUESTION = (
    "Which agent is the best entry point for this task? Its own declared delegates will be "
    "considered next."
)
GATE_QUESTION = (
    "Does this task need a specialised agent, or is a general-purpose one enough?"
)
BUILTINS = [
    {"name": "general-purpose", "description": "General purpose agent for multi-step tasks; has access to all tools."},
    {"name": "explore", "description": "Fast read-only agent specialized for codebase exploration."},
    {"name": "plan", "description": "Software architect for planning implementation strategies; read-only."},
]


def delegates(name: str) -> list[str]:
    path = AGENTS / f"{name}.md"
    if not path.is_file():
        return []
    text = path.read_text(encoding="utf-8", errors="replace")
    match = re.search(r"^## Delegates\s*\n+([^\n#]+)", text, re.MULTILINE)
    return re.findall(r"->\s*([a-z0-9-]+)", match.group(1)) if match else []


@dataclass
class Arm:
    retrieve: str
    rerank: str
    jev: str
    graph: str
    n: int = 0
    top1: int = 0
    pre_top1: int = 0
    gold_in_pool: int = 0
    p50_ms: float = 0.0
    tokens: int = 0
    detail: list[dict[str, Any]] = field(default_factory=list)

    @property
    def label(self) -> str:
        return f"{self.retrieve}/{self.rerank}/jev={self.jev}/graph={self.graph}"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    roster = json.loads((ROOT / "cases" / "agents.json").read_text(encoding="utf-8")) + BUILTINS
    index = {a["name"]: a["description"][:400] for a in roster}
    names = [a["name"] for a in roster]
    docs = [f"{a['name']}: {a['description']}" for a in roster]
    graph = {a["name"]: [d for d in delegates(a["name"]) if d in index] for a in roster}
    cases = json.loads((ROOT / "cases" / "agent_cases.json").read_text(encoding="utf-8"))
    queries = [c["request"] for c in cases]
    vectors = cached_local(docs)
    qvecs = cached_local(queries)
    milvus_setup(vectors, name=f"{COLLECTION}_agents")
    collection = f"{COLLECTION}_agents"

    arms = [
        Arm(r, k, j, g)
        for r in ("native", "milvus", "hybrid")
        for k in ("none", "real")
        for j in ("off", "on")
        for g in ("off", "on")
    ]

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            latencies = []
            for case, qvec in zip(cases, qvecs):
                started = time.perf_counter()
                lexical = bm25_rank(roster, case["request"], SHORTLIST)
                if arm.retrieve == "milvus":
                    pool = [names[i] for i in milvus_search(qvec, SHORTLIST, name=collection)]
                elif arm.retrieve == "hybrid":
                    dense = dense_rank(roster, vectors, qvec, SHORTLIST)
                    pool = rrf(lexical, dense)[:SHORTLIST]
                else:
                    pool = lexical[:SHORTLIST]

                if pool and pool[0] == case["gold"]:
                    arm.pre_top1 += 1
                if case["gold"] in pool:
                    arm.gold_in_pool += 1

                final: str | None = pool[0] if pool else None
                if arm.rerank == "real" and pool:
                    order = real_rerank(case["request"], [index[n] for n in pool])
                    if order:
                        pool = [pool[i] for i in order]
                        final = pool[0]

                if arm.jev == "on":
                    gate = client.ask({"task": case["request"]}, {"gate": primitives.noul(GATE_QUESTION)})
                    arm.tokens += gate.input_tokens
                    if gate.noul("gate") >= 0.40 and len(pool) >= 2:
                        if arm.graph == "on":
                            entry = client.ask(
                                {"task": case["request"]},
                                {"which": primitives.choice(ENTRY_QUESTION, {n: index[n] for n in pool})},
                            )
                            arm.tokens += entry.input_tokens
                            picked_entry = entry.choice("which").choice
                            cands = [picked_entry] + [d for d in graph.get(picked_entry, []) if d != picked_entry]
                            if len(cands) >= 2:
                                second = client.ask(
                                    {"task": case["request"]},
                                    {"which": primitives.choice(
                                        "Given this entry point, which agent should do the work?",
                                        {n: index[n] for n in cands})},
                                )
                                arm.tokens += second.input_tokens
                                final = second.choice("which").choice
                            else:
                                final = picked_entry
                        else:
                            pick = client.ask(
                                {"task": case["request"]},
                                {"which": primitives.choice(QUESTION, {n: index[n] for n in pool})},
                            )
                            arm.tokens += pick.input_tokens
                            final = pick.choice("which").choice
                    elif gate.noul("gate") < 0.40:
                        final = None

                latencies.append((time.perf_counter() - started) * 1000)
                if final == case["gold"]:
                    arm.top1 += 1
                arm.detail.append({"gold": case["gold"], "final": final})

            arm.n = len(cases)
            arm.p50_ms = statistics.median(latencies)

    table = Table(title=f"complete agent space — {len(cases)} cases, 24 arms")
    for column in ("#", "retrieve", "rerank", "jev", "graph", "tipo certo", "gold no pool", "p50 ms", "tokens"):
        table.add_column(column, justify="right" if column not in ("retrieve", "rerank", "jev", "graph") else "left")
    for i, arm in enumerate(sorted(arms, key=lambda a: (-a.top1, -a.gold_in_pool)), start=1):
        table.add_row(str(i), arm.retrieve, arm.rerank, arm.jev, arm.graph,
                      f"{arm.top1}/{arm.n}", f"{arm.gold_in_pool}/{arm.n}", f"{arm.p50_ms:.0f}", f"{arm.tokens:,}")
    console.print(table)

    best = max(arms, key=lambda a: a.top1)
    console.print(f"\n  melhor: {best.label} -> {best.top1}/{best.n}")
    for axis in ("retrieve", "rerank", "jev", "graph"):
        groups: dict[str, list[int]] = {}
        for arm in arms:
            groups.setdefault(getattr(arm, axis), []).append(arm.top1)
        means = {k: statistics.mean(v) for k, v in groups.items()}
        console.print(f"  efeito de {axis}: " + "  ".join(f"{k}={v:.1f}" for k, v in sorted(means.items())))

    raw = OUT / "results-agents-full.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()