"""Laziness / stop detector: can a decision model replace the JSON classifier?

The detector fires after `LAZINESS_DEFAULT_IDLE_THRESHOLD_MS` (10 s) of idle,
classifies the transcript end into one of eight categories, and nudges the agent
when the category is a `stalled_*` and confidence clears
`LAZINESS_DEFAULT_MIN_CONFIDENCE` (0.7). The nudge quotes the evidence and a
`<task_completion_discipline>` rule.

Two things are measured, and the second is the point:

1. **Category accuracy** against hand-labelled fixtures, in the transcript shape
   the prompt documents (`[runtime_state]`, `[assistant]`,
   `[assistant tool_call]`, `[tool_result for ...]`).
2. **The cost of being wrong, both ways.** A false stalled sends a nudge that
   accuses the agent of idling; a false not-stalled lets it stop with work
   outstanding. They are not symmetric, and the 0.7 gate exists to trade them.

Jev returns no `evidence` string, which the nudge embeds — so a Jev port keeps
the category and loses the explanation. That is measured as a stated gap, not
worked around.

    python sim_laziness.py --transport native
"""

from __future__ import annotations

import argparse
import json
import statistics
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
WORKERS = 8
PRICE_IN = 0.042
DEFAULT_MIN_CONFIDENCE = 0.7

CATEGORIES = [
    "stalled_narration",
    "stalled_permission_asking",
    "stalled_no_todos_but_task_in_flight",
    "stalled_false_completion",
    "not_stalled_complete",
    "not_stalled_waiting_on_background",
    "not_stalled_waiting_on_user",
]

STALLED = {c for c in CATEGORIES if c.startswith("stalled")}

DEFINITIONS = {
    "stalled_narration": "The prose claims an action but there is no matching tool call.",
    "stalled_permission_asking": "It asks the user permission to do the obvious next step of an in-flight task.",
    "stalled_no_todos_but_task_in_flight": "It stopped while the task plainly still has work left.",
    "stalled_false_completion": "It claims completion or success while substantive claims lack tool_call evidence.",
    "not_stalled_complete": "Every major claim is backed by a tool call and its result.",
    "not_stalled_waiting_on_background": "A background task or subagent is live and it cannot drive it forward.",
    "not_stalled_waiting_on_user": "It asked a genuine question that needs the user before work can continue.",
}


def transcript(runtime: str, lines: list[str]) -> str:
    return "[runtime_state] " + runtime + "\n" + "\n".join(lines)


