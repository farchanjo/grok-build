#!/usr/bin/env python3
"""Strip the natural-language eval cases this run appended to the user's skills.

Phase 2 of `execute-jev-plan-3` added `nl-<skill>` should_trigger cases to
`~/.grok/skills/*/evals/cases.yaml` — 40 files, 47 cases, outside the repo and
outside git. Each case is one appended block, so removing the blocks restores
the previous content; nothing else in those files was touched.

    python3 revert-skill-nl-cases.py            # report only
    python3 revert-skill-nl-cases.py --apply    # rewrite the files
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

SKILLS = Path.home() / ".grok" / "skills"
ENTRY = "  - id: "


def strip_nl_cases(text: str) -> tuple[str, int]:
    """Drop every `nl-` case block; return the new text and how many went."""
    lines = text.splitlines(keepends=True)
    kept: list[str] = []
    removed = 0
    dropping = False
    for line in lines:
        if line.startswith(ENTRY):
            dropping = line[len(ENTRY) :].startswith("nl-")
            if dropping:
                removed += 1
                continue
        elif not line.startswith(" ") and line.strip() and not line.startswith("#"):
            dropping = False  # a top-level key ends any block
        if not dropping:
            kept.append(line)
    return "".join(kept), removed


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--apply", action="store_true", help="rewrite instead of reporting")
    parser.add_argument("--skills", type=Path, default=SKILLS)
    args = parser.parse_args(argv)

    files = sorted(args.skills.glob("*/evals/cases.yaml"))
    total = 0
    touched = 0
    for path in files:
        original = path.read_text(encoding="utf-8")
        if "id: nl-" not in original:
            continue
        stripped, removed = strip_nl_cases(original)
        if not removed:
            continue
        touched += 1
        total += removed
        print(f"{'rewrite' if args.apply else 'would strip'} {removed} case(s) from {path.parent.parent.name}")
        if args.apply:
            path.write_text(stripped, encoding="utf-8")
    print(f"\n{touched} file(s), {total} nl- case(s) {'removed' if args.apply else 'to remove'}")
    return 0 if total else 1


if __name__ == "__main__":
    raise SystemExit(main())