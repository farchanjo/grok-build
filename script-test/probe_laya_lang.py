#!/usr/bin/env python3
"""Probe variant: does a pt-BR payload rescue Laya on the harness workloads?

Section 4 of the brief claims 5/10 -> 7/10 purely from payload changes (pin the
model, write instructions and criteria in Portuguese, make `other` a strict last
resort). The first probe used the harness's English instructions and scored
13.6% on skills. This probe isolates the payload variables one at a time.

    python probe_laya_lang.py --experiment skills --variant ptbr_instr
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

import httpx  # noqa: E402

LAYA = "http://192.168.200.32:8803/v1/decide"
MODEL = "multilingual"
CASES = Path(__file__).parent / "cases"

# Instruction variants for the skill question. The English one is what the
# harness ships; the pt-BR ones follow section 4 of the brief.
SKILL_INSTR = {
    "en": (
        "Which of these skills, if any, is the right one to load to help with the "
        "user's latest request?"
    ),
    "ptbr": (
        "Qual destas skills, se alguma, deve ser carregada para ajudar no pedido "
        "mais recente do usuario?"
    ),
    "ptbr_strict": (
        "Qual destas skills, se alguma, deve ser carregada para ajudar no pedido "
        "mais recente do usuario? Use SOMENTE se nenhuma outra opcao couber."
    ),
}

AGENT_INSTR = {
    "en": (
        "Which subagent should handle this task, given it is being delegated from a "
        "parent session?"
    ),
    "ptbr": (
        "Qual subagente deve executar esta tarefa, sabendo que ela foi delegada a "
        "partir de uma sessao pai?"
    ),
}

MEMORY_INSTR = {
    "en": (
        "This candidate note is being considered for long-term memory. Should it be "
        "stored, so a future session recalls it?"
    ),
    "ptbr": (
        "Esta nota candidata esta sendo considerada para a memoria de longo prazo. "
        "Ela deve ser guardada, para que uma sessao futura a recorde?"
    ),
}

MEMORY_TRUE = {
    "en": "Durable fact, rule, preference, or verified result worth recalling later",
    "ptbr": "Fato duradouro, regra, preferencia ou resultado verificado que vale recordar depois",
}
MEMORY_FALSE = {
    "en": "Momentary state, trivially rediscoverable, or a duplicate",
    "ptbr": "Estado momentaneo, trivialmente redescobrivel, ou duplicata",
}


def load(name: str) -> list[dict[str, Any]]:
    return json.loads((CASES / name).read_text(encoding="utf-8"))


def post(client: httpx.Client, state: dict, questions: dict) -> dict:
    response = client.post(LAYA, json={"model": MODEL, "state": state, "questions": questions})
    response.raise_for_status()
    return response.json()


def run_choice(client: httpx.Client, state: dict, criteria: dict, instruction: str,
               gold: str, extra_other: bool) -> dict:
    if extra_other:
        criteria = dict(criteria)
        criteria["other"] = (
            "Nenhuma das outras opcoes; saudacoes, elogios puros ou pedido generico"
        )
    started = time.perf_counter()
    payload = post(client, state, {
        "which": {"type": "choice", "instructions": instruction, "criteria": criteria}
    })
    latency = (time.perf_counter() - started) * 1000
    answer = payload["answers"]["which"]
    ranked = sorted(answer["probabilities"].items(), key=lambda kv: -kv[1])
    return {
        "gold": gold,
        "picked": answer["choice"],
        "confidence": answer["confidence"],
        "correct": answer["choice"] == gold,
        "gold_in_top3": gold in [n for n, _ in ranked[:3]],
        "gold_rank": next((i for i, (n, _) in enumerate(ranked, 1) if n == gold), None),
        "latency_ms": latency,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--experiment", default="skills")
    parser.add_argument("--variant", default="ptbr")
    parser.add_argument("--width", type=int, default=60)
    parser.add_argument("--other", action="store_true")
    parser.add_argument("--workers", type=int, default=8)
    args = parser.parse_args()

    roster = load("roster.json")
    results: dict[str, list[dict]] = {}

    def go(name: str, fn) -> None:
        with httpx.Client(timeout=60.0) as client, ThreadPoolExecutor(args.workers) as pool:
            results[name] = list(pool.map(lambda c: fn(c, client), load(f"{name}_cases.json")))

    def instruction(table: dict[str, str]) -> str:
        return table[args.variant if args.variant in table else "ptbr"]

    if args.experiment == "skills":
        criteria = {s["name"]: s["description"][: args.width] for s in roster}
        instr = instruction(SKILL_INSTR)
        if args.variant.endswith("strict"):
            instr = SKILL_INSTR["ptbr_strict"]
        go("skill", lambda c, cl: run_choice(
            cl, {"request": c["request"], "recent_context": ""}, criteria, instr,
            c["gold"], args.other))

    if args.experiment == "agents":
        criteria = {a["name"]: a["description"][:120] for a in load("agents.json")}
        go("agent", lambda c, cl: run_choice(
            cl, {"task": c["request"]}, criteria, instruction(AGENT_INSTR), c["gold"], args.other))

    if args.experiment == "memory":
        def one(c: dict, cl: httpx.Client) -> dict:
            started = time.perf_counter()
            payload = post(cl, {"candidate": c["request"]}, {
                "should_store": {
                    "type": "noul",
                    "instructions": instruction(MEMORY_INSTR),
                    "criteria": {
                        "true": MEMORY_TRUE.get(args.variant, MEMORY_TRUE["ptbr"]),
                        "false": MEMORY_FALSE.get(args.variant, MEMORY_FALSE["ptbr"]),
                    },
                }
            })
            latency = (time.perf_counter() - started) * 1000
            answer = payload["answers"]["should_store"]
            picked = "store" if answer["noul"] >= 0.5 else "skip"
            return {"gold": c["gold"], "picked": picked, "confidence": answer["confidence"],
                    "correct": picked == c["gold"], "latency_ms": latency}
        go("memory", one)

    for name, rows in results.items():
        ok = rows
        correct = sum(1 for r in ok if r["correct"])
        conf = sorted(r["confidence"] for r in ok)
        hit = [r["confidence"] for r in ok if r["correct"]]
        miss = [r["confidence"] for r in ok if not r["correct"]]
        print(f"\n== {name} variant={args.variant} other={args.other} width={args.width}")
        print(f"   acc={correct}/{len(ok)} = {correct / len(ok):.1%}  (p50 conf={statistics.median(conf):.3f})")
        if hit:
            print(f"   conf|hit  p50={statistics.median(hit):.3f}")
        if miss:
            print(f"   conf|miss p50={statistics.median(miss):.3f} max={max(miss):.3f}")
        if "gold_in_top3" in ok[0]:
            print(f"   gold in top-3: {sum(1 for r in ok if r['gold_in_top3'])}/{len(ok)}")
        lat = sorted(r["latency_ms"] for r in ok)
        print(f"   latency p50={statistics.median(lat):.0f}ms")


if __name__ == "__main__":
    main()