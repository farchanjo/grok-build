# archanjo

Out-of-tree **custom tool pack** for Grok Build, following the modular
composition pattern:

| Layer | Responsibility |
| --- | --- |
| **Tools (this crate)** | Typed tools + ports (injected resources) |
| **Shell adapter** | BM25 catalog backend, injects `ModelCatalogSearch` |
| **Agent toolsets** | Reference `archanjo::SearchModelsTool` in product presets |
| **Composition root** | `archanjo::register()` before any `ToolRegistryBuilder::new()` |

## Tools

| Tool id | Kind | Description |
| --- | --- | --- |
| `Archanjo:search_models` | `SearchModels` | Product name → catalog slug for `spawn_subagent` / host `task` |

## Registration (default on)

`search_models` is **enabled by default** for every agent:

1. Pack registration: `archanjo::register()` (pager-bin, shell rebuild, agent build).
2. Toolset lists already include `Archanjo:search_models`.
3. `AgentBuilder` always runs `ensure_search_models_tool` so custom/minimal
   agents still get it unless an explicit tools denylist removes it.

```rust
// composition root (xai-grok-pager-bin) and shell agent rebuild
archanjo::register();
// or
xai_grok_shell::register_extension_tool_packs();
```

Safe to call repeatedly; only the first registration applies.

## Adding another Archanjo tool

1. Add a module under `src/` with `Tool` + `ToolMetadata` (`ToolNamespace::Archanjo`).
2. Register it in `ARCHANJO_TOOL_PACK` inside `lib.rs`.
3. Reference it from agent toolsets if it should be product-default.
4. Inject any ports from the shell at session rebuild time.
