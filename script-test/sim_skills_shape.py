"""Does the description shape help or hurt a semantic chooser?

The shipped descriptions are engineered for a keyword matcher: they carry inline
`TRIGGER: ...` and `SKIP: ... (use X instead)` sections, plus a `metadata.triggers`
keyword list and, for umbrella skills, a `variants:` declaration. That is the
right shape for `matches_query` (literal substring) and the wrong shape for
anything reading meaning.

Three treatments, same retrieval, same queries, same chooser:

  raw       the description as shipped
  cleaned   TRIGGER/SKIP/metadata scaffolding stripped
  raw60     raw truncated to 60 bytes — the width the width-sweep liked
  clean60   cleaned truncated to 60 bytes

raw60 against clean60 isolates content from length.

    python sim_skills_shape.py --transport native
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
from sim_memory_pipeline import embed_key
from sim_vs_stack import bm25_rank, dense_rank, rrf

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
SHORTLIST = 10
WIDTH = 60

QUESTION = (
    "Which of these skills is the right one to load for this request? When a broad umbrella "
    "skill and a specific one both cover the request, prefer the specific one."
)

SCAFFOLD = re.compile(
    r"(TRIGGER[S]?:.*?)(?=SKIP:|$)|(SKIP:.*?)$", re.IGNORECASE | re.DOTALL
)


def clean(description: str) -> str:
    """Drop the keyword scaffolding, keep the prose."""
    text = description
    for marker in ("TRIGGER:", "TRIGGERS:", "SKIP:"):
        index = text.find(marker)
        if index != -1:
            text = text[:index]
    return " ".join(text.split())


@dataclass
class Arm:
    treatment: str
    n: int = 0
    top1: int = 0
    top3: int = 0
    mrr: float = 0.0
    p50_ms: float = 0.0
    tokens: int = 0
    detail: list[dict[str, Any]] = field(default_factory=list)


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
    vectors = cached_local(docs)
    qvecs = cached_local(queries)

    views = {
        "raw": {s["name"]: s["description"][:WIDTH] for s in corpus},
        "cleaned": {s["name"]: clean(s["description"])[:WIDTH] for s in corpus},
        "raw60": {s["name"]: s["description"][:WIDTH] for s in corpus},
        "clean60": {s["name"]: clean(s["description"])[:WIDTH] for s in corpus},
    }
    views["raw"] = {s["name"]: s["description"] for s in corpus}
    views["cleaned"] = {s["name"]: clean(s["description"]) for s in corpus}

    arms = [Arm(name) for name in ("raw", "cleaned", "raw60", "clean60")]

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            index = views[arm.treatment]
            latencies = []
            for case, qvec in zip(cases, qvecs):
                started = time.perf_counter()
                lexical = bm25_rank(corpus, case["request"], SHORTLIST)
                dense = dense_rank(corpus, vectors, qvec, SHORTLIST)
                shortlist = rrf(lexical, dense)[:SHORTLIST]
                if len(shortlist) >= 2:
                    response = client.ask(
                        {"request": case["request"]},
                        {"which": primitives.choice(QUESTION, {n: index[n] for n in shortlist})},
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
                        arm.mrr += 1.0 / (shortlist.index(case["gold"]) + 1)
            arm.n = len(cases)
            arm.mrr /= len(cases)
            arm.p50_ms = statistics.median(latencies)

    table = Table(title=f"description shape — {len(cases)} queries, hybrid retrieval, Jev chooses")
    for column in ("treatment", "top-1", "top-3", "MRR", "p50 ms", "tokens", "chars por skill"):
        table.add_column(column, justify="right" if column != "treatment" else "left")
    for arm in sorted(arms, key=lambda a: -a.top1):
        avg = statistics.mean(len(v) for v in views[arm.treatment].values())
        table.add_row(arm.treatment, f"{arm.top1}/{arm.n}", f"{arm.top3}/{arm.n}", f"{arm.mrr:.3f}",
                      f"{arm.p50_ms:.0f}", f"{arm.tokens:,}", f"{avg:.0f}")
    console.print(table)

    sample = corpus[0]["name"]
    console.print(f"\n  amostra crua  ({sample}): {views['raw'][sample][:150]}")
    console.print(f"  amostra limpa ({sample}): {views['cleaned'][sample][:150]}")

    raw = OUT / "results-skills-shape.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()