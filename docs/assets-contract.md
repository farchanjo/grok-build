# AssetStore — Phase 1 contract (frozen)

Status: **frozen after independent design review** (arch-advisor audit + api-designer contract).
Scope: multi-backend object storage behind one async trait, consumed by five new `asset_*` tools.
Language: en-US for every artifact (code, comments, tests, docs, diagnostics).

Source plan: `plan.md` §2 (Phase 1). This file supersedes the plan where they disagree — the
review produced 27 findings, and the reconciliation is recorded in §1 below.

---

## 0. Non-negotiable invariants

| # | Invariant |
|---|---|
| I1 | Purely additive: `upload_config.rs` (`UploadMethod`, `TraceExportConfig`) keeps its current shape byte-identical. |
| I2 | No secret in `config.toml`. Credentials resolve only from `env_key` (env var *name*) → vault → `credentials_file` → backend ambient chain. |
| I3 | Private by default. `Visibility::Private` is `#[default]`. Public is always explicit. |
| I4 | Bounded presign TTL: `MIN = 1s`, `MAX = 604_800s` (SigV4 ceiling), `DEFAULT = 3_600s`. Out of range is an error, never a silent clamp. |
| I5 | `Unsupported` is explicit. A backend that cannot do an operation returns `AssetError::Unsupported { .. }` — never a fake success. |
| I6 | Keys are path-safe by construction: no `..`, no empty segments, ASCII only, so the local backend needs no canonicalization for containment. |
| I7 | `AssetError` is `Debug + Clone + Send + Sync + 'static` (string-owned payloads; no `io::Error` in the enum) so it crosses the tool boundary and converts into `anyhow::Error`. |
| I8 | **Everything async on tokio.** No blocking I/O on an async task; where the AWS/GCS SDK is blocking-only, wrap in `tokio::task::spawn_blocking`. Trait futures must be `Send` so `Arc<dyn AssetStore>` can be moved into `tokio::spawn`. |

---

## 1. Reconciliation of the design review (binding decisions)

| Finding | Decision |
|---|---|
| F1 — config schema belongs in `xai-grok-config-types` | **Accepted.** `AssetsConfig`/`AssetProviderConfig` live in `xai-grok-config-types/src/assets.rs`; `xai-file-utils` gains a dependency on that crate (verified: no cycle today). Runtime trait/adapters stay in `xai-file-utils`. |
| F2 — `assets/s3.rs` shadows `src/s3.rs` | **Accepted.** Adapter files are `s3_store.rs`, `gcs_store.rs`, `local_store.rs`, `proxy_store.rs`. |
| F4 — factory must be `async` | **Accepted.** S3 client construction awaits. |
| F5 — factory needs more than config | **Accepted.** `resolve_asset_store(&AssetsSettings, &AssetRuntimeContext)`. |
| F6 — `AssetKey` declared but unused | **Accepted.** The trait takes `&AssetKey` everywhere; no bare `&str` in the trait surface. |
| F7/F8 — `provider` vs `kind`, missing `active_profile` | **Accepted.** `kind` inside profiles (mirrors `retrieval.rs`); `[assets].active_profile` is explicit in the schema. |
| F9 — wrong struct cited | **Accepted.** The store lands on `SessionContext` (`registry/types.rs:230`), not `ToolServerConfig` (which only holds the tool list). |
| F10 — error type underspecified | **Accepted.** Typed `AssetError` for all runtime ops; `anyhow` only for construction/config plumbing. |
| F11 — factory `Result` vs `Option` | **Accepted with note.** `Result` covers *construction* failure only; the terminal `local` fallback must be total. |
| F13 — `DynamicEnum` premature | **Split.** `assets.provider` = static `Enum` (4 compile-time kinds). `assets.active_profile` = `DynamicEnum` over `[assets_providers.*]` (new `DynamicEnumSource::AssetProfileCatalog`). Satisfies option (b) without a runtime catalog where none is needed. |
| F14 — no `presign_put` | **Accepted.** Added to the trait. |
| F15 — `Bytes`-only put/get regresses 1-2 GB media | **Accepted.** `put_file` + `download_to` streaming variants in v1. |
| F16 — no `exists`/`head` | **Accepted.** `exists` added. |
| F18 — `x-amz-acl` rejected on bucket-owner-enforced buckets | **Accepted.** On `AccessControlListNotSupported`, retry without ACL and degrade to recorded visibility. |
| F19 — proxy has no delete/visibility | **Accepted.** Capability matrix + `Unsupported`, no pretending. |
| F20 — no key namespacing | **Accepted.** Default `key_prefix = "uploads/"`; deterministic derivation. |
| F26 — no test seam | **Accepted.** `MockAssetStore` + `LocalAssetStore` over a tempdir. |
| F27 — `asset_server_url` is a different host | **Accepted.** Do **not** reuse the dead `asset_server_url` field; `[assets].public_base_url` is its own key. Leave `asset_server_url` untouched in Phase 1. |

