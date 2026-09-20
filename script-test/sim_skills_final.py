"""Every recommendation in FINDINGS-SKILLS.md, stacked, ablated and scored.

Isolated measurements say each change helps on its own. That is not the same as
saying the stack helps, because the changes interact. This runs:

  today       the shipped shape: 400-byte index, no hint, no variants, a
              three-noul gate at 0.30, body re-read, no cleaning
  recommended all of the doc's advice at once: 60-byte cleaned index,
              specificity hint, variants routing, single-noul gate at 0.40,
              no body re-read
  then one ablation per recommendation, removing exactly one thing from the
  recommended stack, so the contribution is visible on top of everything else
  rather than in isolation.

Retrieval is fixed at hybrid + solaris embedding for every arm; one extra arm
runs the whole thing on the native lexical path to show the fail-open floor.

    python sim_skills_final.py --transport native
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
from sim_skills_shape import clean
from sim_skills_variants import variants_of
from sim_vs_stack import bm25_rank, dense_rank, rrf

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
SHORTLIST = 10
PRICE_IN = 0.042

SELECT_QUESTION = "Which of these skills is the right one to load for this request?"
HINT = (
    " When a broad umbrella skill and a specific one both cover the request, prefer the "
    "specific one."
)
VARIANT_QUESTION = (
    "This request matches the topic area, but the topic has several implementation variants. "
    "Which variant is right for this specific request?"
)
GATE_QUESTION = (
    "Would a careful expert answering this request consult a specific documented procedure or "
    "set of commands, rather than answering from general knowledge alone?"
)
GATE_AUX = [
    "Is the assistant being asked to act on files, accounts, devices or online services?",
    "Is this a mechanical lookup rather than a judgement call?",
]


@dataclass
class Arm:
    label: str
    width: int = 60
    cleaning: bool = True
    hint: bool = True
    variants: bool = True
    single_gate: bool = True
    body_reread: bool = False
    retrieve: str = "hybrid"
    n: int = 0
    top1: int = 0
    top3: int = 0
    mrr: float = 0.0
    silent_right: int = 0
    false_silence: int = 0
    p50_ms: float = 0.0
    tokens: int = 0
    detail: list[dict[str, Any]] = field(default_factory=list)


def build_arms() -> list[Arm]:
    recommended = Arm("recommended (all of the doc)")
    today = Arm(
        "today (shipped shape)",
        width=400, cleaning=False, hint=False, variants=False,
        single_gate=False, body_reread=True,
    )
    return [
        today,
        recommended,
        Arm("ablation: index at 400", width=400),
        Arm("ablation: no cleaning", cleaning=False),
        Arm("ablation: no hint", hint=False),
        Arm("ablation: no variants routing", variants=False),
        Arm("ablation: three-noul gate", single_gate=False),
        Arm("ablation: body re-read on", body_reread=True),
        Arm("fail-open: native lexical", retrieve="native"),
    ]


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
    variants = {s["name"]: variants_of(s["name"]) for s in corpus}
    bodies = {s["name"]: s["description"] for s in corpus}

    arms = build_arms()

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            index = {
                s["name"]: (clean(s["description"]) if arm.cleaning else s["description"])[: arm.width]
                for s in corpus
            }
            latencies = []
            for case, qvec in zip(cases, qvecs):
                started = time.perf_counter()
                lexical = bm25_rank(corpus, case["request"], SHORTLIST)
                if arm.retrieve == "hybrid":
                    dense = dense_rank(corpus, vectors, qvec, SHORTLIST)
                    shortlist = rrf(lexical, dense)[:SHORTLIST]
                else:
                    shortlist = lexical[:SHORTLIST]

                # gate
                if arm.single_gate:
                    gate = client.ask(
                        {"request": case["request"]},
                        {"gate": primitives.noul(GATE_QUESTION)},
                    )
                    arm.tokens += gate.input_tokens
                    proceed = gate.noul("gate") >= 0.40
                else:
                    questions = {"g0": primitives.noul(GATE_QUESTION)}
                    for i, aux in enumerate(GATE_AUX):
                        questions[f"g{i+1}"] = primitives.noul(aux)
                    gate = client.ask({"request": case["request"]}, questions)
                    arm.tokens += gate.input_tokens
                    values = [gate.noul(k) for k in questions]
                    proceed = (sum(values) / len(values)) >= 0.30

                final: str | None = None
                if proceed and len(shortlist) >= 2:
                    response = client.ask(
                        {"request": case["request"]},
                        {"which": primitives.choice(SELECT_QUESTION + (HINT if arm.hint else ""),
                                                    {n: index[n] for n in shortlist})},
                    )
                    arm.tokens += response.input_tokens
                    picked = response.choice("which").choice
                    shortlist = [picked] + [n for n in shortlist if n != picked]

                    if arm.body_reread:
                        top3 = shortlist[:3]
                        again = client.ask(
                            {"request": case["request"]},
                            {"which": primitives.choice(
                                SELECT_QUESTION + " Read what each actually does, not just its name.",
                                {n: bodies[n] for n in top3})},
                        )
                        arm.tokens += again.input_tokens
                        picked = again.choice("which").choice
                        shortlist = [picked] + [n for n in shortlist if n != picked]

                    if arm.variants and variants.get(picked):
                        options = [v for v in variants[picked] if v in index]
                        if len(options) >= 2:
                            second = client.ask(
                                {"request": case["request"]},
                                {"which": primitives.choice(VARIANT_QUESTION, {v: index[v] for v in options})},
                            )
                            arm.tokens += second.input_tokens
                            refined = second.choice("which").choice
                            shortlist = [refined] + [n for n in shortlist if n != refined]
                    final = shortlist[0]

                latencies.append((time.perf_counter() - started) * 1000)
                hit = final == case["gold"]
                if hit:
                    arm.top1 += 1
                if case["gold"] and final is None:
                    arm.false_silence += 1
                if case["gold"] is None and final is None:
                    arm.silent_right += 1
                if case["gold"] and case["gold"] in shortlist[:3]:
                    arm.top3 += 1
                if case["gold"] and case["gold"] in shortlist:
                    arm.mrr += 1.0 / (shortlist.index(case["gold"]) + 1)
                arm.detail.append({"gold": case["gold"], "final": final})

            arm.n = len(cases)
            arm.mrr /= len(cases)
            arm.p50_ms = statistics.median(latencies)

    table = Table(title=f"recommendation stack and ablation — {len(cases)} queries")
    for column in ("arm", "top-1", "top-3", "MRR", "false silence", "silent right", "p50 ms", "tokens"):
        table.add_column(column, justify="right" if column != "arm" else "left")
    for arm in arms:
        table.add_row(
            arm.label, f"{arm.top1}/{arm.n}", f"{arm.top3}/{arm.n}", f"{arm.mrr:.3f}",
            str(arm.false_silence), f"{arm.silent_right}/12", f"{arm.p50_ms:.0f}", f"{arm.tokens:,}",
        )
    console.print(table)

    raw = OUT / "results-skills-final.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    tokens = sum(a.tokens for a in arms)
    console.print(f"\n  tokens {tokens:,}, ${tokens / 1e6 * PRICE_IN:.4f} -> {raw.name}")


if __name__ == "__main__":
    main()