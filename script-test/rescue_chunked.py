#!/usr/bin/env python3
"""Can a chunked payload rescue Laya where the flat 172-option list cannot?

Measured: `head_max_len=256` caps each question's option block, and at 172
options the per-option criteria text collapses to ~0.3 tokens — the engine sees
skill *names* and almost no descriptions. Over 256 options the request 400s.

This tests the obvious repair: split the roster into chunks small enough that
descriptions survive, ask each chunk in a single request (one question per chunk,
so one call), then re-rank the survivors with their full text. Both stages are
Laya-only and cost nothing.

    python rescue_chunked.py --chunk 40 --desc 60
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import httpx  # noqa: E402

LAYA = "http://192.168.200.32:8803/v1/decide"
MODEL = "typed-decisions"
CASES = Path(__file__).parent / "cases"
WORKERS = 8


def load(name: str):
    return json.loads((CASES / name).read_text(encoding="utf-8"))


def ask(client: httpx.Client, state: dict, questions: dict) -> dict:
    response = client.post(LAYA, json={"model": MODEL, "state": state, "questions": questions})
    response.raise_for_status()
    return response.json()


def pick_from(payload: dict, key: str) -> tuple[str, float]:
    answer = payload["answers"][key]
    return answer["choice"], answer["confidence"]


def run_case(client: httpx.Client, case: dict, chunks: list[list[dict]], desc: int) -> dict:
    """Stage 1: best skill per chunk. Stage 2: re-rank the survivors in full."""
    state = {"request": case["request"], "recent_context": ""}
    questions = {
        f"chunk_{i}": {
            "type": "choice",
            "instructions": (
                "Qual destas skills melhor corresponde ao pedido do usuario? "
                "Responda a mais provavel mesmo que nenhuma seja perfeita."
            ),
            "criteria": {s["name"]: s["description"][:desc] for s in chunk},
        }
        for i, chunk in enumerate(chunks)
    }
    stage1 = ask(client, state, questions)
    survivors = [pick_from(stage1, key)[0] for key in sorted(stage1["answers"])]

    by_name = {s["name"]: s for chunk in chunks for s in chunk}
    final_criteria = {name: by_name[name]["description"][:desc] for name in survivors}
    stage2 = ask(client, state, {
        "which": {
            "type": "choice",
            "instructions": (
                "Qual destas skills, se alguma, deve ser carregada para ajudar no "
                "pedido mais recente do usuario?"
            ),
            "criteria": final_criteria,
        }
    })
    picked, confidence = pick_from(stage2, "which")
    return {
        "gold": case["gold"],
        "picked": picked,
        "correct": picked == case["gold"],
        "gold_survived": case["gold"] in survivors,
        "confidence": confidence,
    }


def run(roster: list[dict], chunk_size: int, desc: int, workers: int) -> dict:
    cases = load("skill_cases.json")
    chunks = [roster[i:i + chunk_size] for i in range(0, len(roster), chunk_size)]
    with httpx.Client(timeout=120.0) as client, ThreadPoolExecutor(workers) as pool:
        rows = list(pool.map(lambda c: run_case(client, c, chunks, desc), cases))
    correct = sum(1 for r in rows if r["correct"])
    survived = sum(1 for r in rows if r["gold_survived"])
    return {
        "chunk": chunk_size, "desc": desc, "chunks": len(chunks),
        "correct": correct, "n": len(rows), "gold_survived": survived,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--chunk", type=int, default=0, help="0 = sweep a few sizes")
    parser.add_argument("--desc", type=int, default=60)
    parser.add_argument("--workers", type=int, default=8)
    args = parser.parse_args()

    roster = load("roster.json")
    sizes = [args.chunk] if args.chunk else [20, 30, 40, 60, 86]
    print(f"{'chunk':>6} {'chunks':>7} {'gold survived':>14} {'acc':>8}   (flat-172 baseline: 20.4%)")
    for size in sizes:
        started = time.perf_counter()
        stats = run(roster, size, args.desc, args.workers)
        print(f"{stats['chunk']:>6} {stats['chunks']:>7} "
              f"{stats['gold_survived']:>7}/{stats['n']:<6} "
              f"{stats['correct'] / stats['n']:>7.1%}   {time.perf_counter() - started:.0f}s")


if __name__ == "__main__":
    main()