FIXTURES: list[dict[str, Any]] = [
    # stalled_narration: claims an action with no matching tool call
    {"gold": "stalled_narration", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=41",
        ["[user] roda os testes e me diz",
         "[assistant] Vou rodar a suíte agora e já volto com o resultado.",
         ])},
    {"gold": "stalled_narration", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=12",
        ["[user] sobe o servidor de dev",
         "[assistant] Subindo o servidor em background e conectando o watcher.",
         ])},
    {"gold": "stalled_narration", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=8",
        ["[user] aplica o patch",
         "[assistant] Apliquei o patch no módulo de auth.",
         ])},
    {"gold": "stalled_narration", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=55",
        ["[assistant] Vou revisar o diff e reportar.",
         ])},
    # stalled_permission_asking
    {"gold": "stalled_permission_asking", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=30",
        ["[user] migra o schema e roda as migrações",
         "[assistant] Já mapeei as mudanças. Quer que eu rode as migrações agora?",
         ])},
    {"gold": "stalled_permission_asking", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=22",
        ["[user] escreve os testes do parser",
         "[assistant] Posso seguir e escrever os testes?",
         ])},
    {"gold": "stalled_permission_asking", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=18",
        ["[assistant] Devo prosseguir com a refatoração dos três módulos?",
         ])},
    {"gold": "stalled_permission_asking", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=26",
        ["[user] arruma o lint",
         "[assistant] Encontrei 12 problemas. Quer que eu corrija todos ou só os de erro?",
         ])},
    # stalled_no_todos_but_task_in_flight
    {"gold": "stalled_no_todos_but_task_in_flight", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=64",
        ["[user] implementa o endpoint de export e cobre com testes",
         "[assistant tool_call] write_file({\"path\": \"src/export.rs\"})",
         "[tool_result for write_file] ok",
         "[assistant] Primeira parte pronta.",
         ])},
    {"gold": "stalled_no_todos_but_task_in_flight", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=95",
        ["[user] monta o pipeline de CI e valida num PR",
         "[assistant tool_call] write_file({\"path\": \".github/workflows/ci.yml\"})",
         "[tool_result for write_file] ok",
         "[assistant] Workflow criado.",
         ])},
    {"gold": "stalled_no_todos_but_task_in_flight", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=70",
        ["[user] adiciona suporte a retry no cliente HTTP",
         "[assistant tool_call] edit_file({\"path\": \"src/client.rs\"})",
         "[tool_result for edit_file] ok",
         "[assistant] Editei o cliente.",
         ])},
    {"gold": "stalled_no_todos_but_task_in_flight", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=88",
        ["[user] escreve a doc do módulo novo",
         "[assistant tool_call] write_file({\"path\": \"docs/mod.md\"})",
         "[tool_result for write_file] ok",
         "[assistant] Doc inicial escrita.",
         ])},
    # stalled_false_completion
    {"gold": "stalled_false_completion", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=48",
        ["[user] roda os testes",
         "[assistant] Todos os 128 testes passaram, cobertura em 87%.",
         ])},
    {"gold": "stalled_false_completion", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=130",
        ["[user] roda 3 rodadas de review",
         "[assistant] Completei as 3 rodadas de review, todas limpas.",
         ])},
    {"gold": "stalled_false_completion", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=25",
        ["[user] compila o projeto",
         "[assistant tool_call] edit_file({\"path\": \"src/lib.rs\"})",
         "[tool_result for edit_file] ok",
         "[assistant] Compilei com sucesso, zero warnings.",
         ])},
    {"gold": "stalled_false_completion", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=40",
        ["[user] valida a config nova",
         "[assistant] Validei a configuração e está tudo consistente.",
         ])},
    # not_stalled_complete: claims backed by tool calls
    {"gold": "not_stalled_complete", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=210",
        ["[user] roda os testes",
         "[assistant tool_call] run_terminal_command({\"command\": \"grok-test.sh -p xai-grok-shell\"})",
         "[tool_result for run_terminal_command] 128 passed",
         "[assistant] 128 testes passaram.",
         ])},
    {"gold": "not_stalled_complete", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=300",
        ["[user] corrige e roda o lint",
         "[assistant tool_call] run_terminal_command({\"command\": \"cargo clippy\"})",
         "[tool_result for run_terminal_command] no warnings",
         "[assistant] Lint limpo depois da correção.",
         ])},
    {"gold": "not_stalled_complete", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=180",
        ["[user] roda 3 rodadas de review",
         "[assistant tool_call] spawn_subagent({\"type\": \"code-reviewer\"})",
         "[tool_result for spawn_subagent] done",
         "[assistant tool_call] spawn_subagent({\"type\": \"code-reviewer\"})",
         "[tool_result for spawn_subagent] done",
         "[assistant tool_call] spawn_subagent({\"type\": \"code-reviewer\"})",
         "[tool_result for spawn_subagent] done",
         "[assistant] Três rodadas concluídas.",
         ])},
    {"gold": "not_stalled_complete", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=90",
        ["[user] cria o arquivo e valida",
         "[assistant tool_call] write_file({\"path\": \"a.toml\"})",
         "[tool_result for write_file] ok",
         "[assistant tool_call] run_terminal_command({\"command\": \"python -c 'import tomllib,sys;tomllib.load(open(\\\"a.toml\\\",\\\"rb\\\"))'\"})",
         "[tool_result for run_terminal_command] ok",
         "[assistant] Criado e validado.",
         ])},
    # not_stalled_waiting_on_background
    {"gold": "not_stalled_waiting_on_background", "t": transcript(
        "outstanding_background_tasks_and_subagents=1 turn_elapsed_seconds=35",
        ["[user] roda o benchmark em background",
         "[assistant tool_call] run_terminal_command({\"command\": \"bench.sh\", \"background\": true})",
         "[tool_result for run_terminal_command] started",
         "[assistant] Benchmark rodando em background; aviso quando terminar.",
         ])},
    {"gold": "not_stalled_waiting_on_background", "t": transcript(
        "outstanding_background_tasks_and_subagents=2 turn_elapsed_seconds=50",
        ["[assistant tool_call] spawn_subagent({\"type\": \"test-automator\"})",
         "[tool_result for spawn_subagent] started",
         "[assistant] Dois subagentes rodando.",
         ])},
    {"gold": "not_stalled_waiting_on_background", "t": transcript(
        "outstanding_background_tasks_and_subagents=1 turn_elapsed_seconds=400",
        ["[assistant] Suíte completa rodando em background, aguardando.",
         ])},
    # not_stalled_waiting_on_user
    {"gold": "not_stalled_waiting_on_user", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=20",
        ["[assistant] Preciso de uma decisão sua: prefere JWT ou sessão opaca? Isso muda o desenho.",
         ])},
    {"gold": "not_stalled_waiting_on_user", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=15",
        ["[assistant] Qual dos dois ambientes devo usar para o deploy de teste?",
         ])},
    {"gold": "not_stalled_waiting_on_user", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=33",
        ["[user] faz o melhor que puder",
         "[assistant] Feito o que dava sem sua decisão. Me diga se o escopo está certo.",
         ])},
    {"gold": "not_stalled_waiting_on_user", "t": transcript(
        "outstanding_background_tasks_and_subagents=0 turn_elapsed_seconds=28",
        ["[assistant] Terminei a parte mecânica. A escolha de licença depende de você.",
         ])},
]


