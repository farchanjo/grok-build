#!/usr/bin/env python3
"""Probe: does Laya's confidence gate survive the harness's real workloads?

The integration brief calibrates GATE=0.70 on a 6-option pt-BR choice. The
harness's widest question is a 172-option skill choice, and ``confidence`` is
normalised Shannon entropy, so its scale depends on k. This probe measures the
actual confidence distribution and the pick accuracy before any wiring is
written.

    python probe_laya.py --experiment skills --limit 20
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

from jev import primitives  # noqa: E402

import httpx  # noqa: E402

LAYA = "http://192.168.200.32:8803/v1/decide"
MODEL = "multilingual"  # overridden by --model
CASES = Path(__file__).parent / "cases"


def post(client: httpx.Client, state: dict, questions: dict) -> dict:
    body = {"model": MODEL, "state": state, "questions": questions}
    response = client.post(LAYA, json=body)
    response.raise_for_status()
    return response.json()


def load(name: str) -> list[dict[str, Any]]:
    return json.loads((CASES / name).read_text(encoding="utf-8"))


def skill_case(case: dict, roster: list[dict], client: httpx.Client) -> dict:
    criteria = {s["name"]: s["description"][:60] for s in roster}
    questions = {
        "which": primitives.choice(
            "Which of these skills, if any, is the right one to load to help with the "
            "user's latest request?",
            criteria,
        )
    }
    started = time.perf_counter()
    payload = post(client, {"request": case["request"], "recent_context": ""}, questions)
    latency = (time.perf_counter() - started) * 1000
    answer = payload["answers"]["which"]
    ranked = sorted(answer["probabilities"].items(), key=lambda kv: -kv[1])
    return {
        "gold": case["gold"],
        "picked": answer["choice"],
        "confidence": answer["confidence"],
        "correct": answer["choice"] == case["gold"],
        "gold_in_top3": case["gold"] in [n for n, _ in ranked[:3]],
        "gold_rank": next((i for i, (n, _) in enumerate(ranked, 1) if n == case["gold"]), None),
        "top1_prob": ranked[0][1],
        "latency_ms": latency,
    }


def memory_case(case: dict, client: httpx.Client) -> dict:
    questions = {
        "should_store": primitives.noul(
            "This candidate note is being considered for long-term memory. Should it be "
            "stored, so a future session recalls it?",
            true="Durable fact, rule, preference, or verified result worth recalling later",
            false="Momentary state, trivially rediscoverable, or a duplicate",
        )
    }
    started = time.perf_counter()
    payload = post(client, {"candidate": case["request"]}, questions)
    latency = (time.perf_counter() - started) * 1000
    answer = payload["answers"]["should_store"]
    picked = "store" if answer["noul"] >= 0.5 else "skip"
    return {
        "gold": case["gold"],
        "picked": picked,
        "confidence": answer["confidence"],
        "noul": answer["noul"],
        "correct": picked == case["gold"],
        "latency_ms": latency,
    }


def agent_case(case: dict, roster: list[dict], client: httpx.Client) -> dict:
    criteria = {a["name"]: a["description"][:120] for a in roster}
    questions = {
        "which": primitives.choice(
            "Which subagent should handle this task, given it is being delegated from a "
            "parent session?",
            criteria,
        )
    }
    started = time.perf_counter()
    payload = post(client, {"task": case["request"]}, questions)
    latency = (time.perf_counter() - started) * 1000
    answer = payload["answers"]["which"]
    ranked = sorted(answer["probabilities"].items(), key=lambda kv: -kv[1])
    return {
        "gold": case["gold"],
        "picked": answer["choice"],
        "confidence": answer["confidence"],
        "correct": answer["choice"] == case["gold"],
        "gold_in_top3": case["gold"] in [n for n, _ in ranked[:3]],
        "gold_rank": next((i for i, (n, _) in enumerate(ranked, 1) if n == case["gold"]), None),
        "top1_prob": ranked[0][1],
        "latency_ms": latency,
    }


def workflow_case(case: dict, client: httpx.Client) -> dict:
    questions = {
        "name": primitives.choice(
            "Which name is best for this workflow? Prefer a name that is short, "
            "specific, and searchable by what it produces.",
            {c: c for c in case["candidates"]},
        )
    }
    started = time.perf_counter()
    payload = post(client, {"workflow": case["intent"]}, questions)
    latency = (time.perf_counter() - started) * 1000
    answer = payload["answers"]["name"]
    return {
        "gold": case["gold"],
        "picked": answer["choice"],
        "confidence": answer["confidence"],
        "correct": answer["choice"] == case["gold"],
        "latency_ms": latency,
    }


def report(name: str, rows: list[dict]) -> None:
    if not rows:
        return
    ok = [r for r in rows if "error" not in r]
    correct = sum(1 for r in ok if r["correct"])
    conf = sorted(r["confidence"] for r in ok)
    hit_conf = [r["confidence"] for r in ok if r["correct"]]
    miss_conf = [r["confidence"] for r in ok if not r["correct"]]
    print(f"\n== {name}: n={len(ok)} acc={correct}/{len(ok)} = {correct / len(ok):.1%}")
    print(f"   confidence p50={statistics.median(conf):.3f} max={conf[-1]:.3f} min={conf[0]:.3f}")
    if hit_conf:
        print(f"   conf|hit  p50={statistics.median(hit_conf):.3f}")
    if miss_conf:
        print(f"   conf|miss p50={statistics.median(miss_conf):.3f} max={max(miss_conf):.3f}")
    lat = sorted(r["latency_ms"] for r in ok)
    print(f"   latency p50={statistics.median(lat):.0f}ms p95={lat[int(len(lat) * 0.95) - 1]:.0f}ms")
    if "gold_in_top3" in ok[0]:
        t3 = sum(1 for r in ok if r["gold_in_top3"])
        print(f"   gold in top-3: {t3}/{len(ok)}")
    # Gate sweep: what would a confidence threshold buy?
    print("   gate sweep (accept if conf >= T):")
    for threshold in (0.0, 0.3, 0.5, 0.6, 0.7, 0.8):
        accepted = [r for r in ok if r["confidence"] >= threshold]
        escalated = len(ok) - len(accepted)
        acc = sum(1 for r in accepted if r["correct"]) / len(accepted) if accepted else 0.0
        print(f"     T={threshold:.1f}  accept {len(accepted):>3}/{len(ok)}  acc_on_accepted={acc:.1%}  escalated={escalated}")


def main() -> None:
    global MODEL
    parser = argparse.ArgumentParser()
    parser.add_argument("--experiment", default="all",
                        choices=["all", "skills", "agents", "memory", "workflow"])
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--model", default=MODEL)
    args = parser.parse_args()

    MODEL = args.model
    roster = load("roster.json")
    agents = load("agents.json")
    out: dict[str, list[dict]] = {}

    def run(name: str, cases: list[dict], fn) -> None:
        subset = cases[: args.limit] if args.limit else cases
        with httpx.Client(timeout=60.0) as client, ThreadPoolExecutor(args.workers) as pool:
            out[name] = list(pool.map(lambda c: fn(c, client), subset))

    if args.experiment in ("all", "skills"):
        run("skills", [dict(c, id=c["request"][:40]) for c in load("skill_cases.json")],
            lambda c, cl: skill_case(c, roster, cl))
    if args.experiment in ("all", "agents"):
        run("agents", load("agent_cases.json"), lambda c, cl: agent_case(c, agents, cl))
    if args.experiment in ("all", "memory"):
        run("memory", load("memory_cases.json"), lambda c, cl: memory_case(c, cl))
    if args.experiment in ("all", "workflow"):
        run("workflow", load("workflow_cases.json"), lambda c, cl: workflow_case(c, cl))

    for name, rows in out.items():
        report(name, rows)

    Path("/tmp/probe_laya.json").write_text(json.dumps(out, ensure_ascii=False, indent=2))
    print("\nraw -> /tmp/probe_laya.json")


if __name__ == "__main__":
    main()