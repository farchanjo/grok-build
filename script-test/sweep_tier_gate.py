#!/usr/bin/env python3
"""Sweep the tier gate offline, from a verification run's own rows.

Re-running the arms for every candidate threshold would cost paid calls for
nothing: Laya is deterministic and Jev's answer for a case is already recorded, so
the tier's outcome at threshold T is exactly computable —

    accept from Laya when confidence >= T, otherwise take Jev's recorded answer

This reports, per experiment, the frontier a human has to choose from: the
threshold, the tier accuracy, and the share of paid calls avoided. It also
reports what "never escalate" (T = inf) and "always escalate" (T = 0) score, so
the gate has to beat both to be worth having.

    python sweep_tier_gate.py out/verify/verify-<ts>.jsonl
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Pair:
    """One case's two answers: what Laya said and what Jev said."""

    experiment: str
    case_id: str
    laya_confidence: float
    laya_pick: str
    jev_pick: str
    gold: str

    @property
    def laya_correct(self) -> bool:
        return self.laya_pick == self.gold

    @property
    def jev_correct(self) -> bool:
        return self.jev_pick == self.gold

    @property
    def agree(self) -> bool:
        return self.laya_pick == self.jev_pick


def load_pairs(path: Path) -> list[Pair]:
    rows: dict[tuple[str, str], dict[str, dict]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        row = json.loads(line)
        rows.setdefault((row["experiment"], row["case_id"]), {})[row["arm"]] = row
    pairs: list[Pair] = []
    for (experiment, case_id), arms in rows.items():
        laya, jev = arms.get("laya"), arms.get("jev")
        if not laya or not jev or laya["error"] or jev["error"]:
            continue
        if laya["gold"] is None:
            continue
        pairs.append(Pair(
            experiment=experiment,
            case_id=case_id,
            laya_confidence=laya["confidence"],
            laya_pick=laya["picked"],
            jev_pick=jev["picked"],
            gold=laya["gold"],
        ))
    return pairs


def score(pairs: list[Pair], threshold: float) -> tuple[float, int]:
    """Tier accuracy and paid calls avoided at ``threshold``."""
    accepted = [p for p in pairs if p.laya_confidence >= threshold]
    correct = sum(1 for p in accepted if p.laya_correct)
    correct += sum(1 for p in pairs if p.laya_confidence < threshold and p.jev_correct)
    return correct / len(pairs), len(accepted)


def consensus(pairs: list[Pair]) -> dict[str, float | int]:
    """What the agreement signal is worth, if both engines are always called.

    Agreement is not a saving — every case pays for both — but it is the signal
    the brief actually uses for escalation ("disagree -> escalate"). This reports
    whether it separates, so a caller can decide between gating on confidence
    (cheaper) and gating on agreement (costlier, possibly sharper).
    """
    agreed = [p for p in pairs if p.agree]
    split = [p for p in pairs if not p.agree]
    return {
        "n": len(pairs),
        "agreed": len(agreed),
        "agreed_accuracy": (sum(1 for p in agreed if p.jev_correct) / len(agreed)) if agreed else 0.0,
        "split": len(split),
        "split_laya_right": sum(1 for p in split if p.laya_correct),
        "split_jev_right": sum(1 for p in split if p.jev_correct),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", type=Path)
    args = parser.parse_args()

    pairs = load_pairs(args.path)
    experiments = sorted({p.experiment for p in pairs})

    print("=== agreement signal (both engines called on every case) ===")
    for experiment in experiments:
        subset = [p for p in pairs if p.experiment == experiment]
        c = consensus(subset)
        acc = c["agreed_accuracy"]
        print(f"  {experiment:<9} agree {c['agreed']:>3}/{c['n']:<3} "
              f"acc_when_agreed={acc:>6.1%}  split {c['split']:>2} "
              f"(laya right {c['split_laya_right']}, jev right {c['split_jev_right']})")

    for experiment in experiments:
        subset = [p for p in pairs if p.experiment == experiment]
        laya_acc = sum(1 for p in subset if p.laya_correct) / len(subset)
        jev_acc = sum(1 for p in subset if p.jev_correct) / len(subset)
        print(f"\n== {experiment}  n={len(subset)}")
        print(f"   laya alone {laya_acc:.1%}   jev alone {jev_acc:.1%}")
        print(f"   {'T':>6}  {'tier acc':>9}  {'accepted':>9}  {'paid avoided':>13}  {'vs best alone':>14}")
        best = None
        for threshold in [0.0, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.01]:
            acc, accepted = score(subset, threshold)
            best_alone = max(laya_acc, jev_acc)
            label = "inf" if threshold > 1 else f"{threshold:.2f}"
            delta = acc - best_alone
            print(f"   {label:>6}  {acc:>9.1%}  {accepted:>4}/{len(subset):<4}  "
                  f"{accepted / len(subset):>12.1%}  {delta:>+13.1%}")
            if best is None or acc > best[1]:
                best = (threshold, acc, accepted)
        print(f"   best T={best[0]:.2f} -> {best[1]:.1%} (paid avoided {best[2] / len(subset):.0%})")


if __name__ == "__main__":
    main()