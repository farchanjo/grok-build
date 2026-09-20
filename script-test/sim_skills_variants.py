"""Does the corpus's own taxonomy fix the umbrella error mode?

18 skill names are a hyphen-subset of another and absorb 58 children between
them. Every one of those umbrella skills declares its children in the frontmatter:

    metadata:
      variants: "bind9-dns, coredns-dns, dnsmasq-dns, unbound-dns"

and its description says so out loud: "Hub stub never inlines variant content;
matcher picks highest-scoring variant per current context." Nothing reads the
field.

So: when the chooser lands on an umbrella that declares variants, ask a second
question over exactly those variants and take that answer. That is a two-stage
shape again — but the shortlist is not "the top three", it is the declared
children, which is why it can work where the body re-read did not.

    python sim_skills_variants.py --transport native
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
from sim_vs_stack import bm25_rank, dense_rank, rrf

ROOT = Path(__file__).parent
OUT = ROOT / "out"
SKILLS = Path("~/.grok/skills").expanduser()
console = Console()
SHORTLIST = 10
WIDTH = 60

SELECT_QUESTION = (
    "Which of these skills is the right one to load for this request? When a broad umbrella "
    "skill and a specific one both cover the request, prefer the specific one."
)
VARIANT_QUESTION = (
    "This request matches the topic area, but the topic has several implementation variants. "
    "Which variant is the right one for this specific request?"
)


def variants_of(name: str) -> list[str]:
    path = SKILLS / name / "SKILL.md"
    if not path.is_file():
        return []
    text = path.read_text(encoding="utf-8", errors="replace")
    end = text.find("\n---", 3)
    block = text[:end] if end != -1 else text
    match = re.search(r'^\s+variants:\s*"?(.+?)"?\s*$', block, re.MULTILINE)
    if not match:
        return []
    return [v.strip().strip('"') for v in match.group(1).split(",") if v.strip()]


@dataclass
class Arm:
    label: str
    n: int = 0
    top1: int = 0
    top3: int = 0
    mrr: float = 0.0
    routed: int = 0
    routed_fixed: int = 0
    routed_broke: int = 0
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
    index = {s["name"]: clean(s["description"])[:WIDTH] for s in corpus}
    variants = {s["name"]: variants_of(s["name"]) for s in corpus}
    declared = sum(1 for v in variants.values() if v)
    print(f"skills com variants declarados: {declared}")

    docs = [f"{s['name']}: {s['description']}" for s in corpus]
    queries = [c["request"] for c in cases]
    vectors = cached_local(docs)
    qvecs = cached_local(queries)

    arms = [Arm("choice only"), Arm("choice + variants routing")]

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            latencies = []
            for case, qvec in zip(cases, qvecs):
                started = time.perf_counter()
                lexical = bm25_rank(corpus, case["request"], SHORTLIST)
                dense = dense_rank(corpus, vectors, qvec, SHORTLIST)
                shortlist = rrf(lexical, dense)[:SHORTLIST]
                if len(shortlist) >= 2:
                    response = client.ask(
                        {"request": case["request"]},
                        {"which": primitives.choice(SELECT_QUESTION, {n: index[n] for n in shortlist})},
                    )
                    arm.tokens += response.input_tokens
                    picked = response.choice("which").choice
                    shortlist = [picked] + [n for n in shortlist if n != picked]

                    if arm.label.endswith("routing") and variants.get(picked):
                        options = [v for v in variants[picked] if v in index]
                        if len(options) >= 2:
                            arm.routed += 1
                            second = client.ask(
                                {"request": case["request"]},
                                {"which": primitives.choice(VARIANT_QUESTION, {v: index[v] for v in options})},
                            )
                            arm.tokens += second.input_tokens
                            refined = second.choice("which").choice
                            if refined != picked:
                                if refined == case["gold"]:
                                    arm.routed_fixed += 1
                                elif picked == case["gold"]:
                                    arm.routed_broke += 1
                                shortlist = [refined] + [n for n in shortlist if n != refined]
                latencies.append((time.perf_counter() - started) * 1000)
                if shortlist:
                    if shortlist[0] == case["gold"]:
                        arm.top1 += 1
                    if case["gold"] in shortlist[:3]:
                        arm.top3 += 1
                    if case["gold"] in shortlist:
                        arm.mrr += 1.0 / (shortlist.index(case["gold"]) + 1)
                arm.detail.append({"q": case["request"][:40], "gold": case["gold"], "top": shortlist[0] if shortlist else None})
            arm.n = len(cases)
            arm.mrr /= len(cases)
            arm.p50_ms = statistics.median(latencies)

    table = Table(title=f"taxonomy routing — {len(cases)} queries, hybrid + solaris embedding + jev")
    for column in ("arm", "top-1", "top-3", "MRR", "routed", "fixed", "broke", "p50 ms"):
        table.add_column(column, justify="right" if column != "arm" else "left")
    for arm in arms:
        table.add_row(arm.label, f"{arm.top1}/{arm.n}", f"{arm.top3}/{arm.n}", f"{arm.mrr:.3f}",
                      str(arm.routed), str(arm.routed_fixed), str(arm.routed_broke), f"{arm.p50_ms:.0f}")
    console.print(table)

    routing = arms[1]
    if routing.routed:
        console.print(
            f"\n  routing disparou em {routing.routed} consultas: "
            f"[green]{routing.routed_fixed} corrigidas[/green], [red]{routing.routed_broke} quebradas[/red]"
        )
        broke = [d for d in routing.detail if d["top"] != d["gold"]][:6]
        for row in broke:
            console.print(f"    gold={row['gold']:<22} top={row['top']}")

    raw = OUT / "results-skills-variants.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()