---

## 2. Module layout

```
crates/codegen/xai-grok-config-types/src/assets.rs   — AssetsConfig, AssetProviderConfig, AssetVisibility (serde)
crates/codegen/xai-file-utils/src/assets/
  mod.rs        — AssetStore trait, BackendKind, BackendCapabilities, StoreStatus, SharedAssetStore
  key.rs        — AssetKey, AssetPrefix, ContentType, KeyError
  value.rs      — Visibility, AssetMeta, PresignedUrl, PutRequest, PutSource, ListQuery, ListPage, DeleteOutcome
  error.rs      — AssetError, AssetOperation
  factory.rs    — resolve_asset_store, resolve_asset_store_source, AssetStoreSource, AssetRuntimeContext
  s3_store.rs   — S3AssetStore
  gcs_store.rs  — GcsAssetStore
  local_store.rs— LocalAssetStore
  proxy_store.rs— ProxyAssetStore
  mock.rs       — MockAssetStore (cfg(test) or feature test-support)
```

Manifest delta on `xai-file-utils`: add `async-trait`, `bytes`, `xai-grok-config-types` (all already in the workspace graph).

---

## 3. Trait

`#[async_trait::async_trait]` — the store is always behind `dyn` (stored in `Resources`, `TypeId`-keyed), matching every other `Arc<dyn _>` trait in the repo (`EmbeddingProvider`, `AsyncFileSystem`, `MemoryBackend`).

```rust
pub type SharedAssetStore = Arc<dyn AssetStore>;

#[async_trait::async_trait]
pub trait AssetStore: Send + Sync {
    fn backend(&self) -> BackendKind;
    fn capabilities(&self) -> BackendCapabilities;

    async fn put(&self, request: PutRequest) -> Result<AssetMeta, AssetError>;
    async fn put_file(&self, request: PutRequest) -> Result<AssetMeta, AssetError>;   // streaming source
    async fn get(&self, key: &AssetKey) -> Result<Bytes, AssetError>;
    async fn download_to(&self, key: &AssetKey, dest: &Path) -> Result<AssetMeta, AssetError>;
    async fn exists(&self, key: &AssetKey) -> Result<bool, AssetError>;
    async fn delete(&self, key: &AssetKey) -> Result<DeleteOutcome, AssetError>;      // idempotent
    async fn list(&self, query: ListQuery) -> Result<ListPage, AssetError>;
    async fn presign_get(&self, key: &AssetKey, ttl: Duration) -> Result<PresignedUrl, AssetError>;
    async fn presign_put(&self, key: &AssetKey, ct: &ContentType, ttl: Duration) -> Result<PresignedUrl, AssetError>;
    async fn set_visibility(&self, key: &AssetKey, v: Visibility) -> Result<AssetMeta, AssetError>;
    fn public_url(&self, key: &AssetKey) -> Option<String>;
    async fn health(&self) -> Result<StoreStatus, AssetError>;
}
```

Every method that touches the network must be cancellation-safe (dropping the future must not leak a multipart upload — abort on drop where the SDK supports it).

---

## 4. Value types and validation

- `AssetKey`: newtype over `String`; ASCII `[A-Za-z0-9._~/-]`; `/` is separator only; no empty/`.`/`..` segment; first segment ≠ `_meta`; ≤ 1024 bytes total, ≤ 255 per segment. Accepts `s3://bucket/...` and `gs://bucket/...` and strips the bucket prefix.
- `ContentType`: `type/subtype` RFC 7231 tokens, ≤ 255 bytes, optional params. User-supplied is validated (error on garbage); inferred from extension degrades to `application/octet-stream`.
- `Visibility { Private, Public }`, `Default = Private`.
- `AssetMeta { key, size_bytes, content_type, visibility, visibility_enforced, backend, modified_at: Option<String>, etag: Option<String> }`.
- `PutSource { Bytes(Bytes), File(PathBuf) }` — `put_file` must stream; adapters must not buffer whole files.
- `ListQuery { prefix, limit (1..=1000, default 100), cursor: Option<ListCursor>, include_meta: bool }` → `ListPage { items, next_cursor, truncated }`. `_meta/` hidden by default.
- `PresignedUrl { key, url, method, expires_in, expires_at, content_type: Option<String>, emulated: bool }`.

Reserved `_meta/<key>.visibility.json` sidecars back recorded visibility on backends with no enforcement.

---

## 5. Errors

Typed `AssetError` (`#[non_exhaustive]`, `Debug + Clone + Send + Sync`): `NotFound`, `Unauthorized` (401), `AccessDenied` (403), `Unsupported { backend, operation, reason }`, `InvalidKey`, `InvalidContentType`, `TtlOutOfRange`, `ObjectTooLarge`, `PreconditionFailed`, `Transient`, `Io`.

401 and 403 stay distinct: 401 feeds the repo's 401-attribution path, 403 maps to `PermissionDenied`.

