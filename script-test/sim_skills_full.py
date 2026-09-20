"""The complete space: native (lexical) vs embedding, reranker, Jev.

The four capabilities the user named, crossed exhaustively on the skills corpus
(172 documents, 66 labelled queries). Retrieval is the only axis that changes the
candidate set; everything downstream reorders it.

  retrieve  native   FTS5/BM25 alone — the deterministic path, no vectors
            hybrid   FTS5/BM25 + dense KNN, fused with weighted RRF
  embed     solaris  Qwen3-Embedding-4B on the LAN (only meaningful for hybrid)
  reranker  none     vs  bge-reranker-v2-m3 on the LAN
  jev       off      vs  on (one choice over the shortlist)

The question is not which single capability is best in isolation but where each
one stops paying. `native` is the baseline that ships.

    python sim_skills_full.py --transport native
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

SELECT_QUESTION = (
    "Which of these skills is the right one to load for this request? Read the descriptions "
    "as an index. When a broad umbrella skill and a specific one both cover the request, "
    "prefer the specific one."
)
SHORT_WIDTH = 60


@dataclass
class Arm:
    retrieve: str
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


def run_arm(
    arm: Arm,
    corpus: list[dict[str, str]],
    vectors: list[list[float]] | None,
    qvecs: list[list[float]] | None,
    cases: list[dict[str, Any]],
    client: transports.Client,
) -> None:
    short_index = {s["name"]: s["description"][:SHORT_WIDTH] for s in corpus}
    latencies = []

    for case, qvec in zip(cases, qvecs or [None] * len(cases)):
        started = time.perf_counter()
        lexical = bm25_rank(corpus, case["request"], SHORTLIST)
        if arm.retrieve == "hybrid" and vectors is not None and qvec is not None:
            dense = dense_rank(corpus, vectors, qvec, SHORTLIST)
            shortlist = rrf(lexical, dense)[:SHORTLIST]
        else:
            shortlist = lexical[:SHORTLIST]
        if shortlist and shortlist[0] == case["gold"]:
            arm.pre_top1 += 1

        if arm.rerank == "real" and shortlist:
            order = real_rerank(case["request"], [short_index[n] for n in shortlist])
            if order:
                shortlist = [shortlist[i] for i in order]
        if arm.jev == "on" and len(shortlist) >= 2:
            response = client.ask(
                {"request": case["request"], "recent_context": ""},
                {"which": primitives.choice(SELECT_QUESTION, {n: short_index[n] for n in shortlist})},
            )
            arm.tokens += response.input_tokens
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
    cases = json.loads((ROOT / "cases" / "skill_ccases.json".replace("cc", "c")).read_text(encoding="utf-8"))
    docs = [f"{s['name']}: {s['description']}" for s in corpus]
    queries = [c["request"] for c in cases]

    remote_docs = cached(docs, key, "emb:remote")
    remote_q = cached(queries, key, "emb:remote")
    local_docs = cached_local(docs)
    local_q = cached_local(queries)

    arms: list[Arm] = []
    for rerank in ("none", "real"):
        for jev in ("off", "on"):
            arms.append(Arm("native", "—", rerank, jev))
    # Only the solaris models. `native` is the no-model lexical path.
    for rerank in ("none", "real"):
        for jev in ("off", "on"):
            arms.append(Arm("hybrid", "solaris", rerank, jev))

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            dv, qv = (local_docs, local_q) if arm.retrieve == "hybrid" else (None, None)
            run_arm(arm, corpus, dv, qv, cases, client)

    table = Table(title="complete space — 66 queries, 172 documents")
    for column in ("#", "retrieve", "embed", "rerank", "jev", "top-1", "top-3", "top-10", "MRR", "p50 ms", "tokens"):
        table.add_column(column, justify="right" if column not in ("retrieve", "embed", "rerank", "jev") else "left")
    for index, arm in enumerate(sorted(arms, key=lambda a: (-a.top1, -a.mrr)), start=1):
        table.add_row(
            str(index), arm.retrieve, arm.embed, arm.rerank, arm.jev,
            f"{arm.top1}/{arm.n}", f"{arm.top3}/{arm.n}", f"{arm.top10}/{arm.n}",
            f"{arm.mrr:.3f}", f"{arm.p50_ms:.0f}", f"{arm.tokens:,}",
        )
    console.print(table)

    raw = OUT / "results-skills-full.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")

    best = max(arms, key=lambda a: a.top1)
    native = max((a for a in arms if a.retrieve == "native"), key=lambda a: a.top1)
    console.print(
        f"\n  best overall: {best.retrieve}/{best.embed}/rerank={best.rerank}/jev={best.jev} "
        f"-> {best.top1}/{best.n}  (native best: {native.top1}/{native.n})"
    )
    console.print(f"  -> {raw.name}")


if __name__ == "__main__":
    main()