"""Can embedding or Jev improve the plan workflow itself?

Two questions, measured rather than asserted.

1. **Embedding for the fan-out.** The four parallel jobs and their file ownership
   were hand-written. Embedding the plan's items and clustering them asks whether
   that decomposition was right, and whether a machine grouping collides less on
   files than mine does.

2. **Jev inside the script.** The engine has no `decide`, so Jev is only reachable
   through `agent()` — which costs a whole subagent spawn. That prices it against
   the decisions the workflow currently hardcodes.

    python sim_workflow_embedding.py --transport native
"""

from __future__ import annotations

import argparse
import itertools
import json
import re
from collections import defaultdict
from pathlib import Path

from jev import primitives, transports
from sim_embedding import cached_local
from sim_memory_pipeline import cosine

ROOT = Path(__file__).parent
OUT = ROOT / "out"

HAND_GROUPS = {
    "skills": ["2.1", "2.2", "2.3", "2.4", "2.5", "2.6", "2.7"],
    "memory": ["2.8", "2.9", "2.10", "2.11"],
    "permission-laziness": ["2.12", "2.13", "2.14", "2.15", "2.16", "2.16b"],
    "agents-todo-workflow-goal": ["2.17", "2.18", "2.19", "2.20", "2.20b", "2.21",
                                  "2.22", "2.23", "2.24", "2.25", "2.26", "2.27",
                                  "2.28", "2.29", "2.30"],
}
# files each item is expected to touch, taken from the findings docs
DOC_FOR = {
    "skills": "FINDINGS-SKILLS.md", "memory": "FINDINGS-MEMORY.md",
    "permission": "FINDINGS-PERMISSION.md", "laziness": "FINDINGS-LAZINESS.md",
    "agents": "FINDINGS-AGENTS.md", "todo": "FINDINGS-TODO-GATE.md",
    "workflow": "FINDINGS-WORKFLOW.md",
}
PATH = re.compile(r"([a-z0-9_]+(?:/[a-z0-9_]+)*\.(?:rs|rhai))")


def items_from_plan() -> dict[str, str]:
    text = (ROOT / "PLAN.md").read_text()
    out: dict[str, str] = {}
    for line in text.splitlines():
        m = re.match(r"\| (\d+\.\d+b?) \| (.+?) \| `([A-Z-]+)`", line)
        if m and m.group(1).startswith("2."):
            # strip markdown so the embedding sees the words, not the markup
            title = re.sub(r"[*`]", "", m.group(2)).strip()
            out[m.group(1)] = title
    return out


def files_for(label: str) -> set[str]:
    doc = DOC_FOR.get(label)
    if not doc:
        return set()
    return set(PATH.findall((ROOT / doc).read_text()))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    items = items_from_plan()
    ids = sorted(items, key=lambda k: (int(k.split(".")[1].rstrip("b")), k.endswith("b")))
    texts = [items[i] for i in ids]
    print(f"itens da fase 2 no plano: {len(ids)}")

    vectors = cached_local(texts)

    # ── 1. does clustering reproduce the hand grouping? ────────────────────
    hand_of = {i: g for g, members in HAND_GROUPS.items() for i in members}
    print(f"itens com grupo à mão: {len(hand_of)}")

    # greedy agglomerative: repeatedly merge the most similar pair
    def sim(a: str, b: str) -> float:
        return cosine(vectors[ids.index(a)], vectors[ids.index(b)])

    clusters = [[i] for i in ids]
    while len(clusters) > 4:
        best, pair = -2.0, None
        for a, b in itertools.combinations(range(len(clusters)), 2):
            s = max(sim(x, y) for x in clusters[a] for y in clusters[b])
            if s > best:
                best, pair = s, (a, b)
        a, b = pair
        clusters[a] = clusters[a] + clusters[b]
        clusters.pop(b)

    print("\nagrupamento por embedding (4 clusters):")
    for c in sorted(clusters, key=lambda c: -len(c)):
        print(f"  {len(c):>2} itens: {', '.join(sorted(c, key=lambda x: ids.index(x)))}")

    agree = 0
    for c in clusters:
        labels = defaultdict(int)
        for i in c:
            labels[hand_of.get(i, "?")] += 1
        top, n = max(labels.items(), key=lambda kv: kv[1])
        agree += n
        print(f"    cluster de {len(c):>2}: {n}/{len(c)} batem com '{top}'")
    print(f"  concordância total com o agrupamento à mão: {agree}/{len(ids)}")

    # ── 2. does the hand grouping collide less? ────────────────────────────
    print("\ncolisão de arquivos por agrupamento:")
    # files per hand group: the union over the docs its members cite
    hand_files = {}
    for g, members in HAND_GROUPS.items():
        docs = {m.group(1).lower() for m in (re.match(r"\| (2\.\d+b?) \| (.+?) \| `([A-Z-]+)`", l)
                for l in (ROOT / "PLAN.md").read_text().splitlines()) if m and m.group(1) in members}
        hand_files[g] = set().union(*[files_for(d) for d in docs]) if docs else set()
    for name, groups in (
        ("à mão", hand_files),
        ("embedding", {f"c{k}": set().union(*[files_for(hand_of.get(i, "")) for i in c]) for k, c in enumerate(clusters)}),
    ):
        pairs = [(a, b) for a, b in itertools.combinations(groups, 2) if groups[a] & groups[b]]
        shared = set()
        for a, b in pairs:
            shared |= groups[a] & groups[b]
        print(f"  {name:<10} {len(pairs)} par(es) colidindo em {len(shared)} arquivo(s)")

    # ── 3. jev's price inside the script ───────────────────────────────────
    print("\njev dentro do script: o que ele decidiria hoje")
    decisions = [
        ("should this phase be skipped?", "the tree already contains the change"),
        ("retry or escalate after a partial?", "the agent reported partial with a reason"),
        ("which crates to verify?", "the diff touched a set of crates"),
    ]
    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for question, state in decisions:
            response = client.ask(
                {"situation": state},
                {"d": primitives.noul(f"{question} Given only: {state}")},
            )
            print(f"  {question:<44} noul={response.noul('d'):.2f} tokens={response.input_tokens}")

    (OUT / "workflow-embedding.json").write_text(
        json.dumps({"clusters": clusters, "agreement": agree, "total": len(ids)}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"\n  -> {(OUT / 'workflow-embedding.json').name}")


if __name__ == "__main__":
    main()