"""Sweep the admission threshold on the pipeline's own event stream.

The end-to-end run lost 4 valuable notes at threshold 0.25. This collects the two
gate scores once per event and then sweeps offline, so the trade between losing a
note and admitting noise is visible instead of guessed.

    python sim_memory_sweep.py --transport native
"""

from __future__ import annotations

import argparse
import json
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_memory_pipeline import (
    ADMIT_THRESHOLD,
    COVERED_QUESTION,
    COVERED_THRESHOLD,
    EVENTS,
    GATE_FALSE,
    GATE_QUESTION,
    GATE_TRUE,
    OUT,
    WORKSPACE_PATH,
)

console = Console()
WORKERS = 8


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()

    rows: list[tuple[str, bool, bool, float, float]] = []
    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        def one(event) -> tuple:
            state = {
                "candidate_note": event.text,
                "workspace_path": WORKSPACE_PATH,
                "already_in_memory": "(empty)",
            }
            response = client.ask(
                state,
                {
                    "worth": primitives.noul(GATE_QUESTION, true=GATE_TRUE, false=GATE_FALSE),
                    "covered": primitives.noul(COVERED_QUESTION),
                },
            )
            return (event.text, event.keep, event.dupe, response.noul("worth"), response.noul("covered"))

        with ThreadPoolExecutor(max_workers=WORKERS) as pool:
            rows = list(pool.map(one, EVENTS))

    valuable = [r for r in rows if r[1] and not r[2]]
    dupes = [r for r in rows if r[2]]
    noise = [r for r in rows if not r[1]]

    table = Table(title="admission threshold sweep — 36 valuable, 8 restatements, 12 noise")
    for column in ("worth >=", "valuable kept", "valuable lost", "restatements admitted", "noise admitted"):
        table.add_column(column, justify="right" if column != "worth >=" else "left")
    for threshold in (0.10, 0.15, 0.20, 0.25, 0.30, 0.35, 0.40, 0.50):
        kept = sum(1 for r in valuable if r[3] >= threshold)
        lost = len(valuable) - kept
        dupes_in = sum(1 for r in dupes if r[3] >= threshold and r[4] < COVERED_THRESHOLD)
        noise_in = sum(1 for r in noise if r[3] >= threshold)
        marker = "  <- measured" if abs(threshold - ADMIT_THRESHOLD) < 1e-9 else ""
        table.add_row(
            f"{threshold:.2f}{marker}",
            str(kept),
            f"[red]{lost}[/red]" if lost else "[green]0[/green]",
            str(dupes_in),
            str(noise_in),
        )
    console.print(table)

    console.print(
        f"\n  restatements are refused by the separate `covered` question at "
        f">= {COVERED_THRESHOLD} — a lower `worth` bar does not let them back in"
    )
    raw = OUT / "results-memory-sweep.json"
    raw.write_text(
        json.dumps(
            [{"text": t, "keep": k, "dupe": d, "worth": w, "covered": c} for t, k, d, w, c in rows],
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    console.print(f"  -> {raw.name}")


if __name__ == "__main__":
    main()