"""Intelligent skill injection: which mechanism decides what goes in the context.

Selection accuracy is not the whole story once the chosen skill is *injected* —
its body goes into the context and stays there. So the metric that matters is
precision at a fixed budget: inject the right skill, inject few, and inject
nothing when nothing is needed.

Six injectors, same queries, same 3-skill budget:

  native top3          FTS5/BM25 top three, always three
  milvus top3          dense KNN served by Milvus, always three
  hybrid top3          FTS + dense + RRF top three, always three
  jev gate + choice    gate first, then one choice; injects 1 or 0
  hybrid + jev         hybrid shortlist, gate, then one choice
  hybrid + jev scored  hybrid shortlist, one noul per candidate, inject those over
                       threshold, up to three

The last three are the "intelligent" shapes: the first three cannot decline.

    python sim_skills_inject.py --transport native
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

import httpx
from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_embedding import cached_local
from sim_matrix import COLLECTION, milvus, milvus_search, milvus_setup
from sim_memory_pipeline import embed_key
from sim_skills_shape import clean
from sim_skills_variants import variants_of
from sim_vs_stack import bm25_rank, dense_rank, rrf

ROOT = Path(__file__).parent
OUT = ROOT / "out"
SKILLS = Path("~/.grok/skills").expanduser()
console = Console()
SHORTLIST = 10
BUDGET = 3
BODY_CAP = 2000
WIDTH = 200
PRICE_IN = 0.042

GATE_QUESTION = (
    "Would a careful expert answering this request consult a specific documented procedure or "
    "set of commands, rather than answering from general knowledge alone?"
)
SELECT_QUESTION = (
    "Which of these skills is the right one to load for this request? When a broad umbrella "
    "skill and a specific one both cover the request, prefer the specific one."
)
FIT_QUESTION = "Does this skill apply to this request?"


def body_len(name: str) -> int:
    path = SKILLS / name / "SKILL.md"
    return len(path.read_text(encoding="utf-8", errors="replace")) if path.is_file() else 0


@dataclass
class Arm:
    label: str
    n: int = 0
    injected_total: int = 0
    gold_hit: int = 0
    gold_top1: int = 0
    harmful: int = 0
    declined_right: int = 0
    declined_wrong: int = 0
    injected_tokens: int = 0
    p50_ms: float = 0.0
    detail: list[dict[str, Any]] = field(default_factory=list)

    @property
    def precision(self) -> float:
        return self.gold_hit / self.injected_total if self.injected_total else 0.0

    @property
    def tokens_per_turn(self) -> int:
        return self.injected_tokens // max(self.n, 1)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)
    key = embed_key()

    corpus = json.loads((ROOT / "cases" / "roster.json").read_text(encoding="utf-8"))
    cases = json.loads((ROOT / "cases" / "skill_cases.json").read_text(encoding="utf-8"))
    index = {s["name"]: clean(s["description"])[:WIDTH] for s in corpus}
    bodies = {s["name"]: body_len(s["name"]) for s in corpus}
    variants = {s["name"]: variants_of(s["name"]) for s in corpus}
    docs = [f"{s['name']}: {s['description']}" for s in corpus]
    queries = [c["request"] for c in cases]
    vectors = cached_local(docs)
    qvecs = cached_local(queries)
    milvus_setup(vectors, name=f"{COLLECTION}_inject")

    def inject_tokens(names: list[str]) -> int:
        return sum(min(bodies.get(n, 0), BODY_CAP) for n in names) // 4

    arms = [
        Arm("native top3"),
        Arm("milvus top3"),
        Arm("hybrid top3"),
        Arm("jev gate + choice"),
        Arm("hybrid + jev"),
        Arm("hybrid + jev scored"),
    ]

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            latencies = []
            for case, qvec in zip(cases, qvecs):
                started = time.perf_counter()
                lexical = bm25_rank(corpus, case["request"], SHORTLIST)
                dense_idx = dense_rank(corpus, vectors, qvec, SHORTLIST)
                dense_milvus = [corpus[i]["name"] for i in milvus_search(qvec, SHORTLIST, name=f"{COLLECTION}_inject")]
                hybrid = rrf(lexical, dense_idx)[:SHORTLIST]

                picked: list[str] = []
                if arm.label == "native top3":
                    picked = lexical[:BUDGET]
                elif arm.label == "milvus top3":
                    picked = dense_milvus[:BUDGET]
                elif arm.label == "hybrid top3":
                    picked = hybrid[:BUDGET]
                else:
                    if arm.label == "jev gate + choice":
                        pool = None
                    elif arm.label == "hybrid + jev":
                        pool = hybrid
                    else:
                        pool = hybrid
                    gate = client.ask({"request": case["request"]}, {"gate": primitives.noul(GATE_QUESTION)})
                    arm.injected_tokens += 0
                    passed = gate.noul("gate") >= 0.40
                    if passed:
                        if pool is None:
                            response = client.ask(
                                {"request": case["request"]},
                                {"which": primitives.choice(SELECT_QUESTION, index)},
                            )
                            chosen = response.choice("which").choice
                            picked = [chosen]
                        elif arm.label == "hybrid + jev":
                            response = client.ask(
                                {"request": case["request"]},
                                {"which": primitives.choice(SELECT_QUESTION, {n: index[n] for n in pool})},
                            )
                            chosen = response.choice("which").choice
                            picked = [chosen]
                        else:
                            questions = {f"s::{n}": primitives.noul(f"{FIT_QUESTION} Skill `{n}`: {index[n]}") for n in pool}
                            response = client.ask({"request": case["request"]}, questions)
                            scored = sorted(((response.noul(k), k[3:]) for k in questions), reverse=True)
                            picked = [n for v, n in scored if v >= 0.40][:BUDGET]
                    if picked and variants.get(picked[0]):
                        options = [v for v in variants[picked[0]] if v in index]
                        if len(options) >= 2:
                            second = client.ask(
                                {"request": case["request"]},
                                {"which": primitives.choice(
                                    "This request matches the topic area; which variant is right?",
                                    {v: index[v] for v in options})},
                            )
                            picked = [second.choice("which").choice] + picked[1:]

                latencies.append((time.perf_counter() - started) * 1000)
                arm.injected_total += len(picked)
                if case["gold"]:
                    if case["gold"] in picked:
                        arm.gold_hit += 1
                    if picked and picked[0] == case["gold"]:
                        arm.gold_top1 += 1
                    if not picked:
                        arm.declined_wrong += 1
                else:
                    if picked:
                        arm.harmful += 1
                    else:
                        arm.declined_right += 1
                arm.injected_tokens += inject_tokens(picked)
                arm.detail.append({"gold": case["gold"], "picked": picked})

            arm.n = len(cases)
            arm.p50_ms = statistics.median(latencies)

    table = Table(title=f"injection mechanisms — {len(cases)} queries, budget 3 skills")
    for column in ("injector", "injeta/consulta", "precisão", "gold injetado", "gold em 1º", "injeção danosa (12 sem skill)", "corpo injetado tok/turno", "p50 ms"):
        table.add_column(column, justify="right" if column != "injector" else "left")
    for arm in arms:
        table.add_row(
            arm.label,
            f"{arm.injected_total / arm.n:.2f}",
            f"{arm.precision:.1%}",
            f"{arm.gold_hit}/54",
            f"{arm.gold_top1}/54",
            f"{arm.harmful}/12",
            f"{arm.tokens_per_turn:,}",
            f"{arm.p50_ms:.0f}",
        )
    console.print(table)

    raw = OUT / "results-skills-inject.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()