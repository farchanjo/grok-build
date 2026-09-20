"""Extract the permission policy's own labelled corpus from its tests.

``auto_mode.rs`` states, in its test module, which commands the auto-mode
classifier is supposed to allow and which must wait for the user. That is a
free labelled set: the intended policy, written by whoever built the engine,
with the exact edge cases they cared about (word-boundary lookalikes, compound
commands, env prefixes).

Mining it lets us ask the honest question — does a decision model reproduce the
policy the engine encodes, and where does it disagree?

Usage:  python mine_permission_corpus.py > cases/command_cases.json
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path("/Users/farchanjo/dev/grok-build")
SOURCES = [
    REPO / "crates/codegen/xai-grok-workspace/src/permission/auto_mode.rs",
    REPO / "crates/codegen/xai-grok-workspace/src/permission/exec_risk.rs",
]

# assert_eq!(v("cmd"), ClassifierVerdict::X)  and the multiline classify_sync form
CLOSURE = re.compile(r'v\("([^"]+)"\)\s*,\s*ClassifierVerdict::(Allow|Block)')
SYNC = re.compile(
    r'classify_sync\(\s*"[^"]*",\s*&?AccessKind::Bash\("([^"]+)"\.into\(\)\),[^;]*?ClassifierVerdict::(Allow|Block)',
    re.DOTALL,
)
BASH_THEN_VERDICT = re.compile(
    r'AccessKind::Bash\("([^"]+)"\.into\(\)\)(.{0,400}?)ClassifierVerdict::(Allow|Block)',
    re.DOTALL,
)


def extract(path: Path) -> list[dict[str, str]]:
    text = path.read_text(encoding="utf-8")
    start = text.find("#[cfg(test)]")
    if start == -1:
        return []
    body = text[start:]
    found: dict[str, str] = {}

    for match in CLOSURE.finditer(body):
        found.setdefault(match.group(1), match.group(2))
    for match in SYNC.finditer(body):
        found.setdefault(match.group(1), match.group(2))
    for match in BASH_THEN_VERDICT.finditer(body):
        command, _, verdict = match.groups()
        # A window can span two unrelated asserts; only trust the tight ones.
        if "ClassifierVerdict" in match.group(2) or len(match.group(2)) < 400:
            found.setdefault(command, verdict)

    return [
        {
            "command": command,
            "gold": "allow" if verdict == "Allow" else "block",
            "source": path.name,
        }
        for command, verdict in sorted(found.items())
    ]


def main() -> None:
    cases: list[dict[str, str]] = []
    seen: set[str] = set()
    for path in SOURCES:
        for case in extract(path):
            if case["command"] in seen:
                continue
            seen.add(case["command"])
            cases.append(case)
    cases.sort(key=lambda c: (c["gold"], c["command"]))
    print(json.dumps(cases, indent=2, ensure_ascii=False))
    print(
        f"# {len(cases)} cases: {sum(1 for c in cases if c['gold'] == 'allow')} allow, "
        f"{sum(1 for c in cases if c['gold'] == 'block')} block",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()