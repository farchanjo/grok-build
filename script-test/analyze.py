"""Read a raw results log and break the errors down.

Accuracy alone hides the two things that decide whether a layer is usable:
whether the misses are near-misses (a lookalike sibling) or random, and whether
a gate that says "skip" is precise or just quiet. This reads ``out/results-*.jsonl``.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).parent


def load(transport: str) -> list[dict]:
    path = ROOT / "out" / f"results-{transport}.jsonl"
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def family(name: str | None) -> str:
    """The lookalike family a name belongs to: its first hyphenated token."""
    return (name or "").split("-")[0]


def skills(calls: list[dict]) -> None:
    for variant in sorted({c["variant"] for c in calls}):
        subset = [c for c in calls if c["experiment"] == "skills" and c["variant"] == variant]
        covered = [c for c in subset if c["gold"]]
        misses = [c for c in covered if c["picked"] != c["gold"]]
        sibling = sum(1 for c in misses if family(c["picked"]) == family(c["gold"]))
        shortlisted = sum(1 for c in misses if c["gold"] in c.get("extra", {}).get("shortlist", []))
        silent = sum(1 for c in covered if c["picked"] is None)
        print(f"\n== skills/{variant} ==")
        print(f"   misses: {len(misses)}/{len(covered)}")
        print(f"   of the misses, same family as gold: {sibling} ({sibling / len(misses):.0%})" if misses else "")
        print(f"   of the misses, gold was in the top-3 shortlist: {shortlisted} ({shortlisted / len(misses):.0%})" if misses else "")
        print(f"   gate said 'no skill' on a covered case: {silent}")
        for call in misses[:12]:
            print(f"     gold={call['gold']:<22} picked={call['picked']}")


def memory(calls: list[dict]) -> None:
    subset = [c for c in calls if c["experiment"] == "memory"]
    tp = sum(1 for c in subset if c["gold"] == "store" and c["picked"] == "store")
    fn = sum(1 for c in subset if c["gold"] == "store" and c["picked"] == "skip")
    fp = sum(1 for c in subset if c["gold"] == "skip" and c["picked"] == "store")
    tn = sum(1 for c in subset if c["gold"] == "skip" and c["picked"] == "skip")
    precision = tp / (tp + fp) if tp + fp else 0.0
    recall = tp / (tp + fn) if tp + fn else 0.0
    print("\n== memory: gate confusion (gold vs picked) ==")
    print(f"   store  -> store {tp:<3} skip {fn:<3}   (recall {recall:.0%})")
    print(f"   skip   -> store {fp:<3} skip {tn:<3}   (precision {precision:.0%})")
    print(f"   base rate: {sum(1 for c in subset if c['gold'] == 'store')}/{len(subset)} should store")
    print("   false negatives (worth keeping, dropped):")
    for call in [c for c in subset if c["gold"] == "store" and c["picked"] == "skip"][:8]:
        print(f"     {call['value']:.2f}  {call['case_id'][:88]}")
    print("   false positives (noise, kept):")
    for call in [c for c in subset if c["gold"] == "skip" and c["picked"] == "store"][:8]:
        print(f"     {call['value']:.2f}  {call['case_id'][:88]}")


def positional(calls: list[dict]) -> None:
    """Workflow candidates: is the model just picking by position?"""
    subset = [c for c in calls if c["experiment"] == "workflow"]
    entries = json.loads((ROOT / "cases" / "workflow_cases.json").read_text())
    by_intent = {entry["intent"][:48]: entry for entry in entries}
    gold_positions = Counter()
    picked_positions = Counter()
    for call in subset:
        entry = by_intent.get(call["case_id"])
        if not entry:
            continue
        gold_positions[entry["candidates"].index(call["gold"])] += 1
        if call["picked"] in entry["candidates"]:
            picked_positions[entry["candidates"].index(call["picked"])] += 1
    print("\n== workflow: position of gold vs position of pick ==")
    print(f"   gold sits at position: {dict(sorted(gold_positions.items()))}")
    print(f"   picked landed at    : {dict(sorted(picked_positions.items()))}")
    print("   misses:")
    for call in subset:
        if call["picked"] != call["gold"]:
            print(f"     gold={call['gold']:<24} picked={call['picked']}")


def agents(calls: list[dict]) -> None:
    subset = [c for c in calls if c["experiment"] == "agents"]
    misses = [c for c in subset if c["picked"] != c["gold"]]
    print(f"\n== agents: {len(misses)}/{len(subset)} misses ==")
    for call in misses:
        print(f"   gold={call['gold']:<24} picked={call['picked']:<24} effort={call['extra'].get('effort')}")
    print("   effort distribution:", dict(Counter(c["extra"].get("effort") for c in subset)))


def separated_calibration(calls: list[dict]) -> None:
    """Two different questions, two different curves.

    The gate noul predicts "is there a skill for this request"; the choice
    confidence predicts "is the pick right". Averaging them into one number
    hides which of the two is miscalibrated.
    """
    subset = [c for c in calls if c["experiment"] == "skills" and c["value"] is not None]
    print("\n== calibration, separated ==")
    print("   gate noul vs 'a skill exists' (all cases):")
    for low, high in ((0.0, 0.2), (0.2, 0.4), (0.4, 0.6), (0.6, 0.8), (0.8, 1.01)):
        inside = [c for c in subset if low <= c["value"] < high]
        if not inside:
            continue
        truth = sum(1 for c in inside if c["gold"]) / len(inside)
        print(f"     {low:.1f}-{high:.1f}  n={len(inside):<3} mean={sum(c['value'] for c in inside) / len(inside):.2f}  has-skill={truth:.0%}")
    picks = [c for c in subset if c["gold"] and c.get("extra", {}).get("choice_confidence") is not None]
    print("   choice confidence vs 'the pick is right' (covered cases only):")
    for low, high in ((0.0, 0.2), (0.2, 0.4), (0.4, 0.6), (0.6, 0.8), (0.8, 1.01)):
        inside = [c for c in picks if low <= c["extra"]["choice_confidence"] < high]
        if not inside:
            continue
        acc = sum(1 for c in inside if c["correct"]) / len(inside)
        print(f"     {low:.1f}-{high:.1f}  n={len(inside):<3} mean={sum(c['extra']['choice_confidence'] for c in inside) / len(inside):.2f}  correct={acc:.0%}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    calls = load(args.transport)
    skills(calls)
    separated_calibration(calls)
    memory(calls)
    agents(calls)
    positional(calls)


if __name__ == "__main__":
    main()