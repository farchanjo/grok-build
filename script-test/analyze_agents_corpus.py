"""The agent corpus, read-only.

Three things the corpus carries that nothing consumes, and one it lacks:

* **`## Delegates` maps** — hand-written routing (`review -> code-reviewer;
  qa -> qa-expert`). 108 of 109 agents have one. No code reads it.
* **`skills: []`** on every agent — the field exists and is empty everywhere.
* **`capabilityMode`** — present on all 109.
* **No `model` and no `reasoning_effort`** on any agent, which is why the effort
  question lands on the parent (`FINDINGS-AGENTS.md` §4).

    python analyze_agents_corpus.py
"""

from __future__ import annotations

import json
import re
import statistics
from collections import Counter, defaultdict
from pathlib import Path

AGENTS = Path("~/.grok/agents").expanduser()
OUT = Path(__file__).parent / "out"
console = print

FIELD = re.compile(r"^([a-zA-Z_]+):\s*(.*)$")


def frontmatter(path: Path) -> dict[str, str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    fields: dict[str, str] = {}
    for line in text[3:end].splitlines():
        match = FIELD.match(line)
        if match:
            fields[match.group(1)] = match.group(2).strip().strip('"')
    return fields


def delegates(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    match = re.search(r"^## Delegates\s*\n+([^\n#]+)", text, re.MULTILINE)
    if not match:
        return []
    return re.findall(r"->\s*([a-z0-9-]+)", match.group(1))


def main() -> None:
    files = sorted(AGENTS.glob("*.md"))
    names = {f.stem for f in files}
    fields = {f.stem: frontmatter(f) for f in files}

    descs = [fields[n].get("description", "") for n in names]
    modes = Counter(fields[n].get("capabilityMode", "?") for n in names)
    skill_lists = Counter(fields[n].get("skills", "?") for n in names)
    with_model = [n for n in names if fields[n].get("model")]
    with_effort = [n for n in names if fields[n].get("reasoning_effort")]

    console(f"agentes: {len(names)}")
    console(f"  capabilityMode: {dict(modes)}")
    console(f"  campo skills: {dict(skill_lists)}")
    console(f"  com model: {len(with_model)} | com reasoning_effort: {len(with_effort)}")
    console(f"  descrição: mediana {statistics.median(len(d) for d in descs):.0f} chars, "
            f"min {min(len(d) for d in descs)}, max {max(len(d) for d in descs)}")

    graph = {n: delegates(AGENTS / f"{n}.md") for n in names}
    edges = sum(len(v) for v in graph.values())
    sources = sum(1 for v in graph.values() if v)
    targets = Counter(t for v in graph.values() for t in v)
    dangling = sorted({t for t in targets if t not in names})

    console(f"\ngrafo de delegação:")
    console(f"  agentes com arestas: {sources}/{len(names)}")
    console(f"  arestas: {edges}")
    console(f"  alvos distintos: {len(targets)}")
    console(f"  alvos que não resolvem: {len(dangling)} {dangling[:8]}")
    console(f"  nós sem entrada (ninguém delega para eles): "
            f"{sum(1 for n in names if n not in targets)}")

    # reachability from the most-cited node
    roots = [n for n in names if n not in targets][:5]
    seen: set[str] = set()
    stack = list(roots)
    while stack:
        node = stack.pop()
        if node in seen:
            continue
        seen.add(node)
        stack.extend(graph.get(node, []))
    console(f"  alcançáveis a partir de {len(roots)} raízes: {len(seen)}/{len(names)}")

    # cycles
    cycles = []
    for node in names:
        stack, path = [(node, [node])], []
        while stack:
            cur, trail = stack.pop()
            for nxt in graph.get(cur, []):
                if nxt == node and len(trail) > 1:
                    cycles.append(trail + [nxt])
                elif nxt not in trail and len(trail) < 6:
                    stack.append((nxt, trail + [nxt]))
    console(f"  ciclos encontrados: {len(cycles)} {cycles[:3]}")

    # near-synonym names: share a token
    parts = {n: set(n.split("-")) for n in names}
    pairs = []
    for a in names:
        for b in names:
            if a < b and parts[a] & parts[b]:
                pairs.append((a, b, sorted(parts[a] & parts[b])))
    console(f"\nnomes que compartilham um token: {len(pairs)} pares")
    for a, b, shared in pairs[:10]:
        console(f"    {a:<28} {b:<28} {shared}")

    OUT.mkdir(exist_ok=True)
    (OUT / "agents-corpus-analysis.json").write_text(
        json.dumps(
            {
                "count": len(names),
                "with_model": with_model,
                "with_effort": with_effort,
                "edges": edges,
                "sources": sources,
                "dangling": dangling,
                "cycles": cycles[:10],
                "graph": {k: v for k, v in graph.items() if v},
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    console(f"\n  -> agents-corpus-analysis.json")


if __name__ == "__main__":
    main()