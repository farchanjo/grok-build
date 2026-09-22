#!/usr/bin/env python3
"""Control for the workflow case set's known position defect.

`cases/workflow_cases.json` puts the gold at candidate position 0 in 14 of 15
cases, so an engine that leans on position scores well without judging. The
verification run showed Laya picking position 0 in 9 of 15 cases with all 9 of
its hits at that position, which is exactly the confound.

This re-runs the workflow experiment with the candidate list shuffled per case
(several seeds) and reports accuracy, position histograms, and the gap between
"picked the gold" and "picked position 0", for both engines.
"""

from __future__ import annotations

import argparse
import json
import random
import statistics
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

import httpx  # noqa: E402

from jev import primitives, transports  # noqa: E402

CASES = Path(__file__).parent / "cases"
WORKERS = 8


def one(client: transports.Client, case: dict[str, Any], order: list[str]) -> dict[str, Any]:
    question = {
        "name": primitives.choice(
            "Which name is best for this workflow? Prefer a name that is short, "
            "specific, and searchable by what it produces.",
            {c: c for c in order},
        )
    }
    response = client.ask({"workflow": case["intent"]}, question)
    picked = response.choice("name").choice
    return {
        "gold": case["gold"],
        "picked": picked,
        "correct": picked == case["gold"],
        "picked_position": order.index(picked) if picked in order else None,
        "gold_position": order.index(case["gold"]),
    }


def run(arm: str, seeds: int) -> None:
    cases = json.loads((CASES / "workflow_cases.json").read_text(encoding="utf-8"))
    build = (lambda: transports.Client.build("laya", model="typed-decisions", timeout_s=60.0)) \
        if arm == "laya" else (lambda: transports.Client.build("openrouter", timeout_s=60.0))

    per_seed: list[float] = []
    positions: list[int] = []
    started = time.time()
    for seed in range(seeds):
        rng = random.Random(seed)
        orders = []
        for case in cases:
            order = list(case["candidates"])
            rng.shuffle(order)
            orders.append(order)
        with build() as client, ThreadPoolExecutor(WORKERS) as pool:
            rows = list(pool.map(lambda pair: one(client, *pair), zip(cases, orders)))
        acc = sum(1 for r in rows if r["correct"]) / len(rows)
        per_seed.append(acc)
        positions += [r["picked_position"] for r in rows if r["picked_position"] is not None]

    hist = {p: positions.count(p) for p in sorted(set(positions))}
    print(
        f"{arm:<5} acc per seed {[f'{a:.0%}' for a in per_seed]}  "
        f"mean={statistics.mean(per_seed):.1%} sd={statistics.stdev(per_seed):.1%}  "
        f"picked-position hist={hist}  {time.time() - started:.0f}s"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seeds", type=int, default=4)
    parser.add_argument("--arm", default="both", choices=["laya", "jev", "both"])
    args = parser.parse_args()

    # Baseline: the case set's own order, which is what every earlier number used.
    cases = json.loads((CASES / "workflow_cases.json").read_text(encoding="utf-8"))
    gold_pos = {c["candidates"].index(c["gold"]) for c in cases}
    print(f"case set gold positions (original order): {sorted(gold_pos)}  <- position 0 is 14/15\n")

    for arm in (["laya", "jev"] if args.arm == "both" else [args.arm]):
        run(arm, args.seeds)


if __name__ == "__main__":
    main()