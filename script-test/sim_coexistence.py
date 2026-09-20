"""Coexistence: native MEMORY.md + Milvus mirror + the Jev stages, all at once.

Three things have to keep working, and none may be removed:

* **MEMORY.md stays the human surface.** It is markdown, hand-edited, watched,
  and the format a person reads. Writes must go through the append path
  (`append_to_memory`, which normalizes to `## heading` and never truncates),
  never through `write_long_term` (which rewrites the file wholesale).
* **Milvus stays a disposable mirror.** SQLite remains the vector authority; the
  mirror is best-effort fan-out with sqlite-vec fallback. Nothing here talks to
  Milvus directly, so nothing here can depend on it being up.
* **With Jev off, behaviour must equal today.** The stages are additive: no gate
  means store everything, no scope means global, no rerank means RRF order.

This runs both arms over the same event stream with a hand edit interleaved, then
reindexes and queries. It checks the hand edit survives, that the format stays
valid, and what the queries get back.

    python sim_coexistence.py --transport native
"""

from __future__ import annotations

import argparse
import json
import math
import re
import statistics
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_memory_pipeline import (
    ADMIT_THRESHOLD,
    COVERED_QUESTION,
    COVERED_THRESHOLD,
    EVENTS,
    GATE_FALSE,
    GATE_QUESTION,
    GATE_TRUE,
    OUT,
    QUERIES,
    RERANK_FALSE,
    RERANK_QUESTION,
    RERANK_TRUE,
    SCOPE_CRITERIA,
    SCOPE_QUESTION,
    WORKSPACE_PATH,
    cached,
    embed_key,
    retrieve,
)

console = Console()
SHORTLIST = 10

HAND_EDIT = "## Hand-written note\n\nOwner added this by hand in the editor: the vLLM bench box is qwen38."


def normalize(raw: str) -> str:
    """Mirror of `normalize_memory_content`: a bare line becomes a heading."""
    trimmed = raw.strip()
    if not trimmed:
        return ""
    if trimmed.startswith("#"):
        return trimmed
    if "\n" not in trimmed:
        return f"## {trimmed}"
    first, _, rest = trimmed.partition("\n")
    if len(first) <= 80:
        return f"## {first.strip()}\n\n{rest.strip()}"
    return f"## Note\n\n{trimmed}"


def append(buffer: str, content: str) -> str:
    """Mirror of `append_to_memory`: blank-line separator, never truncates."""
    normalized = normalize(content)
    if not normalized:
        return buffer
    return normalized if not buffer else f"{buffer}\n\n{normalized}"


def chunks(buffer: str) -> list[str]:
    """Section-level chunks, the granularity the indexer works at."""
    parts = [p.strip() for p in re.split(r"\n(?=##? )", buffer) if p.strip()]
    return parts or ([buffer.strip()] if buffer.strip() else [])


@dataclass
class Arm:
    name: str
    global_file: str = ""
    workspace_file: str = ""
    hand_edit_survived: bool = False
    notes: int = 0
    top1: int = 0
    top1_no_rerank: int = 0
    pollution: int = 0
    tokens: int = 0
    latency_ms: float = 0.0
    detail: list[dict[str, Any]] = field(default_factory=list)


def seed_files() -> tuple[str, str]:
    """Start from the real profile content, so the shape is not invented."""
    global_file = Path("~/.grok/memory/MEMORY.md").expanduser().read_text(encoding="utf-8")
    workspace_file = Path("~/.grok/memory/grok-build-ce8c9272/MEMORY.md").expanduser().read_text(
        encoding="utf-8"
    )
    return global_file, workspace_file


def run_today(global_file: str, workspace_file: str) -> Arm:
    """No gate, no scope: every event appended to global, exactly as the effect does."""
    arm = Arm("today (MEMORY.md only)")
    for index, event in enumerate(EVENTS):
        if index == 20:
            global_file = append(global_file, HAND_EDIT)
        global_file = append(global_file, event.text)
        arm.notes += 1
    arm.global_file, arm.workspace_file = global_file, workspace_file
    arm.hand_edit_survived = "Hand-written note" in global_file
    return arm


