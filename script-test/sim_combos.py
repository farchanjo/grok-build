"""Factorial: every combination of embedding x reranker x jev, one corpus, one query set.

The question is which combination is best, and that needs all arms measured on
the same data with enough queries to separate them. The skills corpus is the only
one with a usable sample (172 documents, 66 labelled queries), so the factorial
runs there.

Axes:
  embedding  remote (openai/text-embedding-3-small)  vs  local (Qwen3-Embedding-4B)
  reranker   none                                    vs  bge-reranker-v2-m3 (real, on the LAN)
  jev        off                                     vs  on (rerank over the shortlist)

Retrieval itself never changes: FTS5 + dense + weighted RRF, shortlist of 10.

    python sim_combos.py --transport native
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_embedding import cached_local
from sim_matrix import real_rerank
from sim_memory_pipeline import cached, embed_key
from sim_vs_stack import bm25_rank, dense_rank, rrf

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
SHORTLIST = 10
PRICE_IN = 0.042

JEV_QUESTION = (
    "Which of these skills is the right one to load for this request? Read the descriptions "
    "as an index. When a broad umbrella skill and a specific skill both cover the request, "
    "prefer the specific one."
)


@dataclass
class Arm:
    embed: str
    rerank: str
    jev: str
    n: int = 0
    top1: int = 0
    top3: int = 0
    top10: int = 0
    pre_top1: int = 0
    mrr: float = 0.0
    p50_ms: float = 0.0
    tokens: int = 0
    detail: list[dict[str, Any]] = field(default_factory=list)

    @property
    def label(self) -> str:
        return f"embed={self.embed} rerank={self.rerank} jev={self.jev}"


def run_arm(
    arm: Arm,
    corpus: list[dict[str, str]],
    vectors: list[list[float]],
    qvecs: list[list[float]],
    cases: list[dict[str, Any]],
    client: transports.Client,
) -> None:
    short_index = {s["name"]: s["description"][:60] for s in corpus}
    names = [s["name"] for s in corpus]
    latencies = []

    for case, qvec in zip(cases, qvecs):
        started = time.perf_counter()
        lexical = bm25_rank(corpus, case["request"], SHORTLIST)
        dense = dense_rank(corpus, vectors, qvec, SHORTLIST)
        fused = rrf(lexical, dense)[:SHORTLIST]
        if fused and fused[0] == case["gold"]:
            arm.pre_top1 += 1

        shortlist = fused
        if arm.rerank == "real" and shortlist:
            order = real_rerank(case["request"], [short_index[n] for n in shortlist])
            if order:
                shortlist = [shortlist[i] for i in order]
        if arm.jev == "on" and shortlist:
            response = client.ask(
                {"request": case["request"], "recent_context": ""},
                {"which": primitives.choice(JEV_QUESTION, {n: short_index[n] for n in shortlist})},
            )
            arm.tokens += response.input_tokens
            # the choice returns one pick; keep the rest of the order behind it
            picked = response.choice("which").choice
            shortlist = [picked] + [n for n in shortlist if n != picked]

        latencies.append((time.perf_counter() - started) * 1000)
        if shortlist:
            if shortlist[0] == case["gold"]:
                arm.top1 += 1
            if case["gold"] in shortlist[:3]:
                arm.top3 += 1
            if case["gold"] in shortlist:
                arm.top10 += 1
            if case["gold"] in shortlist:
                arm.mrr += 1.0 / (shortlist.index(case["gold"]) + 1)

    arm.n = len(cases)
    arm.mrr /= len(cases)
    arm.p50_ms = statistics.median(latencies)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)
    key = embed_key()

    corpus = json.loads((ROOT / "cases" / "roster.json").read_text(encoding="utf-8"))
    cases = json.loads((ROOT / "cases" / "skill_cases.json").read_text(encoding="utf-8"))
    docs = [f"{s['name']}: {s['description']}" for s in corpus]
    queries = [c["request"] for c in cases]

    remote_docs = cached(docs, key, "emb:remote")
    remote_q = cached(queries, key, "emb:remote")
    local_docs = cached_local(docs)
    local_q = cached_local(queries)

    arms = [
        Arm(e, r, j)
        for e in ("remote", "local")
        for r in ("none", "real")
        for j in ("off", "on")
    ]

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            vectors, qvecs = (remote_docs, remote_q) if arm.embed == "remote" else (local_docs, local_q)
            run_arm(arm, corpus, vectors, qvecs, cases, client)

    table = Table(title="8 combinations — 66 queries, 172 documents, identical retrieval")
    for column in ("#", "embedding", "reranker", "jev", "top-1", "top-3", "top-10", "MRR", "p50 ms", "tokens"):
        table.add_column(column, justify="right" if column not in ("embedding", "reranker", "jev") else "left")
    for index, arm in enumerate(sorted(arms, key=lambda a: (-a.top1, -a.mrr)), start=1):
        table.add_row(
            str(index),
            arm.embed,
            arm.rerank,
            arm.jev,
            f"{arm.top1}/{arm.n}",
            f"{arm.top3}/{arm.n}",
            f"{arm.top10}/{arm.n}",
            f"{arm.mrr:.3f}",
            f"{arm.p50_ms:.0f}",
            f"{arm.tokens:,}",
        )
    console.print(table)

    raw = OUT / "results-combos.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()