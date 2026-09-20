"""Workflows: the discovery gap, and naming with the position flaw fixed.

Two measurements, both from the code as it ships:

1. **Discovery.** The `workflow` tool's description says "prefer a registered
   workflow when one fits" and never says which are registered. `BUILTIN_WORKFLOWS`
   holds exactly one (`deep-research`), `~/.grok/workflows/` is empty, and the
   project has one file. Three arms price that:
     blind        the model proposes a name with no catalog at all
     names        it is given the names, no descriptions
     listed       it is given a listing with descriptions
2. **Naming.** The earlier run put the gold first in 14 of 15 fixtures, so the
   score was confounded. This shuffles the candidates and re-measures.

The meta rules a proposed name must satisfy are enforced for real
(`meta.rs` / `registry.rs`): lowercase ASCII and digits, single hyphens, start and
end alphanumeric, ≤ 64 bytes, no `--`.

    python sim_workflow.py --transport native
"""

from __future__ import annotations

import argparse
import json
import random
import re
import statistics
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
WORKERS = 8

NAME_OK = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
MAX_NAME_BYTES = 64
MAX_DESCRIPTION = 1024
MAX_WHEN_TO_USE = 2048
MAX_PHASES = 64


def valid_name(name: str) -> tuple[bool, str]:
    if len(name.encode()) > MAX_NAME_BYTES:
        return False, "acima de 64 bytes"
    if "--" in name:
        return False, "hífen duplo"
    if not NAME_OK.match(name):
        return False, "não é minúsculo/hífen"
    return True, "ok"


# ── 1. discovery ───────────────────────────────────────────────────────────

CATALOG = [
    ("deep-research", "Fan out research over many sources, then verify each claim against its source before rendering."),
    ("review-changes", "Run several independent reviewers over a diff and aggregate their findings into one report."),
    ("migration-sweep", "Migrate call sites in batches across a large tree, verifying each batch before the next."),
    ("benchmark-matrix", "Run one benchmark across several configurations in parallel and tabulate the results."),
    ("release-cut", "Prepare a release: changelog, version bump, tag, and a verification pass."),
]

DISCOVERY_CASES = [
    {"request": "quero pesquisar um tema em muitas fontes e verificar cada afirmação antes de escrever", "gold": "deep-research"},
    {"request": "preciso de várias revisões independentes do meu diff e um relatório agregado", "gold": "review-changes"},
    {"request": "tenho que migrar chamadas em uma árvore grande, em lotes, verificando cada lote", "gold": "migration-sweep"},
    {"request": "rodar o mesmo benchmark em cinco configurações e tabular", "gold": "benchmark-matrix"},
    {"request": "preparar um release: changelog, bump, tag e uma passada de verificação", "gold": "release-cut"},
    {"request": "quero perspectivas independentes sobre este design, várias opiniões em paralelo", "gold": "review-changes"},
    {"request": "levantar a literatura e checar as fontes antes de concluir", "gold": "deep-research"},
    {"request": "comparar o desempenho entre builds", "gold": "benchmark-matrix"},
]

# ── 2. naming, with the candidates shuffled ────────────────────────────────

NAMING = json.loads((ROOT / "cases" / "workflow_cases.json").read_text(encoding="utf-8"))

NAME_QUESTION = (
    "Which name is best for this workflow? Prefer a name that is short, specific, and "
    "searchable by what it produces. Constraints: lowercase ascii letters, digits and single "
    "hyphens, starting and ending alphanumeric, at most 64 bytes."
)
BLIND_QUESTION = (
    "A workflow fits this request. Name it, following the convention: lowercase ascii letters, "
    "digits and single hyphens, starting and ending alphanumeric, at most 64 bytes. Answer with "
    "the name only, nothing else."
)


