"""Head-to-head: the retrieval stack that exists vs Jev, same corpus, same queries.

What exists, read from the dev profile (`~/.grokdev/config.toml`):

* `[embedding_models.prime-local]` — `openai/text-embedding-3-small`, 1536 dims,
  served through the `prime-embeddings` provider.
* `[retrieval_profiles.prime-main]` — that embedding model, **`reranker_models = []`**,
  `max_candidates = 50`, `max_results = 10`, RRF-style deterministic fallback.
* `[prime.skills] enabled = true`, `max_results = 3` — so this pipeline is live in
  the dev profile.
* `[vector_stores.milvus-vm]` reachable at `vm.services:19530`.

The real selector is `prime/skills.rs`: deterministic evidence, then FTS + KNN over
the indexed inventory, then weighted RRF over the fused shortlist. This reproduces
that core — FTS5/BM25 and dense KNN over the same metadata, fused with RRF — and
puts three things next to it:

* **A** the stack as it runs today (BM25 + dense + RRF), no reranker attached
* **B** Jev ranking the roster in one call
* **C** A's shortlist, reranked by Jev — i.e. Jev in the reranker slot the profile
  leaves empty

Nothing is removed in any arm; C is strictly A plus a rerank.

    python sim_vs_stack.py --transport native
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import sqlite3
import statistics
import struct
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

import httpx
from rich.console import Console
from rich.table import Table

from jev import primitives, transports

ROOT = Path(__file__).parent
OUT = ROOT / "out"
CACHE = OUT / "embeddings.json"
PRICE_IN = 0.042
WORKERS = 8
RRF_K = 60
SHORTLIST = 10
JEV_QUESTION = (
    "Which of these skills is the right one to load for this request? Read the descriptions "
    "as an index. When a broad umbrella skill and a specific skill both cover the request, "
    "prefer the specific one."
)
EMBED_MODEL = "openai/text-embedding-3-small"
EMBED_URL = "https://openrouter.ai/api/v1/embeddings"

console = Console()


def embed_key() -> str:
    for path in ("~/.grokdev/auth.json", "~/.grok/auth.json"):
        p = Path(path).expanduser()
        if not p.is_file():
            continue
        store = json.loads(p.read_text(encoding="utf-8"))
        for scope in ("openai_compatible::prime-embeddings::api_key", "openrouter::api_key"):
            value = store.get(scope)
            if isinstance(value, dict) and value.get("key"):
                return value["key"]
            if isinstance(value, str) and value:
                return value
    raise RuntimeError("no embedding credential found")


def embed(texts: list[str], key: str) -> list[list[float]]:
    with httpx.Client(timeout=120.0) as http:
        response = http.post(
            EMBED_URL,
            headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
            json={"model": EMBED_MODEL, "input": texts},
        )
        response.raise_for_status()
        payload = response.json()
    return [item["embedding"] for item in payload["data"]]


def cached_embeddings(texts: list[str], key: str, namespace: str) -> list[list[float]]:
    cache: dict[str, list[float]] = {}
    if CACHE.is_file():
        cache = json.loads(CACHE.read_text(encoding="utf-8"))
    todo = [t for t in texts if f"{namespace}:{hashlib.sha1(t.encode()).hexdigest()}" not in cache]
    if todo:
        vectors = embed(todo, key)
        for text, vector in zip(todo, vectors):
            cache[f"{namespace}:{hashlib.sha1(text.encode()).hexdigest()}"] = vector
        OUT.mkdir(exist_ok=True)
        CACHE.write_text(json.dumps(cache), encoding="utf-8")
    return [cache[f"{namespace}:{hashlib.sha1(t.encode()).hexdigest()}"] for t in texts]


def cosine(a: list[float], b: list[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a)) or 1.0
    nb = math.sqrt(sum(y * y for y in b)) or 1.0
    return dot / (na * nb)


def bm25_rank(corpus: list[dict[str, str]], query: str, limit: int) -> list[str]:
    """FTS5 over the same metadata the real index holds."""
    db = sqlite3.connect(":memory:")
    db.execute("CREATE VIRTUAL TABLE skills USING fts5(name, description)")
    db.executemany(
        "INSERT INTO skills (name, description) VALUES (?, ?)",
        [(s["name"], s["description"]) for s in corpus],
    )
    terms = " OR ".join(re.findall(r"[A-Za-z0-9_]{3,}", query.lower()) or ["a"])
    rows = db.execute(
        "SELECT name FROM skills WHERE skills MATCH ? ORDER BY bm25(skills) LIMIT ?",
        (terms, limit),
    ).fetchall()
    db.close()
    return [r[0] for r in rows]


def dense_rank(corpus: list[dict[str, str]], vectors: list[list[float]], qvec: list[float], limit: int) -> list[str]:
    scored = [(cosine(qvec, v), c["name"]) for c, v in zip(corpus, vectors)]
    return [name for _, name in sorted(scored, key=lambda kv: -kv[0])[:limit]]


def rrf(*rankings: list[str], weights: tuple[float, ...] = (1.0, 1.0)) -> list[str]:
    """Weighted reciprocal rank fusion — the deterministic fusion the profile names."""
    scores: dict[str, float] = {}
    for ranking, weight in zip(rankings, weights):
        for position, name in enumerate(ranking, start=1):
            scores[name] = scores.get(name, 0.0) + weight / (RRF_K + position)
    return [name for name, _ in sorted(scores.items(), key=lambda kv: -kv[1])]


@dataclass
class Row:
    case_id: str
    gold: str | None
    a_top1: str = ""
    a_shortlist: list[str] = field(default_factory=list)
    b_top1: str | None = None
    c_top1: str | None = None
    a_ok: bool = False
    b_ok: bool = False
    c_ok: bool = False
    a_in_shortlist: bool = False
    latency_b: float = 0.0
    latency_c: float = 0.0
    tokens: int = 0
    error: str = ""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    corpus = json.loads((ROOT / "cases" / "roster.json").read_text(encoding="utf-8"))
    cases = json.loads((ROOT / "cases" / "skill_cases.json").read_text(encoding="utf-8"))
    short_index = {s["name"]: s["description"][:60] for s in corpus}

    key = embed_key()
    console.print(f"embedding {len(corpus)} skills + {len(cases)} queries via {EMBED_MODEL}")
    started = time.time()
    corpus_vectors = cached_embeddings([s["description"] for s in corpus], key, "corpus")
    query_vectors = cached_embeddings([c["request"] for c in cases], key, "query")
    console.print(f"embeddings ready in {time.time() - started:.0f}s")

    # ── arm A: the stack as it runs today ──────────────────────────────────
    rows: list[Row] = []
    for case, qvec in zip(cases, query_vectors):
        row = Row(case_id=case["request"][:44], gold=case["gold"])
        lexical = bm25_rank(corpus, case["request"], SHORTLIST)
        dense = dense_rank(corpus, corpus_vectors, qvec, SHORTLIST)
        fused = rrf(lexical, dense)[:SHORTLIST]
        row.a_shortlist = fused
        row.a_top1 = fused[0] if fused else ""
        row.a_ok = bool(row.gold) and row.a_top1 == row.gold
        row.a_in_shortlist = bool(row.gold) and row.gold in fused
        rows.append(row)

    # ── arms B and C: Jev ──────────────────────────────────────────────────
    def jev(case: dict[str, Any], row: Row, shortlist: list[str]) -> None:
        state = {"request": case["request"], "recent_context": ""}
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                if shortlist:
                    response = client.ask(
                        state, {"which": primitives.choice(JEV_QUESTION, {n: short_index[n] for n in shortlist})}
                    )
                    row.c_top1 = response.choice("which").choice
                    row.latency_c = response.latency_ms
                    row.tokens += response.input_tokens
        except transports.TransportError as error:
            row.error = str(error)

    wide_questions = {"which": primitives.choice(JEV_QUESTION, short_index)}

    def wide(case: dict[str, Any], row: Row) -> None:
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                response = client.ask({"request": case["request"], "recent_context": ""}, wide_questions)
            row.b_top1 = response.choice("which").choice
            row.latency_b = response.latency_ms
            row.tokens += response.input_tokens
        except transports.TransportError as error:
            row.error = str(error)

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        list(pool.map(lambda i: wide(cases[i], rows[i]), range(len(cases))))
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        list(pool.map(lambda i: jev(cases[i], rows[i], rows[i].a_shortlist), range(len(cases))))

    for row in rows:
        row.b_ok = bool(row.gold) and row.b_top1 == row.gold
        row.c_ok = bool(row.gold) and row.c_top1 == row.gold

    covered = [r for r in rows if r.gold]
    uncovered = [r for r in rows if not r.gold]

    console.rule("what we have vs Jev — 54 covered, 12 uncovered")
    table = Table()
    for column in ("arm", "top-1", "gold in top-10", "uncovered handled", "p50 ms", "tokens"):
        table.add_column(column, justify="right" if column != "arm" else "left")
    table.add_row(
        "A: BM25 + dense + RRF (today)",
        f"{sum(1 for r in covered if r.a_ok) / len(covered):.1%}",
        f"{sum(1 for r in covered if r.a_in_shortlist) / len(covered):.1%}",
        "-",
        "<1",
        "0",
    )
    table.add_row(
        "B: Jev wide",
        f"{sum(1 for r in covered if r.b_ok) / len(covered):.1%}",
        "-",
        f"{sum(1 for r in uncovered if r.b_top1)}/{len(uncovered)}",
        f"{statistics.median([r.latency_b for r in rows if r.latency_b]):.0f}",
        f"{sum(r.tokens for r in rows):,}",
    )
    table.add_row(
        "C: A's shortlist, Jev reranks",
        f"{sum(1 for r in covered if r.c_ok) / len(covered):.1%}",
        f"{sum(1 for r in covered if r.a_in_shortlist) / len(covered):.1%}",
        f"{sum(1 for r in uncovered if r.c_top1)}/{len(uncovered)}",
        f"{statistics.median([r.latency_c for r in rows if r.latency_c]):.0f}",
        f"{sum(r.tokens for r in rows):,}",
    )
    console.print(table)

    recovered = sum(1 for r in covered if not r.a_ok and r.c_ok)
    broken = sum(1 for r in covered if r.a_ok and not r.c_ok)
    console.print(f"\n  C against A: [green]{recovered} recovered[/green], [red]{broken} broken[/red]")
    console.print(f"  ceiling for C (gold in A's shortlist): {sum(1 for r in covered if r.a_in_shortlist)}/{len(covered)}")

    raw = OUT / f"results-vs-stack-{args.transport}.jsonl"
    with raw.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(asdict(row), ensure_ascii=False) + "\n")
    console.print(f"  -> {raw.relative_to(ROOT)}")


if __name__ == "__main__":
    main()