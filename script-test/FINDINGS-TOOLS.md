# Tool search with Jev — complete findings and design

Consolidated 2026-09-19. Companion to the other `FINDINGS-*.md`; `FINDINGS.md` is
the umbrella.

Measured against `jev-1.13`, native transport, live dev profile read-only. No
Rust was changed.

## 1. Goal

The tool path is the one retrieval surface in the harness that never got the
treatment the others have. Find out whether that costs anything.

## 2. What exists today (verified)

`session/tool_index.rs` (2502 lines) is a **BM25 index over registered MCP tools**:

```rust
use bm25::{Language, SearchEngineBuilder};
// "The index is rebuilt on each search call (sub-millisecond for tens
//  to low hundreds of tools)."
```

- Ranking is BM25 over a normalised token stream, plus a short-circuit when the
  query exactly equals a qualified tool name (`score: 1.0`).
- `split_identifier` handles `__`, snake_case, kebab-case and camelCase — the
  tokeniser is careful, the ranking is lexical.
- The trait lives in `xai-grok-tools` (`types/tool_index.rs`) and the
  implementation in the shell, the same shape as `MemoryBackend` — so swapping the
  backend is an existing seam, not a new one.
- Scale: the dev profile declares five enabled MCP servers — `arithma` (174
  tools), `ssh` (39), `obscura` (37), `admaxium` (33), `detent` (10) — around
  **293 tools**, plus the built-ins.

No embedding, no reranker, no Jev — in a codebase where memory has a vector store
and a reranker slot, and compaction already calls Jev.

## 3. Measured: the lexical path collapses cross-lingually

35 requests in Portuguese against a 100-tool catalog modelled from `arithma`'s own
surface, including the lookalikes it deliberately contains (`add` vs `sumArray` vs
`subtract`, `convert` vs `convertAutoDetect`, `subnetCalculator` vs `ipInSubnet`
vs `vlsmSubnets`).

| arm | top-1 | gold in the pool | p50 | tokens |
| --- | --- | --- | --- | --- |
| **bm25 (today)** | **5/35** | **7/35** | 0 ms | 0 |
| bm25 + reranker | 5/35 | 7/35 | 6 ms | 0 |
| **milvus + jev** | **31/35** | **33/35** | 306 ms | 18,973 |
| milvus + reranker + jev | 31/35 | 33/35 | 310 ms | 18,973 |

**The embedding finds the gold 4.7x more often, and picks it 6x more often.**

The cause is not subtle: **the requests are in Portuguese and the tool
descriptions are in English.** BM25 has no cross-lingual ability, so 28 of the 35
requests return an empty pool — `bm25_1o` is `—` for almost every line. The same
failure shape as the skill eval matcher (`FINDINGS-SKILLS.md` §3), which also
scored 0 on natural language.

**The reranker is neutral on both paths** — 5/35 → 5/35 and 31/35 → 31/35. Sixth
measurement of the axis, sixth neutral-or-negative result.

## 4. Design

The seam already exists: `ToolSearchIndex` is a trait, `MemoryBackend` set the
precedent, and the concrete implementation is one file.

| stage | attachment | fail-open |
| --- | --- | --- |
| index | a dense index beside the BM25 one, keyed on name + description | BM25, which is what runs today |
| search | fuse BM25 and dense with RRF, as `prime/skills.rs` already does | BM25 only |
| selection | one `choice` over the fused shortlist | fused top-1 |
| reranker | leave the slot empty (§3) | — |

```
search: fuse(bm25, dense) -> shortlist of 10
select: one choice, "Which tool should be called for this request? Prefer the one
        that does exactly this job."
```

Two notes for whoever wires it:

- **Keep the exact-name short-circuit.** It is the one thing BM25 does better than
  an embedding, and `use_tool` calls often name the tool verbatim.
- **The index must rebuild from the live MCP toolset**, since servers connect and
  disconnect; the current implementation already rebuilds per call, which is
  cheap at this scale and removes a whole class of staleness bugs.

## 5. TUI configuration

**What exists.** Nothing specific to tool search. MCP servers have their own
enable/disable rows, and `/mcp` reports connection state.

**What is missing.** A search-backend choice, and any visibility of what a search
returned.

**Shape, following `/jev`.** `/jev on|off|status` dispatches a typed action, the
same one the settings row dispatches; the value persists and the live session
picks it up through the config reload fan-out.

```
/tool-search on|off|status
  status  -> backend, index size, last query and its top-3, whether it fell back

settings row: "Tool search backend" enum {bm25, dense, fused}, default fused
              once it ships, bm25 until then
```

**Any tool-search list must scroll and filter too** — a shortlist of ten is fine
flat, but a browsable index is not. Use the existing `list_pane` (virtualised) and
the `/` filter the six list modals already ship, per `FINDINGS.md` cross-cutting.

The `status` line matters more here than elsewhere: tool search is invisible
today, and a `use_tool` that returns nothing looks like a model mistake rather
than a retrieval miss.

## 6. Implementation order

1. **Add the dense index and fuse it.** 7/35 → 33/35 on the pool, which is the
   whole finding. Reuse the `prime` fusion rather than writing a second one.
2. **Keep BM25 in the fusion and the exact-name short-circuit.** Do not remove
   the lexical path — the measurement says it is bad cross-lingually, not that it
   is bad.
3. **Add the selection call only if the fused top-1 disappoints.** It is +0 in the
   fixtures here (33/35 pool → 31/35 pick is the ceiling, and the fused top-1 was
   not measured separately).
4. **Leave the reranker off.** Sixth neutral result.
5. **`/tool-search status`** so a miss is visible.

## 7. Hazards

- **Rebuilding a dense index per call is not free.** BM25 rebuilds in
  sub-milliseconds; an embedding call per search is ~300 ms. Cache the vectors
  keyed on the toolset, and rebuild only when the MCP toolset changes.
- **A stale tool index invents tools.** Servers come and go; the exact-name
  short-circuit will happily match a tool that no longer exists.
- **293 tools is the current ceiling and it is small.** The argument for dense
  search here is cross-lingual recall, not scale — say so, or the next reader will
  assume a scale problem.
- **`use_tool` needs the full input schema**, and `ToolSearchResult` already
  carries it — a dense backend must return the same fields or the call sites
  break.

## 8. Not verified

- **The catalog is modelled, not captured.** 100 of `arithma`'s 174 tools, written
  from its own grouping; descriptions are mine. The `ssh`, `obscura`, `admaxium`
  and `detent` tools are not in it.
- The 35 requests are mine, in Portuguese against English descriptions — a
  deliberate cross-lingual test, and the reason the BM25 number is so low. A
  monolingual user would see a smaller gap.
- The `ToolSearchIndex` trait and the BM25 implementation were read, never run;
  the BM25 arm is a Python FTS5 stand-in, not the `bm25` crate.
- Whether the *agent's* tool choice improves with a better pool was not measured —
  only the retrieval.
- Nothing was measured inside the TUI.