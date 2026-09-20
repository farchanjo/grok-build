"""Verify that the workflow actually covers the plan.

A keyword check would miss a paraphrase, so this is two-stage like everything
else here: **embedding** finds the brief each plan item should live in, then
**jev** reads that brief and judges whether it instructs the agent to do the item.

  for each plan item
      best brief by cosine  ->  jev noul: "does this brief tell the agent to do this?"
      below threshold       ->  report as a gap

The reverse is reported too: brief instructions that back no plan item.

    python verify_plan_coverage.py --transport native
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_embedding import cached_local
from sim_memory_pipeline import cosine

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
WORKERS = 8
COVER_THRESHOLD = 0.6

COVER_QUESTION = (
    "A plan item is listed below, then a workflow brief. Does the brief actually instruct the "
    "agent to carry out this plan item — not merely mention the same area?"
)
COVER_TRUE = "The brief names this item's change and says to make it."
COVER_FALSE = "The brief is only in the same area, or omits the item entirely."


@dataclass
class Row:
    item: str
    title: str
    best_brief: str = ""
    similarity: float = 0.0
    retrieved: str = ""
    covered: bool | None = None
    noul: float | None = None
    error: str = ""


def plan_items() -> list[tuple[str, str]]:
    """(id, title) for every numbered row in PLAN.md, all phases."""
    out = []
    for line in (ROOT / "PLAN.md").read_text().splitlines():
        m = re.match(r"\| (\d+\.\d+b?) \| (.+?) \|", line)
        if m:
            # table cells escape literal pipes as `\|`; the judge should see `|`
            title = re.sub(r"[*`]", "", m.group(2)).replace("\\|", "|").strip()
            out.append((m.group(1), title))
    return out


# Which brief each item belongs to is known by construction — the plan groups the
# work and the rhai mirrors the groups. Retrieval over whole briefs is too noisy to
# recover it (long brief vs short item), so use the intended map and let the judge
# verify against it.
INTENDED = {
    "0": "phase0", "1": "phase1", "3": "correctness", "4": "transport-tui",
}
INTENDED_BY_ITEM = {
    **{f"2.{n}": "skills" for n in range(1, 8)},
    **{f"2.{n}": "memory" for n in range(8, 12)},
    **{f"2.{n}": "permission-laziness" for n in range(12, 17)},
    **{f"2.{n}": "agents-todo-workflow-goal" for n in range(17, 31)},
}


def intended_brief(item: str) -> str | None:
    if item.endswith("b"):
        item = item[:-1]
    return INTENDED_BY_ITEM.get(item) or INTENDED.get(item.split(".")[0])


def briefs(script: str) -> dict[str, str]:
    """label -> brief text, from each `prompt: SHARED + ...` up to its `label:`."""
    found: dict[str, str] = {}
    # single-agent phases use `let brief = SHARED + ...`, the parallel jobs use
    # `prompt: SHARED + ...`; both end at the label.
    for m in re.finditer(r'(?:let brief = SHARED|prompt: SHARED) \+ (.*?)label: "([^"]+)"', script, re.DOTALL):
        text = re.sub(r'"\s*\+\s*"', "", m.group(1))
        text = re.sub(r'\\n', " ", text)
        found[m.group(2)] = re.sub(r"\s+", " ", text)[:3000]
    return found


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    parser.add_argument("--all", action="store_true", help="also list the covered items")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    items = plan_items()
    script = (ROOT / "execute-jev-plan.rhai").read_text()
    bs = briefs(script)
    labels = list(bs)
    print(f"itens do plano: {len(items)} | briefs no rhai: {len(labels)}")

    brief_vecs = cached_local([bs[l] for l in labels])
    item_vecs = cached_local([f"{i} {t}" for i, t in items])

    def one(index: int) -> Row:
        item_id, title = items[index]
        row = Row(item=item_id, title=title)
        sims = sorted(((cosine(item_vecs[index], brief_vecs[j]), labels[j]) for j in range(len(labels))), reverse=True)
        row.similarity, retrieved = sims[0]
        row.best_brief = intended_brief(row.item) or retrieved
        row.retrieved = retrieved
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                response = client.ask(
                    {"plan_item": f"{item_id} — {title}", "brief": bs[row.best_brief]},
                    {"covers": primitives.noul(COVER_QUESTION, true=COVER_TRUE, false=COVER_FALSE)},
                )
        except transports.TransportError as error:
            row.error = str(error)
            return row
        row.noul = response.noul("covers")
        row.covered = row.noul >= COVER_THRESHOLD
        return row

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        rows = list(pool.map(one, range(len(items))))

    graded = [r for r in rows if r.error == ""]
    gaps = [r for r in graded if not r.covered]

    table = Table(title=f"cobertura do plano pelo rhai — {len(graded)} itens")
    table.add_column("medida", justify="left")
    table.add_column("n", justify="right")
    table.add_column("%", justify="right")
    table.add_row("itens cobertos", str(len(graded) - len(gaps)), f"{(len(graded)-len(gaps))/len(graded):.1%}")
    table.add_row("itens NÃO cobertos", str(len(gaps)), f"{len(gaps)/len(graded):.1%}")
    table.add_row("similaridade mediana ao brief escolhido", "—", f"{statistics.median(r.similarity for r in graded):.3f}")
    mismatch = sum(1 for r in graded if r.retrieved != r.best_brief)
    table.add_row("em que a recuperação erraria o brief", str(mismatch), f"{mismatch/len(graded):.1%}")
    console.print(table)

    if gaps:
        console.print("\n  [red]lacunas — o brief mais próximo não instrui o item:[/red]")
        for r in sorted(gaps, key=lambda r: r.similarity):
            console.print(f"    {r.item:<7} noul={r.noul:.2f} sim={r.similarity:.2f} brief={r.best_brief:<26} {r.title[:52]}")
    if args.all:
        console.print("\n  cobertos:")
        for r in sorted(graded, key=lambda r: r.item):
            if r.covered:
                console.print(f"    {r.item:<7} noul={r.noul:.2f} brief={r.best_brief:<26} {r.title[:48]}")

    # briefs que não servem a item nenhum
    used = {r.best_brief for r in graded}
    unused = [l for l in labels if l not in used]
    if unused:
        console.print(f"\n  briefs sem item correspondente: {unused}")

    raw = OUT / "coverage.json"
    raw.write_text(json.dumps([asdict(r) for r in rows], ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()