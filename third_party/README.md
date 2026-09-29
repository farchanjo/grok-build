# Third-party vendored crates

This directory holds **upstream source** vendored into the repository. It is
**not** first-party application code.

## Why vendor

These crates sit on the path that renders **untrusted model output** (diagram
source → SVG) or on the **wire contract** with hosts (ACP). Vendoring gives a
full audit surface, pins exact source, and avoids crates.io yanks. Local
patches and upgrade checklists live in each crate’s `Cargo.toml` header
comments — treat those as the source of truth when re-vendoring.

## ACP protocol stack

| Crate | Version | License | Upstream | Full license text |
|-------|---------|---------|----------|-------------------|
| [`agent-client-protocol`](./agent-client-protocol/) | 0.10.4 | Apache-2.0 | [agentclientprotocol/agent-client-protocol](https://github.com/agentclientprotocol/agent-client-protocol) | [`LICENSE`](./agent-client-protocol/LICENSE) |
| [`agent-client-protocol-schema`](./agent-client-protocol-schema/) | 0.11.4 | Apache-2.0 | [agentclientprotocol/agent-client-protocol](https://github.com/agentclientprotocol/agent-client-protocol) | [`LICENSE`](./agent-client-protocol-schema/LICENSE) |

Patched, not pristine. The schema's response structs gain a
`#[serde(flatten)] extra` catch-all so an agent can emit top-level keys the
spec has no field for — hosts read `openCodeSessionId` (resume handle) and a
load-time `sessionId` — and its widest enum variants are boxed; the SDK crate
follows that one boxed variant. Both are pinned by `[patch.crates-io]` in the
workspace root; the SDK pins the schema with `=`, so the two versions move
together.

## Mermaid layout stack

| Crate | Version | License | Upstream | Full license text |
|-------|---------|---------|----------|-------------------|
| [`mermaid-to-svg`](./mermaid-to-svg/) | (path) | MIT | [warpdotdev/mermaid-to-svg](https://github.com/warpdotdev/mermaid-to-svg) | [`LICENSE`](./mermaid-to-svg/LICENSE) |
| [`dagre_rust`](./dagre_rust/) | 0.0.5 | Apache-2.0 | [r3alst/dagre-rust](https://github.com/r3alst/dagre-rust) / Warp re-vendor | [`LICENCE`](./dagre_rust/LICENCE) |
| [`graphlib_rust`](./graphlib_rust/) | 0.0.2 | Apache-2.0 | [r3alst/graphlib-rust](https://github.com/r3alst/graphlib-rust) | [`LICENCE`](./graphlib_rust/LICENCE) |
| [`ordered_hashmap`](./ordered_hashmap/) | 0.0.3 | Apache-2.0 | [r3alst/ordered-hashmap](https://github.com/r3alst/ordered-hashmap) | [`LICENCE`](./ordered_hashmap/LICENCE) |

Dependency shape:

```text
xai-grok-mermaid
  └── mermaid-to-svg          (MIT)
        ├── dagre_rust        (Apache-2.0)
        │     ├── graphlib_rust
        │     └── ordered_hashmap
        └── graphlib_rust     (Apache-2.0)
              └── ordered_hashmap
```

## Notices and ancestry

- **[`NOTICE`](./NOTICE)** — short index of the crates above (names, licenses,
  upstream links, paths to full text). Prefer that file for a one-page overview.
- **[`mermaid-to-svg/THIRD_PARTY_NOTICES`](./mermaid-to-svg/THIRD_PARTY_NOTICES)** —
  additional ancestry for the SVG engine (e.g. mermaid.js, dagre.js MIT notices).

British spelling **`LICENCE`** is intentional on the Apache crates (as upstream
vendored); grepping only for `LICENSE` will miss them.

## crates.io dependencies

Normal Cargo dependencies (tokio, serde, …) are **not** under `third_party/`.
They resolve via `Cargo.lock` / crates.io. Full attribution and license texts
for the Grok CLI dependency closure are maintained in
[`THIRD-PARTY-NOTICES`](../THIRD-PARTY-NOTICES).

This directory is only for **in-tree vendored** sources.

## Upgrading

1. Read the `VENDORING NOTES` block at the top of the crate’s `Cargo.toml`.
2. Re-apply listed local patches (fmt, hermetic env, unsafe fixes, dropped bins/tests).
3. Confirm the license file still matches the declared `license =` field.
4. Refresh [`NOTICE`](./NOTICE) if versions or upstream URLs change.
