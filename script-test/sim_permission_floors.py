"""How much of the permission decision the classifier actually controls.

The gate is layered, and the classifier is only one layer:

  fast-path (free)  allowlisted access/tool, every file edit, exact no-ops
  classifier        LLM, with the heuristic as fallback on Unavailable
  floors            four structural predicates that override a classifier Allow
  prompt            the user decides

The floors are `bash_{write,unsafe_env,opaque_shell,exec}_floor_requires_prompt`
(manager.rs:1003-1020): each fires on a structural property of the command
unless there is an exact grant. When one fires, **no classifier improvement
changes the outcome** — the user is prompted regardless.

So the headroom is the residual: commands that reach the classifier and are not
floored. This measures that residual on the 127-command corpus mined from the
engine's own tests.

Floor predicates are approximated here by regex over the command text, not by
running the tree-sitter parser. They are labelled as such.

    python sim_permission_floors.py
"""

from __future__ import annotations

import json
import re
from pathlib import Path

from rich.console import Console
from rich.table import Table

ROOT = Path(__file__).parent
CASES = ROOT / "cases" / "command_cases.json"
console = Console()


def unescape(command: str) -> str:
    return command.replace("\\n", "\n").replace('\\"', '"').replace("\\'", "'")


# ── approximate floor predicates ───────────────────────────────────────────

ENV_PREFIX = re.compile(r"^(?:env\s|(?:[A-Za-z_][A-Za-z0-9_]*=\S*\s)+)")
OPAQUE = re.compile(r"<<|\$\(|`|^(?:bash|sh|zsh)\s+-c\b")
WRITES = re.compile(r"(?<![|&])>{1,2}(?!&)\s*(?!/dev/null)\S|\btee\b|\b(?:cp|mv|rm|install|mkdir|touch|sed -i)\b")
EXEC_RISK = re.compile(r"\bgit\b|\bxargs\b|\bfind\b.*-exec|\bmake\b|\bcargo\b\s+(?:run|build|test)")


def floors(command: str) -> list[str]:
    hits = []
    if WRITES.search(command):
        hits.append("write")
    if ENV_PREFIX.search(command):
        hits.append("unsafe_env")
    if OPAQUE.search(command):
        hits.append("opaque_shell")
    if EXEC_RISK.search(command):
        hits.append("exec")
    return hits


def main() -> None:
    cases = json.loads(CASES.read_text(encoding="utf-8"))
    commands = [{"cmd": unescape(c["command"]), "gold": c["gold"]} for c in cases]

    per_floor: dict[str, int] = {}
    floored = 0
    floored_but_allowed = 0
    clean: list[dict[str, str]] = []

    for case in commands:
        hits = floors(case["cmd"])
        if hits:
            floored += 1
            for name in hits:
                per_floor[name] = per_floor.get(name, 0) + 1
            if case["gold"] == "allow":
                floored_but_allowed += 1
        else:
            clean.append(case)

    total = len(commands)
    allowed = sum(1 for c in commands if c["gold"] == "allow")

    table = Table(title=f"floor coverage — {total} commands ({allowed} policy-allow)")
    table.add_column("medida", justify="left")
    table.add_column("n", justify="right")
    table.add_column("% do corpus", justify="right")
    table.add_row("comandos que batem em algum piso", str(floored), f"{floored / total:.0%}")
    table.add_row("  … e que a política permite", str(floored_but_allowed), f"{floored_but_allowed / allowed:.0%} dos allow")
    table.add_row("residual: chega ao classificador sem piso", str(len(clean)), f"{len(clean) / total:.0%}")
    for name, count in sorted(per_floor.items(), key=lambda kv: -kv[1]):
        table.add_row(f"  piso: {name}", str(count), f"{count / total:.0%}")
    console.print(table)

    print(f"\nno residual de {len(clean)} comandos:")
    print(f"  política allow: {sum(1 for c in clean if c['gold'] == 'allow')}")
    print(f"  política block: {sum(1 for c in clean if c['gold'] == 'block')}")

    if clean:
        console.print("\n  amostra do residual (onde o classificador de fato decide):")
        for case in clean[:12]:
            print(f"    [{case['gold']:<5}] {case['cmd'][:70]}")

    console.print(
        f"\n  leitura: em {floored / total:.0%} do corpus o veredito do classificador é "
        f"anulado por um piso. Melhorar o classificador só move os {len(clean)} restantes."
    )
    console.print(
        f"  dos {allowed} comandos que a política permite, {floored_but_allowed} "
        f"({floored_but_allowed / allowed:.0%}) são promptados por piso independentemente do que o modelo diga."
    )

    raw = ROOT / "out" / "results-permission-floors.json"
    raw.parent.mkdir(exist_ok=True)
    raw.write_text(
        json.dumps(
            {
                "total": total,
                "floored": floored,
                "floored_but_allowed": floored_but_allowed,
                "per_floor": per_floor,
                "residual": [c["cmd"] for c in clean],
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"  -> {raw.name}")


if __name__ == "__main__":
    main()