Projection to `xai_tool_runtime::ToolError` is one shared function; `code()` returns a stable snake_case string for `details.code`.

---

## 6. Config

`[assets]`: `provider` (Option, Enum), `active_profile` (Option), `bucket`, `region`, `endpoint_url`, `credentials_file`, `env_key`, `public_base_url`, `default_visibility`, `default_ttl_secs`, `local_root`, `proxy_base_url`, `max_object_bytes`, `request_timeout_secs`, `key_prefix`.

`[assets_providers.<id>]`: `kind` (required) + the same fields, inheriting from `[assets]`; `deny_unknown_fields`; id matches `[A-Za-z0-9_-]{1,64}`.

Selection precedence: explicit `provider` → named `active_profile` → bucket scheme (`s3://`/`gs://`) → `local` (total). Named mistakes fail closed; only an unrecognized scheme warns and falls through.

Env overrides mirror the existing `GROK_*` convention: `GROK_ASSETS_PROVIDER`, `GROK_ASSETS_PROFILE`, `GROK_ASSETS_BUCKET`, `GROK_ASSETS_REGION`, `GROK_ASSETS_ENDPOINT_URL`, `GROK_ASSETS_CREDENTIALS_FILE`, `GROK_ASSETS_PUBLIC_BASE_URL`, `GROK_ASSETS_DEFAULT_VISIBILITY`, `GROK_ASSETS_DEFAULT_TTL_SECS`, `GROK_ASSETS_LOCAL_ROOT`, `GROK_ASSETS_PROXY_BASE_URL`, `GROK_ASSETS_MAX_OBJECT_BYTES`, `GROK_ASSETS_REQUEST_TIMEOUT_SECS`, `GROK_ASSETS_KEY_PREFIX`.

---

## 7. Capability matrix (must be encoded, not documented-only)

| Op | s3 | gcs | local | proxy |
|---|---|---|---|---|
| put / put_file / get / download_to / exists | native | native | native | native |
| delete | native (new) | native | native | unsupported |
| list | native (new, cursor) | native | native | unsupported |
| presign_get | native | emulated (needs SA key) | emulated (`file://`) | native |
| presign_put | native | emulated | unsupported | native |
| set_visibility | emulated (ACL; degrade on rejection) | unsupported | recorded if `public_base_url` | recorded if `public_base_url` |
| visibility_enforced | true, false after ACL rejection | false | false | false |

`S3AssetStore` must close these gaps in `s3.rs` (additive): `put_object` with ACL, `put_object_acl`, `delete_object`, `list_objects_v2` with continuation token.

---

## 8. Tools

`asset_upload`, `asset_share`, `asset_list`, `asset_delete`, `asset_set_visibility` — namespace `GrokBuild`, registered in `registry/types.rs`, store injected via `Resources` (`resources.insert(Arc<dyn AssetStore>)`, `TypeId`-keyed, no macro needed).

`ToolConsumer::AssetStore` added to `attribution.rs`. `ToolKind` gains five variants; `tool_taxonomy.rs` exhaustive matches gain arms; `asset_share`/`asset_list` are read-only.

Tool outputs surface `visibility_enforced` / `enforced: false` honestly, plus a secret-free `hint` when a backend records without enforcing.

---

## 9. Async / tokio requirements (explicit)

- Trait methods are `async` with `Send` futures; no `block_on` inside a tool.
- Any blocking SDK call runs under `tokio::task::spawn_blocking`.
- The store is built once per session in the registry builder and shared via `Resources` (never rebuilt per tool call, never per subagent).
- Timeouts come from `[assets].request_timeout_secs`; retries are explicit (`Transient` is retryable, everything else is not) — no blanket retry wrapper.
- Streaming paths (`put_file`, `download_to`) must not materialize the whole object in memory.

---

## 10. Test obligations

Unit: key/content-type accept+reject tables; TTL bounds; error `code()` stability; `Display` is secret-free (no `sk-`, no `X-Amz-Signature`); factory precedence incl. fail-closed named cases; per-adapter request shaping against the existing axum mock-S3 router in `s3.rs`; ACL rejection → degrade path; capability fast-fail.

Integration: `LocalAssetStore` over a tempdir as the tool-test seam; `MockAssetStore` for tool unit tests.

Contract: `settings_e2e.rs` — every new key in `ALL_SETTINGS_EXERCISED` + keyboard + mouse. PTY: `tests/pty_e2e/asset_settings_pty.rs`. tmux: upload → share → open → set_visibility → list → delete.

---

## 11. Open items (decidable during implementation, no contract change)

1. S3 public path: per-object ACL vs bucket policy + CDN. Default: ACL with degrade-on-rejection, `public_base_url` as the public contract.
2. Whether `image_gen` auto-uploads after saving locally (Phase 3 coupling).
3. `local_root` default (`$GROK_HOME/assets`) and its budget.