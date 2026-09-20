"""Build the case-set inputs from the live profile, read-only.

Reads ``~/.grok/skills`` and ``~/.grok/agents`` and writes ``cases/roster.json``
and ``cases/agents.json``. Nothing under ``~/.grok`` is modified.

The skill description is what a roster index actually shows, so the case set is
built from that field and not from the body: judging a roster is the point.
"""

from __future__ import annotations

import json
import os
import re
from pathlib import Path

GROK_HOME = Path(os.environ.get("GROK_HOME_FOR_CASES", "~/.grok")).expanduser()
CASES = Path(__file__).parent / "cases"

_NAME = re.compile(r'^name:\s*"?(?P<name>[^"\n]+?)"?\s*$', re.MULTILINE)


def frontmatter(path: Path) -> dict[str, str]:
    """Minimal frontmatter reader: enough for ``name`` and ``description``."""
    text = path.read_text(encoding="utf-8", errors="replace")
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    block = text[3:end]
    fields: dict[str, str] = {}
    lines = block.splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        match = re.match(r"^([a-z_]+):\s*(.*)$", line)
        if not match:
            index += 1
            continue
        key, rest = match.group(1), match.group(2).strip()
        if rest in (">-", ">", "|", "|-"):
            # Folded scalar: consume the indented continuation lines.
            parts: list[str] = []
            index += 1
            while index < len(lines) and (lines[index].startswith("  ") or not lines[index].strip()):
                parts.append(lines[index].strip())
                index += 1
            fields[key] = " ".join(p for p in parts if p)
            continue
        fields[key] = rest.strip('"').strip("'")
        index += 1
    return fields


def collect(directory: Path, pattern: str) -> list[dict[str, str]]:
    entries: list[dict[str, str]] = []
    if not directory.is_dir():
        return entries
    for path in sorted(directory.glob(pattern)):
        fields = frontmatter(path)
        name = fields.get("name") or path.parent.name if pattern.endswith("SKILL.md") else path.stem
        description = fields.get("description", "")
        if not description:
            continue
        entries.append({"name": name.strip(), "description": description.strip()})
    return entries


def main() -> None:
    CASES.mkdir(exist_ok=True)
    skills = collect(GROK_HOME / "skills", "*/SKILL.md")
    agents = collect(GROK_HOME / "agents", "*.md")
    for filename, entries, label in (
        ("roster.json", skills, "skills"),
        ("agents.json", agents, "agents"),
    ):
        target = CASES / filename
        target.write_text(json.dumps(entries, indent=2, ensure_ascii=False), encoding="utf-8")
        widths = [len(e["description"]) for e in entries] or [0]
        print(
            f"{label}: {len(entries)} entries -> {target.name} "
            f"(description avg {sum(widths) // len(widths)} chars, max {max(widths)})"
        )


if __name__ == "__main__":
    main()