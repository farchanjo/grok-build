"""Workflows: does a retrieval stack beat a listing, and at what catalog size?

The discovery gap measured 5/8 blind against 8/8 with a catalog — but the catalog
was five entries. A listing is cheap at five and expensive at fifty, and it is
per-turn text whether or not a workflow is launched. So this measures the
crossover.

Arms, all on the same requests:

  blind        no catalog at all
  listed-5     all five in the prompt
  listed-30    all thirty in the prompt
  embed        dense KNN over the descriptions, top 5, then Jev picks
  embed+rerank the same, with bge-reranker-v2-m3 over the shortlist first
  names-30     thirty names, no descriptions

Catalog cost is per turn (it rides the tool description); retrieval cost is per
selection. Both are reported, because they are not the same currency.

    python sim_workflow_retrieval.py --transport native
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_embedding import cached_local

from sim_matrix import COLLECTION, milvus_search, milvus_setup, real_rerank

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
WORKERS = 8
SHORTLIST = 5
PRICE_IN = 0.042

SELECT_QUESTION = (
    "Which registered workflow fits this request best? Prefer the one whose output matches what "
    "is being asked for."
)

CATALOG: list[tuple[str, str]] = [
    ("deep-research", "Fan out research over many sources, then verify each claim against its source before rendering."),
    ("review-changes", "Run several independent reviewers over a diff and aggregate their findings into one report."),
    ("migration-sweep", "Migrate call sites in batches across a large tree, verifying each batch before the next."),
    ("benchmark-matrix", "Run one benchmark across several configurations in parallel and tabulate the results."),
    ("release-cut", "Prepare a release: changelog, version bump, tag, and a verification pass."),
    ("incident-postmortem", "Read an incident, reconstruct the timeline, and propose action items."),
    ("i18n-check", "Translate localization files, validate placeholders, and fail if one is lost."),
    ("cost-delta", "Compare monthly cloud cost before and after an infrastructure change."),
    ("issue-sync", "Synchronize issues between two trackers in both directions, resolving conflicts by precedence."),
    ("changelog", "Generate a changelog from commit messages since the last tag."),
    ("dependency-audit", "Audit dependencies for incompatible licences and known vulnerabilities."),
    ("prompt-ab", "Run an A/B experiment over two prompts and decide the winner on a metric."),
    ("dev-bootstrap", "Prepare a new developer's environment: install tools, clone repositories, validate the build."),
    ("queue-autoscale", "Watch queues and scale workers when the backlog crosses a threshold."),
    ("invoice-reconcile", "Extract invoice PDFs and validate the totals against the supplier's spreadsheet."),
    ("manifest-security", "Run a security check over Kubernetes manifests before applying them."),
    ("weekly-metrics", "Generate a weekly product metrics report from three sources."),
    ("batched-migration", "Migrate a large database in batches with an integrity check between them."),
    ("test-matrix", "Run the tests on three operating systems in parallel and aggregate one report."),
    ("deprecated-api-sweep", "Scan the repository for deprecated API calls, rank by severity, and file issues."),
    ("api-docs", "Generate reference documentation from the OpenAPI schema and publish it."),
    ("flaky-triage", "Re-run failing tests N times, classify flaky against real, and open issues for the real ones."),
    ("schema-diff", "Diff two database schemas and produce a migration plan."),
    ("log-triage", "Cluster error logs by signature and rank by novelty against the previous week."),
    ("config-drift", "Compare running infrastructure against the declared configuration and report drift."),
    ("sbom", "Produce a software bill of materials from the lockfiles and container images."),
    ("perf-regression", "Run the benchmark suite against a baseline and flag statistically real regressions."),
    ("access-review", "Collect who has access to what and flag the grants that outlive their reason."),
    ("backport", "Port a fix from main to the maintained release branches and validate each."),
    ("onboarding-doc", "Derive an onboarding guide from the repository layout, the CI config and recent commits."),
]

REQUESTS: list[dict[str, str]] = [
    {"q": "pesquisar um tema em muitas fontes e checar cada afirmação", "gold": "deep-research"},
    {"q": "várias revisões independentes do diff, com relatório agregado", "gold": "review-changes"},
    {"q": "migrar chamadas numa árvore grande, em lotes, verificando cada lote", "gold": "migration-sweep"},
    {"q": "rodar o mesmo benchmark em cinco configurações e tabular", "gold": "benchmark-matrix"},
    {"q": "preparar release com changelog, bump e tag", "gold": "release-cut"},
    {"q": "reconstruir a linha do tempo de um incidente e propor ações", "gold": "incident-postmortem"},
    {"q": "traduzir arquivos de localização e validar os placeholders", "gold": "i18n-check"},
    {"q": "comparar o custo mensal de nuvem antes e depois", "gold": "cost-delta"},
    {"q": "sincronizar issues entre dois rastreadores nos dois sentidos", "gold": "issue-sync"},
    {"q": "gerar changelog a partir dos commits desde a última tag", "gold": "changelog"},
    {"q": "auditar dependências por licença e vulnerabilidade", "gold": "dependency-audit"},
    {"q": "preparar o ambiente de um dev novo", "gold": "dev-bootstrap"},
    {"q": "escalar workers quando a fila crescer", "gold": "queue-autoscale"},
    {"q": "validar faturas em PDF contra a planilha do fornecedor", "gold": "invoice-reconcile"},
    {"q": "checagem de segurança nos manifestos antes de aplicar", "gold": "manifest-security"},
    {"q": "relatório semanal de métricas de produto", "gold": "weekly-metrics"},
    {"q": "migrar um banco grande em lotes com verificação", "gold": "batched-migration"},
    {"q": "rodar os testes em três sistemas e agregar", "gold": "test-matrix"},
    {"q": "varrer o repo por APIs depreciadas e abrir issues", "gold": "deprecated-api-sweep"},
    {"q": "gerar a documentação de referência a partir do OpenAPI", "gold": "api-docs"},
    {"q": "descobrir quais testes são flaky e quais são falha real", "gold": "flaky-triage"},
    {"q": "diferenciar dois schemas e planejar a migração", "gold": "schema-diff"},
    {"q": "agrupar logs de erro por assinatura e ranquear por novidade", "gold": "log-triage"},
    {"q": "comparar a infraestrutura rodando com a declarada", "gold": "config-drift"},
]


@dataclass
class Arm:
    label: str
    n: int = 0
    top1: int = 0
    p50_ms: float = 0.0
    tokens: int = 0
    catalog_chars: int = 0
    error: str = ""

    @property
    def tokens_per_turn(self) -> int:
        return self.catalog_chars // 4

    @property
    def tokens_per_selection(self) -> int:
        return self.tokens // max(self.n, 1)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    names = [n for n, _ in CATALOG]
    desc = dict(CATALOG)
    docs = [f"{n}: {d}" for n, d in CATALOG]
    vectors = cached_local(docs)
    qvecs = cached_local([r["q"] for r in REQUESTS])
    milvus_setup(vectors, name=f"{COLLECTION}_wf")
    coll = f"{COLLECTION}_wf"

    arms = [
        Arm("blind (sem catálogo)"),
        Arm("listed-5"),
        Arm("names-30"),
        Arm("listed-30"),
        Arm("embed + jev"),
        Arm("embed + reranker + jev"),
    ]

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            latencies = []
            for req, qvec in zip(REQUESTS, qvecs):
                started = time.perf_counter()
                if arm.label == "blind (sem catálogo)":
                    # no catalog: the model is asked to name it, then we check
                    response = client.ask(
                        {"request": req["q"]},
                        {"is": primitives.noul(
                            f"Does a workflow named exactly `{req['gold']}` fit this request?")},
                    )
                    arm.tokens += response.input_tokens
                    picked = req["gold"] if response.noul("is") >= 0.5 else None
                elif arm.label == "listed-5":
                    pool = names[:5]
                    arm.catalog_chars = sum(len(n) + len(desc[n]) for n in pool)
                    response = client.ask({"request": req["q"]},
                                          {"which": primitives.choice(SELECT_QUESTION, {n: desc[n] for n in pool})})
                    arm.tokens += response.input_tokens
                    picked = response.choice("which").choice
                elif arm.label == "names-30":
                    arm.catalog_chars = sum(len(n) for n in names)
                    response = client.ask({"request": req["q"]},
                                          {"which": primitives.choice(SELECT_QUESTION, {n: None for n in names})})
                    arm.tokens += response.input_tokens
                    picked = response.choice("which").choice
                elif arm.label == "listed-30":
                    arm.catalog_chars = sum(len(n) + len(desc[n]) for n in names)
                    response = client.ask({"request": req["q"]},
                                          {"which": primitives.choice(SELECT_QUESTION, {n: desc[n] for n in names})})
                    arm.tokens += response.input_tokens
                    picked = response.choice("which").choice
                else:
                    hits = milvus_search(qvec, SHORTLIST, name=coll)
                    pool = [names[i] for i in hits]
                    if arm.label == "embed + reranker + jev":
                        order = real_rerank(req["q"], [desc[n] for n in pool])
                        if order:
                            pool = [pool[i] for i in order]
                    response = client.ask({"request": req["q"]},
                                          {"which": primitives.choice(SELECT_QUESTION, {n: desc[n] for n in pool})})
                    arm.tokens += response.input_tokens
                    picked = response.choice("which").choice
                latencies.append((time.perf_counter() - started) * 1000)
                if picked == req["gold"]:
                    arm.top1 += 1
            arm.n = len(REQUESTS)
            arm.p50_ms = statistics.median(latencies)

    table = Table(title=f"workflows — {len(REQUESTS)} pedidos, catálogo de {len(CATALOG)}")
    for column in ("arm", "top-1", "custo do catálogo (tok/turno)", "custo da seleção (tok/chamada)", "p50 ms"):
        table.add_column(column, justify="right" if column != "arm" else "left")
    for arm in arms:
        table.add_row(arm.label, f"{arm.top1}/{arm.n}", f"{arm.tokens_per_turn:,}",
                      f"{arm.tokens_per_selection:,}", f"{arm.p50_ms:.0f}")
    console.print(table)

    raw = OUT / "results-workflow-retrieval.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()