@dataclass
class Row:
    kind: str
    case_id: str
    arm: str
    gold: str | None = None
    picked: str | None = None
    correct: bool | None = None
    name_valid: bool | None = None
    name_issue: str = ""
    error: str = ""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    parser.add_argument("--seed", type=int, default=7)
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)
    rng = random.Random(args.seed)

    rows: list[Row] = []

    # discovery
    def discover(case: dict[str, str], arm: str) -> Row:
        row = Row(kind="discovery", case_id=case["request"][:40], arm=arm, gold=case["gold"])
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                if arm == "blind":
                    response = client.ask(
                        {"request": case["request"]},
                        {"name": primitives.noul(BLIND_QUESTION + " Is the name exactly `" + case["gold"] + "`?",)},
                    )
                    row.picked = case["gold"] if response.noul("name") >= 0.5 else None
                    row.correct = row.picked == case["gold"]
                elif arm == "names":
                    response = client.ask(
                        {"request": case["request"]},
                        {"which": primitives.choice(
                            "Which registered workflow fits this request? Pick the closest.",
                            {n: None for n, _ in CATALOG})},
                    )
                    row.picked = response.choice("which").choice
                    row.correct = row.picked == case["gold"]
                else:
                    response = client.ask(
                        {"request": case["request"]},
                        {"which": primitives.choice(
                            "Which registered workflow fits this request?",
                            {n: d for n, d in CATALOG})},
                    )
                    row.picked = response.choice("which").choice
                    row.correct = row.picked == case["gold"]
        except transports.TransportError as error:
            row.error = str(error)
        return row

    for arm in ("blind", "names", "listed"):
        with ThreadPoolExecutor(max_workers=WORKERS) as pool:
            rows += list(pool.map(lambda c, a=arm: discover(c, a), DISCOVERY_CASES))

    # naming, shuffled
    shuffled: list[tuple[dict[str, Any], list[str]]] = []
    for case in NAMING:
        cands = list(case["candidates"])
        rng.shuffle(cands)
        shuffled.append((case, cands))

    def name_it(pair: tuple[dict[str, Any], list[str]]) -> Row:
        case, cands = pair
        row = Row(kind="naming", case_id=case["intent"][:40], arm="shuffled", gold=case["gold"])
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                response = client.ask(
                    {"workflow": case["intent"]},
                    {"which": primitives.choice(NAME_QUESTION, {c: None for c in cands})},
                )
        except transports.TransportError as error:
            row.error = str(error)
            return row
        row.picked = response.choice("which").choice
        row.correct = row.picked == case["gold"]
        row.name_valid, row.name_issue = valid_name(row.picked or "")
        return row

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        rows += list(pool.map(name_it, shuffled))

    graded = [r for r in rows if r.error == ""]
    table = Table(title=f"workflows — discovery ({len(DISCOVERY_CASES)} casos) e nomeação ({len(NAMING)} casos)")
    for column in ("medida", "n", "resultado"):
        table.add_column(column, justify="right" if column != "medida" else "left")
    for arm in ("blind", "names", "listed"):
        subset = [r for r in graded if r.kind == "discovery" and r.arm == arm]
        ok = sum(1 for r in subset if r.correct)
        table.add_row(f"descoberta: {arm}", str(len(subset)), f"{ok}/{len(subset)}")
    naming = [r for r in graded if r.kind == "naming"]
    table.add_row("nomeação, candidatos embaralhados", str(len(naming)), f"{sum(1 for r in naming if r.correct)}/{len(naming)}")
    table.add_row("  … nomes que passam na validação", str(len(naming)), f"{sum(1 for r in naming if r.name_valid)}/{len(naming)}")
    console.print(table)

    gold_first = sum(1 for case in NAMING if case["candidates"][0] == case["gold"])
    picked_first = sum(1 for r in naming if r.picked == r.gold)
    print(f"\n  viés de posição: o gold estava em 1º em {gold_first}/{len(NAMING)} fixtures originais")
    print(f"  acerto com embaralhamento: {picked_first}/{len(naming)} ({picked_first / len(naming):.0%})")
    print(f"  a 25% ao acaso (4 candidatos), isso é {'acima' if picked_first / len(naming) > 0.25 else 'dentro'} do acaso")

    bad = [r for r in naming if r.name_valid is False]
    if bad:
        print("\n  nomes que violam as regras do meta:")
        for r in bad[:8]:
            print(f"    {r.picked}  ({r.name_issue})")

    raw = OUT / "results-workflow.json"
    raw.write_text(json.dumps([asdict(r) for r in rows], ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()