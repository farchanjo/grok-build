"""Width as a cost decision, with both cost components counted.

The stacked ablation said width is not an accuracy lever once a shortlist
exists. It is still a cost lever, and there are two costs, not one:

* **advertisement** — what the model reads every turn: 172 skills x width chars,
  present whether or not prime fires. Computed, not measured.
* **selection** — what the choice call carries: 10 shortlist candidates x width.
  Measured by the harness.

The first dominates by 17x. Any width decision has to be read against it.

Stack held fixed at the corrected shape: single-noul gate at 0.40, variants
routing, no body re-read, hybrid retrieval with the solaris embedding.

    python sim_skills_width.py --transport native
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
from sim_memory_pipeline import embed_key
from sim_skills_shape import clean
from sim_skills_variants import variants_of
from sim_vs_stack import bm25_rank, dense_rank, rrf

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
SHORTLIST = 10
ROSTER = 172
PRICE_IN = 0.042

SELECT_QUESTION = "Which of these skills is the right one to load for this request?"
VARIANT_QUESTION = (
    "This request matches the topic area, but the topic has several implementation variants. "
    "Which variant is right for this specific request?"
)
GATE_QUESTION = (
    "Would a careful expert answering this request consult a specific documented procedure or "
    "set of commands, rather than answering from general knowledge alone?"
)


@dataclass
class Arm:
    width: int
    cleaned: bool = True
    n: int = 0
    top1: int = 0
    top3: int = 0
    mrr: float = 0.0
    false_silence: int = 0
    selection_tokens: int = 0
    p50_ms: float = 0.0
    detail: list[dict[str, Any]] = field(default_factory=list)

    @property
    def label(self) -> str:
        return f"width {self.width}{'' if self.cleaned else ' raw'}"

    @property
    def advertisement_tokens(self) -> int:
        """What the listing costs every turn, at 4 bytes per token."""
        return ROSTER * self.width // 4

    @property
    def turn_tokens(self) -> int:
        """Advertisement per turn plus the selection calls amortised over the run."""
        return self.advertisement_tokens + (self.selection_tokens // max(self.n, 1))


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

    arms = [Arm(w) for w in (60, 100, 150, 200, 300, 400, 700)]
    arms.append(Arm(400, cleaned=False))
    arms.append(Arm(60, cleaned=False))

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            index = {
                s["name"]: (clean(s["description"]) if arm.cleaned else s["description"])[: arm.width]
                for s in corpus
            }
            latencies = []
            for case, qvec in zip(cases, qvecs):
                started = time.perf_counter()
                lexical = bm25_rank(corpus, case["request"], SHORTLIST)
                dense = dense_rank(corpus, vectors, qvec, SHORTLIST)
                shortlist = rrf(lexical, dense)[:SHORTLIST]

                gate = client.ask(
                    {"request": case["request"]}, {"gate": primitives.noul(GATE_QUESTION)}
                )
                arm.selection_tokens += gate.input_tokens
                final: str | None = None
                if gate.noul("gate") >= 0.40 and len(shortlist) >= 2:
                    response = client.ask(
                        {"request": case["request"]},
                        {"which": primitives.choice(SELECT_QUESTION, {n: index[n] for n in shortlist})},
                    )
                    arm.selection_tokens += response.input_tokens
                    picked = response.choice("which").choice
                    shortlist = [picked] + [n for n in shortlist if n != picked]
                    if variants.get(picked):
                        options = [v for v in variants[picked] if v in index]
                        if len(options) >= 2:
                            second = client.ask(
                                {"request": case["request"]},
                                {"which": primitives.choice(VARIANT_QUESTION, {v: index[v] for v in options})},
                            )
                            arm.selection_tokens += second.input_tokens
                            refined = second.choice("which").choice
                            shortlist = [refined] + [n for n in shortlist if n != refined]
                    final = shortlist[0]

                latencies.append((time.perf_counter() - started) * 1000)
                if final == case["gold"]:
                    arm.top1 += 1
                if case["gold"] and final is None:
                    arm.false_silence += 1
                if case["gold"] and case["gold"] in shortlist[:3]:
                    arm.top3 += 1
                if case["gold"] and case["gold"] in shortlist:
                    arm.mrr += 1.0 / (shortlist.index(case["gold"]) + 1)
                arm.detail.append({"gold": case["gold"], "final": final})

            arm.n = len(cases)
            arm.mrr /= len(cases)
            arm.p50_ms = statistics.median(latencies)

    table = Table(title=f"width sweep, corrected stack — {len(cases)} queries")
    for column in ("width", "cleaned", "top-1", "top-3", "MRR", "anúncio tok/turno", "seleção tok/turno", "custo/turno $", "p50 ms"):
        table.add_column(column, justify="right" if column != "width" else "left")
    for arm in sorted(arms, key=lambda a: a.width):
        table.add_row(
            str(arm.width), "sim" if arm.cleaned else "não",
            f"{arm.top1}/{arm.n}", f"{arm.top3}/{arm.n}", f"{arm.mrr:.3f}",
            f"{arm.advertisement_tokens:,}", f"{arm.selection_tokens // max(arm.n,1):,}",
            f"${arm.turn_tokens / 1e6 * PRICE_IN:.5f}", f"{arm.p50_ms:.0f}",
        )
    console.print(table)

    best = max(arms, key=lambda a: a.top1)
    cheapest = min(arms, key=lambda a: a.turn_tokens)
    console.print(
        f"\n  melhor acurácia: {best.label} -> {best.top1}/{best.n} @ {best.turn_tokens:,} tok/turno"
        f"\n  mais barato:     {cheapest.label} -> {cheapest.top1}/{cheapest.n} @ {cheapest.turn_tokens:,} tok/turno"
    )
    for arm in sorted(arms, key=lambda a: a.width):
        extra = arm.turn_tokens - cheapest.turn_tokens
        gain = arm.top1 - cheapest.top1
        per = f"{extra / gain:,.0f} tok por acerto extra" if gain > 0 else "—"
        console.print(f"    {arm.label:<12} {gain:+d} acertos, {extra:+,} tok/turno   {per}")

    raw = OUT / "results-skills-width.json"
    raw.write_text(json.dumps([asdict(a) | {"turn_tokens": a.turn_tokens} for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()