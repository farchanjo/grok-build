"""Where the agent corpus can be improved.

Read-only. Each check maps to a decision the harness makes.

1. **Mode versus description.** `capabilityMode` is `read-write` on 107 and `all`
   on 2. An agent described as read-only or as a reviewer should not be able to
   write, and one described as an implementer should not be read-only. A mismatch
   is a correctness bug the corpus can show.
2. **Synonym clusters.** 448 name pairs share a token; the tight ones are where a
   chooser confuses two agents. Cluster by shared tokens and rank by tightness.
3. **Dangling delegate targets.** The graph points at names that are not agents.
4. **Description discriminability.** Does each description carry the token that
   distinguishes it from its nearest sibling?

    python analyze_agents_improve.py
"""

from __future__ import annotations

import json
import re
import statistics
from collections import Counter, defaultdict
from pathlib import Path

AGENTS = Path("~/.grok/agents").expanduser()
OUT = Path(__file__).parent / "out"

READONLY_HINT = re.compile(r"\bread[- ]only\b|\breviewer\b|\bauditor\b|\bplanning\b|\bexplor", re.I)
WRITE_HINT = re.compile(r"\bimplement|\bbuild\b|\bwrite|\bdevelop|\bauthor", re.I)


def frontmatter(path: Path) -> dict[str, str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    fields = {}
    for line in text[3:end].splitlines():
        m = re.match(r"^([a-zA-Z_]+):\s*(.*)$", line)
        if m:
            fields[m.group(1)] = m.group(2).strip().strip('"')
    return fields


def delegates(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    m = re.search(r"^## Delegates\s*\n+([^\n#]+)", text, re.MULTILINE)
    return re.findall(r"->\s*([a-z0-9-]+)", m.group(1)) if m else []


def main() -> None:
    files = sorted(AGENTS.glob("*.md"))
    names = {f.stem for f in files}
    data = {f.stem: frontmatter(f) for f in files}
    desc = {n: data[n].get("description", "") for n in names}
    mode = {n: data[n].get("capabilityMode", "?") for n in names}

    print("== 1. modo versus descrição ==")
    mismatches = []
    for n in sorted(names):
        d = desc[n]
        ro = bool(READONLY_HINT.search(d))
        wr = bool(WRITE_HINT.search(d))
        if ro and not wr and mode[n] != "read-only":
            mismatches.append((n, mode[n], "descrição soa read-only"))
        if wr and not ro and mode[n] == "read-only":
            mismatches.append((n, mode[n], "descrição soa implementador"))
    print(f"  agentes com modo read-write/all: {sum(1 for n in names if mode[n] != 'read-only')}")
    print(f"  possíveis descompassos: {len(mismatches)}")
    for n, m, why in mismatches[:12]:
        print(f"    {n:<30} mode={m:<12} {why}")
        print(f"      {desc[n][:100]}")

    print("\n== 2. clusters de sinônimos (pares que compartilham token) ==")
    parts = {n: set(n.split("-")) for n in names}
    tight = []
    for a in sorted(names):
        for b in sorted(names):
            if a < b and parts[a] & parts[b]:
                shared = parts[a] & parts[b]
                tight.append((len(shared), a, b, sorted(shared)))
    tight.sort(key=lambda x: -x[0])
    by_token: dict[str, list[str]] = defaultdict(list)
    for n in names:
        for p in parts[n]:
            by_token[p].append(n)
    big = sorted(((t, v) for t, v in by_token.items() if len(v) >= 4), key=lambda kv: -len(kv[1]))
    print(f"  tokens que agrupam 4+ agentes: {len(big)}")
    for token, group in big[:8]:
        print(f"    {token:<16} {len(group)}: {', '.join(sorted(group)[:6])}")

    print("\n== 3. alvos de delegação que não resolvem ==")
    targets = Counter()
    for n in names:
        for t in delegates(AGENTS / f"{n}.md"):
            targets[t] += 1
    dangling = sorted((t for t in targets if t not in names), key=lambda t: -targets[t])
    print(f"  {len(dangling)} alvos, {sum(targets[t] for t in dangling)} arestas apontando para o vazio")
    for t in dangling:
        guess = [n for n in names if n.startswith(t + "-") or n == t]
        print(f"    -> {t:<14} ({targets[t]}x)  candidato: {guess[0] if guess else '—'}")

    print("\n== 4. discriminabilidade da descrição ==")
    missing = []
    for n in sorted(names):
        siblings = [o for o in names if o != n and parts[n] & parts[o]]
        if not siblings:
            continue
        unique = [p for p in parts[n] if all(p not in parts[s] for s in siblings)]
        d = desc[n].lower()
        if unique and not any(u in d for u in unique):
            missing.append((n, unique[:2], siblings[:2]))
    print(f"  agentes cujo token único não aparece na própria descrição: {len(missing)}")
    for n, uniq, sib in missing[:10]:
        print(f"    {n:<28} token único {uniq}  vs irmãos {sib}")

    lens = [len(desc[n]) for n in names]
    print(f"\n  descrições: mediana {statistics.median(lens):.0f}, min {min(lens)}, max {max(lens)}")

    OUT.mkdir(exist_ok=True)
    (OUT / "agents-improve.json").write_text(
        json.dumps(
            {
                "mode_mismatches": [{"agent": n, "mode": m, "why": w} for n, m, w in mismatches],
                "dangling_targets": {t: targets[t] for t in dangling},
                "indiscriminate": [{"agent": n, "unique": u} for n, u, _ in missing],
                "token_clusters": {t: sorted(v) for t, v in big},
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    print("\n  -> agents-improve.json")


if __name__ == "__main__":
    main()