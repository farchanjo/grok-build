"""Embedding head-to-head, and the requested configuration.

Part 1 isolates the embedding model: dense-only retrieval, no FTS, no fusion, no
reranker, no Jev, on the skills corpus (172 documents, 66 labelled queries) where
the sample is large enough to say something.

Part 2 runs the requested configuration — milvus + real cross-encoder + LOCAL
embedding — on the memory corpus, with and without the Jev stages, so the Jev
contribution stays separable.

    python sim_embedding.py --transport native
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import statistics
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

import httpx
from rich.console import Console
from rich.table import Table

from jev import transports
from sim_matrix import (
    COLLECTION,
    LOCAL_EMBED,
    LOCAL_EMBED_MODEL,
    LOCAL_RERANK,
    LOCAL_RERANK_MODEL,
    milvus,
    milvus_search,
    milvus_setup,
    real_rerank,
    unit,
)
from sim_memory_pipeline import cached, cosine, embed_key

ROOT = Path(__file__).parent
OUT = ROOT / "out"
CACHE_LOCAL = OUT / "embeddings-local.json"
console = Console()

OPENROUTER_EMBED = "https://openrouter.ai/api/v1/embeddings"
OPENROUTER_MODEL = "openai/text-embedding-3-small"


def embed_local(texts: list[str]) -> list[list[float]]:
    vectors: list[list[float]] = []
    with httpx.Client(timeout=180.0) as http:
        for start in range(0, len(texts), 16):
            batch = texts[start : start + 16]
            response = http.post(LOCAL_EMBED, json={"model": LOCAL_EMBED_MODEL, "input": batch})
            response.raise_for_status()
            vectors.extend(item["embedding"] for item in response.json()["data"])
    return vectors


def cached_local(texts: list[str]) -> list[list[float]]:
    cache = json.loads(CACHE_LOCAL.read_text(encoding="utf-8")) if CACHE_LOCAL.is_file() else {}
    missing = [t for t in texts if hashlib.sha1(t.encode()).hexdigest() not in cache]
    if missing:
        for text, vector in zip(missing, embed_local(missing)):
            cache[hashlib.sha1(text.encode()).hexdigest()] = vector
        OUT.mkdir(exist_ok=True)
        CACHE_LOCAL.write_text(json.dumps(cache), encoding="utf-8")
    return [cache[hashlib.sha1(t.encode()).hexdigest()] for t in texts]


def recall_at(vectors: list[list[float]], qvecs: list[list[float]], golds: list[int], k: int) -> int:
    hits = 0
    for qvec, gold in zip(qvecs, golds):
        ranked = sorted(((cosine(qvec, v), i) for i, v in enumerate(vectors)), key=lambda kv: -kv[0])[:k]
        if any(i == gold for _, i in ranked):
            hits += 1
    return hits


@dataclass
class Result:
    label: str
    n: int = 0
    r1: int = 0
    r5: int = 0
    r10: int = 0
    top1: int = 0
    top10: int = 0
    p50_ms: float = 0.0
    tokens: int = 0
    detail: list[dict[str, Any]] = field(default_factory=list)


def part1_embedding(key: str) -> list[Result]:
    roster = json.loads((ROOT / "cases" / "roster.json").read_text(encoding="utf-8"))
    cases = json.loads((ROOT / "cases" / "skill_cases.json").read_text(encoding="utf-8"))
    docs = [f"{s['name']}: {s['description']}" for s in roster]
    index_of = {s["name"]: i for i, s in enumerate(roster)}
    queries = [c["request"] for c in cases]
    golds = [index_of.get(c["gold"], -1) for c in cases]

    started = time.perf_counter()
    remote_docs = cached(docs, key, "emb:remote")
    remote_q = cached(queries, key, "emb:remote")
    remote_secs = time.perf_counter() - started

    started = time.perf_counter()
    local_docs = cached_local(docs)
    local_q = cached_local(queries)
    local_secs = time.perf_counter() - started

    results = []
    for label, dv, qv, secs, dims in (
        ("openai/text-embedding-3-small (OpenRouter)", remote_docs, remote_q, remote_secs, len(remote_docs[0])),
        ("Qwen3-Embedding-4B (local, 192.168.200.32:8001)", local_docs, local_q, local_secs, len(local_docs[0])),
    ):
        result = Result(label, n=len(cases), r1=recall_at(dv, qv, golds, 1), r5=recall_at(dv, qv, golds, 5), r10=recall_at(dv, qv, golds, 10))
        result.detail.append({"dims": dims, "embed_secs": round(secs, 2)})
        results.append(result)
    return results


def part2_config(transport: str) -> list[Result]:
    from sim_matrix import write_stores
    from sim_memory_pipeline import QUERIES, WORKSPACE_PATH, RERANK_FALSE, RERANK_QUESTION, RERANK_TRUE

    OUT.mkdir(exist_ok=True)
    key = embed_key()
    results: list[Result] = []

    with transports.Client.build(transport, timeout_s=60.0) as client:
        _, jev_text, _ = write_stores(client)
        from sim_coexistence import chunks

        corpus = chunks(jev_text)
        vectors = cached_local(corpus)
        qvecs = cached_local([q["q"] for q in QUERIES])

        milvus_setup(vectors, name=f"{COLLECTION}_{len(vectors[0])}_local")
        name = f"{COLLECTION}_{len(vectors[0])}_local"

        for label, use_rerank, use_jev in (
            ("milvus + LOCAL embedding (no reranker, no jev)", False, False),
            ("milvus + reranker + LOCAL embedding", True, False),
            ("milvus + reranker + LOCAL embedding + jev", True, True),
        ):
            result = Result(label, n=len(QUERIES))
            latencies = []
            for case, qvec in zip(QUERIES, qvecs):
                started = time.perf_counter()
                dense = milvus_search(qvec, 10, name=name)
                shortlist = [corpus[i] for i in dense]
                if use_rerank:
                    order = real_rerank(case["q"], shortlist) or list(range(len(shortlist)))
                    shortlist = [shortlist[i] for i in order]
                if use_jev:
                    response = client.ask(
                        {"workspace_path": WORKSPACE_PATH, "query": case["q"]},
                        {
                            f"cand::{i}": __import__("jev").primitives.noul(
                                f"{RERANK_QUESTION}\n\nChunk: {t}", true=RERANK_TRUE, false=RERANK_FALSE
                            )
                            for i, t in enumerate(shortlist)
                        },
                    )
                    result.tokens += response.input_tokens
                    order = sorted(
                        ((response.noul(f"cand::{i}"), i) for i in range(len(shortlist))),
                        key=lambda kv: -kv[0],
                    )
                    shortlist = [shortlist[i] for _, i in order]
                latencies.append((time.perf_counter() - started) * 1000)
                if shortlist and case["gold"] in shortlist[0]:
                    result.top1 += 1
                if any(case["gold"] in c for c in shortlist):
                    result.top10 += 1
            result.p50_ms = statistics.median(latencies)
            results.append(result)
    return results


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    key = embed_key()

    console.rule("part 1 — embedding head-to-head (skills corpus, 172 docs, 66 queries)")
    part1 = part1_embedding(key)
    table = Table()
    for column in ("model", "dims", "recall@1", "recall@5", "recall@10", "embed secs"):
        table.add_column(column, justify="right" if column != "model" else "left")
    for result in part1:
        meta = result.detail[0]
        table.add_row(
            result.label,
            str(meta["dims"]),
            f"{result.r1}/{result.n}",
            f"{result.r5}/{result.n}",
            f"{result.r10}/{result.n}",
            f"{meta['embed_secs']}",
        )
    console.print(table)

    console.rule("part 2 — the requested configuration (memory corpus, 12 queries)")
    part2 = part2_config(args.transport)
    table = Table()
    for column in ("config", "top-1", "gold in top-10", "p50 ms", "tokens"):
        table.add_column(column, justify="right" if column != "config" else "left")
    for result in part2:
        table.add_row(result.label, f"{result.top1}/12", f"{result.top10}/12", f"{result.p50_ms:.0f}", f"{result.tokens:,}")
    console.print(table)

    raw = OUT / "results-embedding.json"
    raw.write_text(
        json.dumps({"part1": [asdict(r) for r in part1], "part2": [asdict(r) for r in part2]}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()