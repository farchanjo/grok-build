"""Improve execute-jev-plan.rhai by giving every phase a specialist.

The generated workflow spawns generic agents: `agent(prompt, #{label, capability_mode,
output_schema, phase})` with no `agent_type`. The harness has 109 registered
specialists and a `## Delegates` graph, and the workflow reaches neither.

This assigns a type per phase using the stack, then patches the script:

* **embedding** ranks the 109 agent descriptions against the phase's intent
* **reranker** reorders the shortlist
* **jev** picks one from the shortlist, with the phase's brief as context
* a **validator** re-checks the patched script: the type must exist, and the meta
  rules and phase references must still hold

Nothing here builds; the script is text.

    python improve_plan_workflow.py --transport native
"""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import asdict, dataclass, field
from pathlib import Path

from jev import primitives, transports
from sim_embedding import cached_local
from sim_matrix import real_rerank
from sim_memory_pipeline import cosine

ROOT = Path(__file__).parent
AGENTS = Path("~/.grok/agents").expanduser()
TARGET = ROOT / "execute-jev-plan.rhai"
OUT = ROOT / "out"
SHORTLIST = 5

# What each phase is about, in the words of the plan. These are the retrieval
# queries; the phase brief in the script is what the agent reads.
# Keyed by the agent's `label`, not its phase: the four parallel jobs share one
# phase and would otherwise all get the same type.
PHASE_INTENT = {
    "preflight": "read-only reconnaissance of a codebase: look around, report what is there, change nothing",
    "phase0": "fix known bugs in a systems-level Rust library, small surgical changes with failure-path tests",
    "phase1": "integrate an external model provider and a vector index into existing search code",
    "skills": "tune a retrieval-and-ranking pipeline over a catalogue of markdown documents, and write its eval cases",
    "memory": "fix a storage layer: file indexing, merging, routing between scopes, and a write-time gate",
    "permission-laziness": "tune two thresholded classifiers and their calibration against labelled samples",
    "agents-todo-workflow-goal": "fix metadata across a catalogue of agent definitions and add a new host function to a scripting engine",
    "correctness": "review a batch of changes for whether they actually landed and whether the rules held",
    "transport-tui": "add configuration plumbing and terminal UI surfaces to a Rust application",
    "verify": "build and test a Rust workspace, one crate at a time, and report failures",
}

PICK_QUESTION = (
    "A workflow phase is about to run. Which registered subagent is the best fit to carry it out? "
    "The phase brief is in the state; the agent will be spawned with this type."
)


@dataclass
class Choice:
    phase: str
    intent: str
    picked: str
    shortlist: list[str] = field(default_factory=list)
    confidence: float = 0.0


def phase_briefs(script: str) -> dict[str, str]:
    """The brief text per phase, taken from the label that follows it."""
    briefs: dict[str, str] = {}
    for match in re.finditer(r'let brief = SHARED \+ "\\n\\n"\s*\n\s*\+ (.*?);\n\s*let res = agent', script, re.DOTALL):
        text = match.group(1)
        briefs[text] = text
    # the parallel jobs carry phase: near the end of each map
    return briefs


def current_phase_labels(script: str) -> list[str]:
    return re.findall(r'phase:\s*"([^"]+)"', script)


def assign(client: transports.Client, names: list[str], desc: dict[str, str], vectors) -> list[Choice]:
    choices: list[Choice] = []
    for phase, intent in PHASE_INTENT.items():
        qvec = cached_local([intent])[0]
        ranked = sorted(((cosine(qvec, vectors[i]), i) for i in range(len(names))), key=lambda kv: -kv[0])
        short = [names[i] for _, i in ranked[:12]]
        order = real_rerank(intent, [desc[n] for n in short])
        if order:
            short = [short[i] for i in order]
        short = short[:SHORTLIST]
        response = client.ask(
            {"label": phase, "brief": intent},
            {"which": primitives.choice(PICK_QUESTION, {n: desc[n][:200] for n in short})},
        )
        choices.append(
            Choice(
                phase=phase,
                intent=intent,
                picked=response.choice("which").choice,
                shortlist=short,
                confidence=response.choice("which").confidence,
            )
        )
    return choices


