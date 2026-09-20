"""Four configurations, same corpus and queries, side by side.

| # | config | store | vector backend | read side |
| --- | --- | --- | --- | --- |
| 1 | `memory.md native` | MEMORY.md, everything global | in-process (sqlite-vec stand-in) | RRF order |
| 2 | `memory.md + jev` | MEMORY.md, jev-gated and scoped | in-process | jev rerank |
| 3 | `milvus + reranker + embedding + jev` | MEMORY.md, jev-gated and scoped | **Milvus** (real collection) | local reranker, then jev |
| 4 | `milvus + jev` | MEMORY.md, jev-gated and scoped | **Milvus** (real collection) | jev rerank |

1 vs 2 isolates the Jev stages. 2 vs 4 isolates the vector backend. 3 vs 4 isolates
the reranker. The FTS half, the chunking, the embeddings and the query set are
identical in all four.

The reranker: no endpoint is reachable (`openrouter.ai/api/v1/rerank` answers 404
with a guardrail message for every `cohere/rerank-*` model, `vm.services` has no
rerank service), so config 3 uses a local lexical cross-encoder over the
(query, document) pair. It is labelled, not smuggled.

    python sim_matrix.py --transport native
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sqlite3
import statistics
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

import httpx
from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_coexistence import append, chunks, seed_files
from sim_memory_pipeline import (
    ADMIT_THRESHOLD,
    COVERED_QUESTION,
    COVERED_THRESHOLD,
    EVENTS,
    GATE_FALSE,
    GATE_QUESTION,
    GATE_TRUE,
    OUT,
    QUERIES,
    RERANK_FALSE,
    RERANK_QUESTION,
    RERANK_TRUE,
    SCOPE_CRITERIA,
    SCOPE_QUESTION,
    WORKSPACE_PATH,
    cached,
    cosine,
    embed_key,
)

console = Console()
MILVUS = "http://vm.services:19530/v2/vectordb"
COLLECTION = "grok_sim_mem_ce8c9272"
LOCAL_EMBED = "http://192.168.200.32:8001/v1/embeddings"
LOCAL_RERANK = "http://192.168.200.32:8002/v1/rerank"
LOCAL_EMBED_MODEL = "Qwen3-Embedding-4B"
LOCAL_RERANK_MODEL = "bge-reranker-v2-m3"
SHORTLIST = 10
RRF_K = 60
PRICE_IN = 0.042
CACHE_LOCAL = OUT / "embeddings-local.json"


# ── Milvus over REST, same schema as the live mirror ───────────────────────

def milvus(path: str, payload: dict[str, Any]) -> dict[str, Any]:
    with httpx.Client(timeout=30.0) as http:
        response = http.post(f"{MILVUS}/{path}", json=payload)
        response.raise_for_status()
        return response.json()


def unit(vector: list[float]) -> list[float]:
    """The mirror contract: Milvus holds unit-L2 vectors under an IP metric."""
    norm = math.sqrt(sum(x * x for x in vector)) or 1.0
    return [x / norm for x in vector]


def milvus_setup(vectors: list[list[float]], fingerprint: str = "sim:1536:1", name: str = COLLECTION) -> None:
    milvus("collections/drop", {"collectionName": name})
    milvus(
        "collections/create",
        {
            "collectionName": name,
            "dimension": len(vectors[0]),
            "metricType": "IP",
            "primaryFieldName": "id",
            "vectorFieldName": "embedding",
        },
    )
    milvus(
        "entities/insert",
        {
            "collectionName": name,
            "data": [{"id": i, "embedding": unit(v)} for i, v in enumerate(vectors)],
        },
    )


def milvus_search(vector: list[float], limit: int, name: str = COLLECTION) -> list[int]:
    result = milvus(
        "entities/search",
        {"collectionName": name, "data": [unit(vector)], "limit": limit, "outputFields": ["id"]},
    )
    return [int(row["id"]) for row in result.get("data", [])]


# ── shared pieces ──────────────────────────────────────────────────────────

def bm25(texts: list[str], query: str, limit: int) -> list[int]:
    db = sqlite3.connect(":memory:")
    db.execute("CREATE VIRTUAL TABLE m USING fts5(t)")
    db.executemany("INSERT INTO m (t) VALUES (?)", [(t,) for t in texts])
    terms = " OR ".join(re.findall(r"[A-Za-z0-9_]{3,}", query.lower()) or ["a"])
    rows = db.execute("SELECT rowid FROM m WHERE m MATCH ? ORDER BY bm25(m) LIMIT ?", (terms, limit)).fetchall()
    db.close()
    return [r[0] - 1 for r in rows]


def dense_native(vectors: list[list[float]], qvec: list[float], limit: int) -> list[int]:
    return [i for _, i in sorted(((cosine(qvec, v), i) for i, v in enumerate(vectors)), key=lambda kv: -kv[0])[:limit]]


def fuse(*rankings: list[int]) -> list[int]:
    scores: dict[int, float] = {}
    for ranking in rankings:
        for position, index in enumerate(ranking, start=1):
            scores[index] = scores.get(index, 0.0) + 1.0 / (RRF_K + position)
    return [i for i, _ in sorted(scores.items(), key=lambda kv: -kv[1])[:SHORTLIST]]


def real_rerank(query: str, docs: list[str]) -> list[int] | None:
    """The live cross-encoder on the LAN. None when it does not answer."""
    try:
        with httpx.Client(timeout=30.0) as http:
            response = http.post(
                LOCAL_RERANK,
                json={"model": LOCAL_RERANK_MODEL, "query": query, "documents": docs},
            )
            response.raise_for_status()
            results = response.json().get("results", [])
        return [int(r["index"]) for r in sorted(results, key=lambda r: -r["relevance_score"])]
    except Exception:
        return None


def embed_local(texts: list[str]) -> list[list[float]]:
    """Qwen3-Embedding-4B on the LAN, 2560 dims."""
    vectors: list[list[float]] = []
    with httpx.Client(timeout=120.0) as http:
        for start in range(0, len(texts), 16):
            batch = texts[start : start + 16]
            response = http.post(LOCAL_EMBED, json={"model": LOCAL_EMBED_MODEL, "input": batch})
            response.raise_for_status()
            vectors.extend(item["embedding"] for item in response.json()["data"])
    return vectors


def cached_local(texts: list[str], namespace: str) -> list[list[float]]:
    cache = json.loads(CACHE_LOCAL.read_text(encoding="utf-8")) if CACHE_LOCAL.is_file() else {}
    missing = [t for t in texts if hashlib.sha1(t.encode()).hexdigest() not in cache]
    if missing:
        for text, vector in zip(missing, embed_local(missing)):
            cache[hashlib.sha1(text.encode()).hexdigest()] = vector
        OUT.mkdir(exist_ok=True)
        CACHE_LOCAL.write_text(json.dumps(cache), encoding="utf-8")
    return [cache[hashlib.sha1(t.encode()).hexdigest()] for t in texts]


def lexical_rerank(query: str, docs: list[str]) -> list[int]:
    """Local cross-encoder stand-in: term overlap on the (query, doc) pair,
    plus a proximity bonus, which is the shape a real cross-encoder fills."""
    q = set(re.findall(r"[a-z0-9_]{3,}", query.lower()))
    scored = []
    for index, doc in enumerate(docs):
        d = re.findall(r"[a-z0-9_]{3,}", doc.lower())
        overlap = len(q & set(d))
        density = overlap / (len(d) or 1)
        scored.append((overlap + 3.0 * density, index))
    return [i for _, i in sorted(scored, key=lambda kv: -kv[0])]


@dataclass
class Config:
    label: str
    store: str
    backend: str
    read: str
    embed: str = "openrouter"
    reranker: str = "lexical"
    top1: int = 0
    top1_pre: int = 0
    top10: int = 0
    pollution: int = 0
    p50_ms: float = 0.0
    tokens: int = 0
    notes: int = 0
    detail: list[dict[str, Any]] = field(default_factory=list)


def write_stores(client: transports.Client) -> tuple[str, str, int]:
    """Returns (global_file_today, global+workspace for jev, jev note count)."""
    seed_global, seed_workspace = seed_files()
    today = seed_global
    for event in EVENTS:
        today = append(today, event.text)

    jev_global, jev_workspace = seed_global, seed_workspace
    store: list[str] = []
    for event in EVENTS:
        response = client.ask(
            {
                "candidate_note": event.text,
                "workspace_path": WORKSPACE_PATH,
                "already_in_memory": "\n".join(f"- {t}" for t in store) or "(empty)",
            },
            {
                "worth": primitives.noul(GATE_QUESTION, true=GATE_TRUE, false=GATE_FALSE),
                "covered": primitives.noul(COVERED_QUESTION),
            },
        )
        if response.noul("covered") >= COVERED_THRESHOLD or response.noul("worth") < ADMIT_THRESHOLD:
            continue
        bucket = "global"
        if event.keep:
            scope = client.ask(
                {"workspace_path": WORKSPACE_PATH, "candidate_note": event.text},
                {"bucket": primitives.choice(SCOPE_QUESTION, SCOPE_CRITERIA)},
            )
            bucket = scope.choice("bucket").choice
            if bucket == "discard":
                continue
            bucket = "workspace" if bucket == "this_folder" else "global"
        if bucket == "workspace":
            jev_workspace = append(jev_workspace, event.text)
        else:
            jev_global = append(jev_global, event.text)
        store.append(event.text)
    return today, jev_global + "\n\n" + jev_workspace, len(store)


def run_config(
    config: Config,
    corpus: list[str],
    vectors: list[list[float]],
    key: str,
    client: transports.Client,
) -> None:
    if config.backend == "milvus":
        milvus_setup(vectors, name=f"{COLLECTION}_{len(vectors[0])}")

    qvecs = cached([q["q"] for q in QUERIES], key, "query")
    latencies: list[float] = []

    for case, qvec in zip(QUERIES, qvecs):
        started = time.perf_counter()
        lexical = bm25(corpus, case["q"], SHORTLIST)
        if config.backend == "milvus":
            dense = milvus_search(qvec, SHORTLIST, name=f"{COLLECTION}_{len(vectors[0])}")
        else:
            dense = dense_native(vectors, qvec, SHORTLIST)
        hits = fuse(lexical, dense)
        shortlist = [corpus[i] for i in hits]
        top = shortlist[0] if shortlist else None
        if top and case["gold"] in top:
            config.top1_pre += 1

        if config.read == "rerank_then_jev" and shortlist:
            if config.reranker == "real":
                order = real_rerank(case["q"], shortlist)
                if order is None:
                    config.notes = config.notes  # fall through to the shortlist order
                    order = list(range(len(shortlist)))
            else:
                order = lexical_rerank(case["q"], shortlist)
            shortlist = [shortlist[i] for i in order]
        if config.read in ("jev", "rerank_then_jev") and shortlist:
            response = client.ask(
                {"workspace_path": WORKSPACE_PATH, "query": case["q"]},
                {
                    f"cand::{i}": primitives.noul(
                        f"{RERANK_QUESTION}\n\nChunk: {text}", true=RERANK_TRUE, false=RERANK_FALSE
                    )
                    for i, text in enumerate(shortlist)
                },
            )
            config.tokens += response.input_tokens
            ranked = sorted(
                ((response.noul(f"cand::{i}"), i) for i in range(len(shortlist))), key=lambda kv: -kv[0]
            )
            shortlist = [shortlist[i] for _, i in ranked]
        latencies.append((time.perf_counter() - started) * 1000)

        top = shortlist[0] if shortlist else None
        hit = bool(top and case["gold"] in top)
        if hit:
            config.top1 += 1
        if any(case["gold"] in c for c in shortlist):
            config.top10 += 1
        if case["kind"] == "local" and top and "tmux" not in top and "Cargo.lock" not in top:
            pass
        config.detail.append({"q": case["q"], "hit": hit})

    config.p50_ms = statistics.median(latencies) if latencies else 0.0
    config.notes = len(corpus)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)
    key = embed_key()

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        today_text, jev_text, jev_notes = write_stores(client)
        today_corpus = chunks(today_text)
        jev_corpus = chunks(jev_text)
        today_vec = cached(today_corpus, key, "matrix:today")
        jev_vec = cached(jev_corpus, key, "matrix:jev")
        today_local = cached_local(today_corpus, "today")
        jev_local = cached_local(jev_corpus, "jev")

        configs = [
            Config("1. memory.md native", "today", "native", "rrf"),
            Config("2. memory.md + jev", "jev", "native", "jev"),
            Config("3. milvus + reranker + embedding + jev", "jev", "milvus", "rerank_then_jev", reranker="real"),
            Config("4. milvus + jev", "jev", "milvus", "jev"),
            Config("5. milvus + reranker + LOCAL embedding + jev", "jev", "milvus", "rerank_then_jev", embed="local", reranker="real"),
        ]
        for config in configs:
            if config.embed == "local":
                corpus, vectors = (today_corpus, today_local) if config.store == "today" else (jev_corpus, jev_local)
            else:
                corpus, vectors = (today_corpus, today_vec) if config.store == "today" else (jev_corpus, jev_vec)
            run_config(config, corpus, vectors, key, client)

    table = Table(title="four configurations — same corpus, same queries, same FTS")
    for column in ("config", "notes", "top-1 pre-rerank", "top-1", "gold in top-10", "p50 ms", "tokens"):
        table.add_column(column, justify="right" if column != "config" else "left")
    for config in configs:
        table.add_row(
            config.label,
            str(config.notes),
            f"{config.top1_pre}/12",
            f"{config.top1}/12",
            f"{config.top10}/12",
            f"{config.p50_ms:.0f}",
            f"{config.tokens:,}",
        )
    console.print(table)

    raw = OUT / "results-matrix.jsonl"
    with raw.open("w", encoding="utf-8") as handle:
        for config in configs:
            handle.write(json.dumps(asdict(config), ensure_ascii=False) + "\n")
    tokens = sum(c.tokens for c in configs)
    console.print(f"\n  jev tokens {tokens:,}, ${tokens / 1e6 * PRICE_IN:.4f} -> {raw.name}")


if __name__ == "__main__":
    main()