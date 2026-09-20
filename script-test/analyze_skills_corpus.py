"""Where the skills corpus itself can be improved.

Read-only analysis of the live roster. No model calls. Every number here comes
from the shipped SKILL.md frontmatter, and each one maps to a place in the
pipeline that decides something.

What it looks for, and why each matters:

* **when_to_use presence** — the listing splits `MAX_LISTING_COMBINED_BYTES` (400)
  proportionally between description and when_to_use, so a long when_to_use
  steals bytes from the description.
* **trigger prefixes** — `strip_leading_trigger_prefix` removes phrases like
  "use this skill when" before listing, so a description can lose its opening.
* **short descriptions** — below `MIN_DESC_LENGTH` (20) the listing falls back to
  names only.
* **name-only ambiguity** — how many skill names are a hyphen-segment subset of
  another. This is the umbrella layer, quantified.
* **first-N-byte uniqueness** — for each skill, how many siblings share its
  opening bytes. If narrowing the index to 60 bytes collapses siblings, the
  narrowing has a cost that must be paid elsewhere.

    python analyze_skills_corpus.py
"""

from __future__ import annotations

import json
import re
import statistics
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).parent
SKILLS = Path("~/.grok/skills").expanduser()
OUT = ROOT / "out"

TRIGGER_PREFIXES = [
    "use this skill when", "use when", "auto-invoke when", "invoke when",
    "triggers on", "trigger on", "called when", "must trigger when",
    "must invoke when", "must be invoked when",
]
MIN_DESC_LENGTH = 20
MAX_LISTING_COMBINED_BYTES = 400


def frontmatter(path: Path) -> dict[str, str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    fields: dict[str, str] = {}
    lines = text[3:end].splitlines()
    index = 0
    while index < len(lines):
        match = re.match(r"^([a-z_]+):\s*(.*)$", lines[index])
        if not match:
            index += 1
            continue
        key, rest = match.group(1), match.group(2).strip()
        if rest in (">-", ">", "|", "|-"):
            parts, index = [], index + 1
            while index < len(lines) and (lines[index].startswith("  ") or not lines[index].strip()):
                parts.append(lines[index].strip())
                index += 1
            fields[key] = " ".join(p for p in parts if p)
            continue
        fields[key] = rest.strip('"').strip("'")
        index += 1
    return fields


def main() -> None:
    skills = []
    for path in sorted(SKILLS.glob("*/SKILL.md")):
        fields = frontmatter(path)
        grok = {}
        raw = path.read_text(encoding="utf-8", errors="replace")
        block = raw[: raw.find("\n---", 3)] if raw.startswith("---") else ""
        for key in ("when_to_use", "short_description"):
            match = re.search(rf"^\s+{key}:\s*(.+)$", block, re.MULTILINE)
            if match:
                grok[key] = match.group(1).strip().strip('"')
        skills.append(
            {
                "name": fields.get("name") or path.parent.name,
                "description": fields.get("description", ""),
                "when_to_use": grok.get("when_to_use"),
                "short_description": grok.get("short_description"),
            }
        )

    print(f"skills analisadas: {len(skills)}\n")

    lengths = [len(s["description"]) for s in skills]
    print("== descrição ==")
    print(f"  min {min(lengths)}  mediana {statistics.median(lengths):.0f}  max {max(lengths)}")
    print(f"  abaixo de MIN_DESC_LENGTH ({MIN_DESC_LENGTH}) -> só o nome é anunciado: "
          f"{sum(1 for l in lengths if l < MIN_DESC_LENGTH)}")
    print(f"  acima do teto de listagem ({MAX_LISTING_COMBINED_BYTES}): "
          f"{sum(1 for l in lengths if l > MAX_LISTING_COMBINED_BYTES)}")

    with_wtu = [s for s in skills if s["when_to_use"]]
    print(f"\n== when_to_use (divide os 400 bytes com a descrição) ==")
    print(f"  skills com when_to_use: {len(with_wtu)}/{len(skills)}")
    if with_wtu:
        wt = [len(s["when_to_use"]) for s in with_wtu]
        print(f"  comprimento: mediana {statistics.median(wt):.0f}, max {max(wt)}")
        worst = sorted(with_wtu, key=lambda s: -len(s["when_to_use"] or ""))[:5]
        for s in worst:
            desc_share = len(s["description"]) / (len(s["description"]) + len(s["when_to_use"]))
            print(f"    {s['name']:<28} desc fica com ~{desc_share:.0%} dos bytes")

    prefixed = [s for s in skills if s["description"].lower().startswith(tuple(TRIGGER_PREFIXES))]
    print(f"\n== prefixo de gatilho removido antes do anúncio ==")
    print(f"  descrições que começam com um prefixo reconhecido: {len(prefixed)}")
    for s in prefixed[:4]:
        print(f"    {s['name']:<28} {s['description'][:70]}")

    # name ambiguity: a name whose hyphen segments are a subset of another's
    names = [s["name"] for s in skills]
    parts = {n: set(n.split("-")) for n in names}
    ambiguous = defaultdict(list)
    for a in names:
        for b in names:
            if a != b and parts[a] < parts[b] and len(parts[a]) >= 1 and len(parts[b]) <= len(parts[a]) + 2:
                ambiguous[a].append(b)
    print(f"\n== ambiguidade por nome (guarda-chuva) ==")
    print(f"  nomes que são subconjunto de outro: {len(ambiguous)}")
    for name in sorted(ambiguous, key=lambda n: -len(ambiguous[n]))[:6]:
        print(f"    {name:<24} absorve {len(ambiguous[name])}: {', '.join(ambiguous[name][:4])}")

    print("\n== unicidade dos primeiros N bytes ==")
    for width in (40, 60, 100, 200):
        buckets = Counter(s["description"][:width] for s in skills)
        collided = sum(1 for s in skills if buckets[s["description"][:width]] > 1)
        empty = sum(1 for s in skills if len(s["description"][:width].strip()) == 0)
        print(f"  {width:>3} bytes: {collided} skills com abertura idêntica a outra, {empty} vazias")

    OUT.mkdir(exist_ok=True)
    (OUT / "skills-corpus-analysis.json").write_text(
        json.dumps(
            {
                "count": len(skills),
                "desc_len": {"min": min(lengths), "median": statistics.median(lengths), "max": max(lengths)},
                "with_when_to_use": len(with_wtu),
                "trigger_prefixed": len(prefixed),
                "ambiguous_names": {k: v for k, v in ambiguous.items()},
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"\n  -> {(OUT / 'skills-corpus-analysis.json').name}")


if __name__ == "__main__":
    main()