def patch(script: str, choices: list[Choice]) -> tuple[str, int]:
    """Insert `agent_type` into the agent opts of each phase."""
    by_phase = {c.phase: c.picked for c in choices}
    out, count = script, 0

    def add(match: re.Match) -> str:
        nonlocal count
        block = match.group(0)
        label = re.search(r'label:\s*"([^"]+)"', block)
        if not label or label.group(1) not in by_phase:
            return block
        if "agent_type:" in block:
            return block
        count += 1
        indent = re.search(r'\n(\s+)capability_mode:', block)
        pad = indent.group(1) if indent else "        "
        return block.replace("capability_mode:", f'agent_type: "{by_phase[label.group(1)]}",\n{pad}capability_mode:', 1)

    # agent opts appear both as `agent(prompt, #{...})` and as `jobs.push(#{...})`;
    # both end with the phase key followed by a closing brace.
    # a value may continue on the next line with `+ "..."`, so a line is either
    # `key:` or a `+` continuation
    out = re.sub(r'#\{\n(?:\s+(?:\w+:|\+).*\n)+?\s+phase:\s*"[^"]+",\n\s*\}', add, script)
    return out, count


def validate(script: str, known: set[str]) -> list[str]:
    problems: list[str] = []
    NAME_OK = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
    meta = re.search(r"let meta = #\{(.*?)\n\};", script, re.DOTALL)
    if not meta:
        return ["meta block lost"]
    body = meta.group(1)
    name = re.search(r'name:\s*"([^"]*)"', body)
    if not name or not NAME_OK.match(name.group(1)) or len(name.group(1).encode()) > 64:
        problems.append("meta.name invalid")
    titles = re.findall(r'title:\s*"([^"]*)"', body)
    if len(set(titles)) != len(titles):
        problems.append("duplicate phase titles")
    for used in set(re.findall(r'phase:\s*"([^"]+)"', script)) | set(re.findall(r'phase\("([^"]+)"\)', script)):
        if used not in titles:
            problems.append(f'phase "{used}" not declared')
    for used in set(re.findall(r'agent_type:\s*"([^"]+)"', script)):
        if used not in known:
            problems.append(f'agent_type "{used}" is not a registered agent')
    if "complete(" not in script:
        problems.append("complete() lost")
    return problems


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    parser.add_argument("--write", action="store_true", help="patch the file in place")
    parser.add_argument("--floor", type=float, default=0.50,
                        help="confidence below which the phase keeps general-purpose")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    script = TARGET.read_text(encoding="utf-8")
    roster = json.loads((ROOT / "cases" / "agents.json").read_text(encoding="utf-8"))
    known = {a["name"] for a in roster} | {"general-purpose", "explore", "plan"}
    names = sorted(known)
    desc = {a["name"]: a["description"] for a in roster}
    desc.update({"general-purpose": "General purpose agent for multi-step tasks; all tools.",
                 "explore": "Fast read-only agent for codebase exploration.",
                 "plan": "Software architect for planning; read-only."})
    vectors = cached_local([f"{n}: {desc[n]}" for n in names])

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        choices = assign(client, names, desc, vectors)

    # Below the floor the specialist is a stretch, so the phase keeps
    # general-purpose. The confidence tracked the arguable picks in every run.
    kept = [c for c in choices if c.confidence >= args.floor]
    dropped = [c for c in choices if c.confidence < args.floor]
    patched, added = patch(script, kept)
    problems = validate(patched, known)

    print(f"agentes no registry: {len(known)} | piso de confiança: {args.floor}")
    print(f"especializados: {added} | mantidos em general-purpose: {len(dropped)}")
    print()
    for c in choices:
        mark = "esp " if c.confidence >= args.floor else "gen "
        print(f"  {mark}{c.phase:<28} -> {c.picked:<24} conf={c.confidence:.2f}")
        print(f"      shortlist: {', '.join(c.shortlist[:4])}")
    print()
    print(f"validação do script corrigido: {'LIMPA' if not problems else str(len(problems)) + ' problema(s)'}")
    for p in problems:
        print(f"  - {p}")

    if args.write and not problems:
        TARGET.write_text(patched, encoding="utf-8")
        print(f"\n  escrito em {TARGET.name} ({len(patched)} bytes)")

    (OUT / "workflow-improvement.json").write_text(
        json.dumps({"choices": [asdict(c) for c in choices], "added": added, "problems": problems},
                   ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"  -> {(OUT / 'workflow-improvement.json').name}")


if __name__ == "__main__":
    main()