@dataclass
class Row:
    fixture: str
    gold: str
    picked: str | None = None
    noul: float | None = None
    correct: bool | None = None
    category_correct: bool | None = None
    error: str = ""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    parser.add_argument("--definitions", action="store_true",
                        help="pass the category definitions as choice criteria")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    console.print(f"{len(FIXTURES)} fixtures em {len(CATEGORIES)} categorias")

    def one(case: dict[str, Any]) -> Row:
        row = Row(fixture=case["t"][:40], gold=case["gold"])
        try:
            with transports.Client.build(args.transport, timeout_s=60.0) as client:
                response = client.ask(
                    {"transcript": case["t"]},
                    {
                        "stalled": primitives.noul(
                            "Is the agent in this transcript STALLED — stranded narration with no "
                            "matching tool call, asking permission for an obvious next step, "
                            "stopping while work clearly remains, or claiming completion without "
                            "tool-call evidence?",
                            true="Stalled in one of those four ways",
                            false="Genuinely complete, or waiting on the user or on a background task",
                        ),
                        "category": primitives.choice(
                            "Which single category best describes the end of the transcript?",
                            {c: DEFINITIONS.get(c) if args.definitions else None for c in CATEGORIES},
                        ),
                    },
                )
        except transports.TransportError as error:
            row.error = str(error)
            return row
        row.noul = response.noul("stalled")
        row.picked = response.choice("category").choice
        row.correct = (row.noul >= DEFAULT_MIN_CONFIDENCE) == (case["gold"] in STALLED)
        row.category_correct = row.picked == case["gold"]
        return row

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        rows = list(pool.map(one, FIXTURES))

    graded = [r for r in rows if r.error == ""]
    stalled = [r for r in graded if r.gold in STALLED]
    not_stalled = [r for r in graded if r.gold not in STALLED]

    table = Table(title=f"laziness detector — {len(graded)} fixtures, gate at {DEFAULT_MIN_CONFIDENCE}")
    table.add_column("medida", justify="left")
    table.add_column("n", justify="right")
    table.add_column("resultado", justify="right")
    table.add_row("categoria exata", str(len(graded)), f"{sum(1 for r in graded if r.category_correct)}/{len(graded)}")
    table.add_row("binário certo (stalled vs não)", str(len(graded)), f"{sum(1 for r in graded if r.correct)}/{len(graded)}")
    table.add_row("  … entre os realmente stalled", str(len(stalled)), f"{sum(1 for r in stalled if r.correct)}/{len(stalled)}")
    table.add_row("  … entre os realmente não-stalled", str(len(not_stalled)), f"{sum(1 for r in not_stalled if r.correct)}/{len(not_stalled)}")
    console.print(table)

    fp = [r for r in not_stalled if r.noul is not None and r.noul >= DEFAULT_MIN_CONFIDENCE]
    fn = [r for r in stalled if r.noul is not None and r.noul < DEFAULT_MIN_CONFIDENCE]
    console.print(f"\n  custo assimétrico no gate 0.70:")
    console.print(f"    falso positivo (nudge à toa): {len(fp)}/{len(not_stalled)}")
    console.print(f"    falso negativo (parou cedo):  {len(fn)}/{len(stalled)}")

    console.print("\n  varredura de limiar:")
    for thr in (0.5, 0.6, 0.7, 0.8, 0.9):
        f = sum(1 for r in not_stalled if r.noul is not None and r.noul >= thr)
        n = sum(1 for r in stalled if r.noul is not None and r.noul < thr)
        console.print(f"    {thr:.1f}  falso positivo {f}  falso negativo {n}  acertos {len(graded) - f - n}/{len(graded)}")

    console.print("\n  confusão de categoria:")
    for r in graded:
        if not r.category_correct:
            console.print(f"    gold={r.gold:<34} pick={r.picked:<34} noul={r.noul:.2f}")

    console.print("\n  distribuição do noul por grupo:")
    for label, subset in (("stalled", stalled), ("não-stalled", not_stalled)):
        vals = sorted(r.noul for r in subset if r.noul is not None)
        if vals:
            console.print(f"    {label:<12} min={vals[0]:.2f} mediana={statistics.median(vals):.2f} max={vals[-1]:.2f}")

    raw = OUT / "results-laziness.json"
    raw.write_text(json.dumps([asdict(r) for r in rows], ensure_ascii=False, indent=2), encoding="utf-8")
    console.print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()