def run_with_jev(client: transports.Client, global_file: str, workspace_file: str) -> Arm:
    """Gate, dedup and scope routing, writing through the same append path."""
    arm = Arm("today + jev (MEMORY.md + milvus untouched)")
    store: list[str] = []

    for index, event in enumerate(EVENTS):
        if index == 20:
            global_file = append(global_file, HAND_EDIT)
            store.append(HAND_EDIT)

        response = client.ask(
            {
                "candidate_note": event.text,
                "workspace_path": WORKSPACE_PATH,
                "already_in_memory": "\n".join(f"- {t}" for t in store) or "(empty)",
            },
            {
                "worth": primitives.noul(GATE_QUESTION, true=GATE_TRUE, false=GATE_FALSE),
                "covered": primitives.noul(COVERED_QUESTION),
            },
        )
        arm.tokens += response.input_tokens
        arm.latency_ms += response.latency_ms
        if response.noul("covered") >= COVERED_THRESHOLD:
            continue
        if response.noul("worth") < ADMIT_THRESHOLD:
            continue

        bucket = "global"
        if event.keep:
            scope = client.ask(
                {"workspace_path": WORKSPACE_PATH, "candidate_note": event.text},
                {"bucket": primitives.choice(SCOPE_QUESTION, SCOPE_CRITERIA)},
            )
            arm.tokens += scope.input_tokens
            arm.latency_ms += scope.latency_ms
            bucket = scope.choice("bucket").choice
            if bucket == "discard":
                continue
            bucket = "workspace" if bucket == "this_folder" else "global"

        if bucket == "workspace":
            workspace_file = append(workspace_file, event.text)
        else:
            global_file = append(global_file, event.text)
        store.append(event.text)
        arm.notes += 1

    arm.global_file, arm.workspace_file = global_file, workspace_file
    arm.hand_edit_survived = "Hand-written note" in global_file
    return arm


def score(arm: Arm, key: str, client: transports.Client | None) -> None:
    """Index what the files hold, then query them with the unchanged retriever."""
    corpus = chunks(arm.global_file) + chunks(arm.workspace_file)
    scope_of = ["global"] * len(chunks(arm.global_file)) + ["workspace"] * len(chunks(arm.workspace_file))
    vectors = cached(corpus, key, f"coexist:{arm.name}")
    qvecs = cached([q["q"] for q in QUERIES], key, "query")

    for case, qvec in zip(QUERIES, qvecs):
        hits = retrieve(corpus, vectors, case["q"], qvec, SHORTLIST)
        if hits and case["gold"] in corpus[hits[0]]:
            arm.top1_no_rerank += 1
        top, top_scope = (corpus[hits[0]], scope_of[hits[0]]) if hits else (None, None)
        if client is not None and hits:
            shortlist = [corpus[i] for i in hits]
            response = client.ask(
                {"workspace_path": WORKSPACE_PATH, "query": case["q"]},
                {
                    f"cand::{i}": primitives.noul(
                        f"{RERANK_QUESTION}\n\nChunk: {text}", true=RERANK_TRUE, false=RERANK_FALSE
                    )
                    for i, text in enumerate(shortlist)
                },
            )
            arm.tokens += response.input_tokens
            arm.latency_ms += response.latency_ms
            ranked = sorted(
                ((response.noul(f"cand::{i}"), i) for i in range(len(shortlist))), key=lambda kv: -kv[0]
            )
            best = ranked[0][1]
            top, top_scope = corpus[hits[best]], scope_of[hits[best]]
        if top and case["gold"] in top:
            arm.top1 += 1
        if case["kind"] == "local" and top_scope == "global":
            arm.pollution += 1
        arm.detail.append({"q": case["q"], "top_scope": top_scope, "hit": bool(top and case["gold"] in top)})


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)
    key = embed_key()
    seed_global, seed_workspace = seed_files()

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        today = run_today(seed_global, seed_workspace)
        jev = run_with_jev(client, seed_global, seed_workspace)
        score(today, key, client)
        score(jev, key, client)

    table = Table(title="coexistence — hand edit interleaved at event 20")
    for column in ("check", today.name, jev.name):
        table.add_column(column, justify="right" if column != "check" else "left")
    rows = [
        ("hand-written note survived", lambda a: "[green]yes[/green]" if a.hand_edit_survived else "[red]LOST[/red]"),
        ("notes written", lambda a: str(a.notes)),
        ("global MEMORY.md, chars", lambda a: f"{len(a.global_file):,}"),
        ("workspace MEMORY.md, chars", lambda a: f"{len(a.workspace_file):,}"),
        ("sections indexed", lambda a: str(len(chunks(a.global_file)) + len(chunks(a.workspace_file)))),
        ("headings well-formed", lambda a: f"{sum(1 for c in chunks(a.global_file) + chunks(a.workspace_file) if c.startswith('#'))}/{len(chunks(a.global_file)) + len(chunks(a.workspace_file))}"),
        ("query top-1, no rerank", lambda a: f"{a.top1_no_rerank}/12"),
        ("query top-1, reranked", lambda a: f"{a.top1}/12"),
        ("local query served by global", lambda a: f"{a.pollution}/7"),
    ]
    for label, getter in rows:
        table.add_row(label, getter(today), getter(jev))
    console.print(table)

    console.print("\n  format sample of one appended entry (normalize + append):")
    console.print("    " + append("", EVENTS[0].text).replace("\n", "\n    "))

    raw = OUT / "results-coexistence.jsonl"
    with raw.open("w", encoding="utf-8") as handle:
        for arm in (today, jev):
            handle.write(json.dumps(asdict(arm), ensure_ascii=False) + "\n")
    console.print(f"\n  tokens {jev.tokens:,}, ${jev.tokens / 1e6 * 0.042:.4f} -> {raw.name}")


if __name__ == "__main__":
    main()