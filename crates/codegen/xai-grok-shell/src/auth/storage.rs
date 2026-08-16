use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::provider_registry::id::ProviderId;
use crate::provider_registry::instance::ProviderIncarnation;
use crate::provider_registry::secrets::ProviderOAuthBinding;

use super::model::{API_KEY_SCOPE, AuthMode, AuthStore, GrokAuth, lookup_auth};

/// RAII guard for an exclusive advisory lock on `auth.json.lock`.
/// The lock is released when the inner `File` is dropped (closing the FD).
pub(crate) struct AuthFileLock {
    pub(super) _file: File,
}

impl AuthFileLock {
    /// Returns `true` while this guard still refers to the **live**
    /// `auth.json.lock` inode.
    ///
    /// A waiter that finds a holder stuck past the stale-lock timeout breaks
    /// the lock by `unlink`ing the file and recreating it on a fresh inode
    /// (see [`crate::auth::manager::lock`]). The usual cause of a "stuck"
    /// holder is a process **suspended across system sleep** while holding the
    /// lock: it stays alive (so the kernel never releases its flock) yet makes
    /// no progress, so siblings break it. When such a holder resumes, its
    /// flock lives on the now-deleted inode — it no longer holds the live lock
    /// even though this `AuthFileLock` still exists.
    ///
    /// Callers about to perform an irreversible, lock-protected action
    /// (sending a refresh token to the IdP, writing `auth.json`) MUST
    /// re-validate first; otherwise two processes can spend the same refresh
    /// token and trip token-family revocation.
    ///
    /// Non-Unix has no inode concept, so this conservatively returns `true`.
    #[cfg(unix)]
    pub(crate) fn still_live(&self, auth_json_path: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        let lock_path = auth_json_path.with_file_name("auth.json.lock");
        let (Ok(fd_meta), Ok(path_meta)) = (self._file.metadata(), std::fs::metadata(&lock_path))
        else {
            // Lock file gone or unreadable → we no longer hold the live lock.
            return false;
        };
        fd_meta.ino() == path_meta.ino() && fd_meta.dev() == path_meta.dev()
    }

    #[cfg(not(unix))]
    pub(crate) fn still_live(&self, _auth_json_path: &Path) -> bool {
        true
    }
}

pub fn read_auth_json(auth_file: &Path) -> std::io::Result<AuthStore> {
    let mut file = File::open(auth_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // Tighten world-readable copies (hand-restored, umask edge cases, etc.).
    // Best-effort: a chmod failure must not block login/read paths.
    if let Err(e) = crate::util::secure_file::ensure_owner_only_permissions(auth_file) {
        tracing::warn!(
            path = %auth_file.display(),
            error = %e,
            "auth: failed to enforce owner-only permissions on auth.json"
        );
    }

    // Empty files are valid (recover from prior crash/partial write).
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return Ok(AuthStore::new());
    }

    let map = serde_json::from_str(trimmed)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(map)
}

/// Read auth.json, returning an empty map if the file does not exist.
///
/// Non-empty corrupt JSON, permission errors, etc. are returned as errors
/// so the caller can decide whether to skip the write (to avoid clobbering
/// sibling scopes).
///
/// Used by provider-scoped writes and the test-only `persist_and_swap`.
pub(crate) fn read_auth_json_or_empty(auth_file: &Path) -> std::io::Result<AuthStore> {
    match read_auth_json(auth_file) {
        Ok(map) => Ok(map),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AuthStore::new()),
        Err(e) => Err(e),
    }
}

/// Best-effort backup of a corrupt (unparseable) auth.json.
///
/// If the file exists and `read_auth_json` fails with `InvalidData`,
/// it is renamed to `auth.json.corrupt.<millis>` (sibling in the same
/// directory) and the backup path is returned. Used before recovery
/// writes so the original bytes are never silently lost.
pub(crate) fn backup_corrupt_auth_file(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    if read_auth_json(path).is_ok() {
        return None;
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "auth.json".to_string());

    let backup_name = format!("{}.corrupt.{}", file_name, ts);
    let backup = path.with_file_name(backup_name);

    match std::fs::rename(path, &backup) {
        Ok(()) => {
            // Corrupt backups still hold token material — keep them owner-only.
            let _ = crate::util::secure_file::ensure_owner_only_permissions(&backup);
            tracing::warn!(
                original = %path.display(),
                backup = %backup.display(),
                "auth: backed up corrupt auth.json before recovery write"
            );
            // Must reach unified.jsonl: the tracing line above is invisible
            // in production captures, and this is the only record of both
            // the corruption and where the original bytes went.
            xai_grok_telemetry::unified_log::error(
                "auth: corrupt auth.json backed up",
                None,
                Some(serde_json::json!({
                    "original": path.display().to_string(),
                    "backup": backup.display().to_string(),
                })),
            );
            Some(backup)
        }
        Err(e) => {
            tracing::warn!(error = %e, "auth: failed to rename corrupt auth.json for backup");
            xai_grok_telemetry::unified_log::error(
                "auth: corrupt auth.json backup failed",
                None,
                Some(serde_json::json!({
                    "original": path.display().to_string(),
                    "error": e.to_string(),
                })),
            );
            None
        }
    }
}

/// Read auth.json for an upcoming write, with recovery for corrupt files.
///
/// - Missing/empty → empty map (safe to write fresh)
/// - Valid JSON → parsed map
/// - Non-empty corrupt JSON → backs up to `auth.json.corrupt.<millis>`,
///   then returns empty map so the caller can write the new credential.
///
/// Other I/O errors (PermissionDenied, etc.) are still returned as errors.
pub(crate) fn read_auth_json_or_empty_recovering_corrupt(
    auth_file: &Path,
) -> std::io::Result<AuthStore> {
    match read_auth_json(auth_file) {
        Ok(map) => Ok(map),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AuthStore::new()),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            let _ = backup_corrupt_auth_file(auth_file);
            Ok(AuthStore::new())
        }
        Err(e) => Err(e),
    }
}

/// Persist `auth.json`, preferring a crash-safe atomic write but falling
/// back to a non-atomic in-place write when the disk is full.
///
/// The atomic path (temp + rename) needs free space >= the file size,
/// because the old file and a full temp copy coexist until the rename. On a
/// nearly-full disk that temp copy can fail with `StorageFull` (ENOSPC)
/// even though the credentials themselves are tiny. When that happens we
/// retry with an in-place truncate+write, which only needs the freed blocks
/// of the old file — far less than the temp-copy approach.
///
/// The in-place path is non-atomic, with two accepted trade-offs:
/// - If the in-place write itself fails (e.g. a concurrent process grabs the
///   just-freed blocks, or a crash mid-write), the prior bytes are restored
///   best-effort so a torn/empty file never *replaces* the previous on-disk
///   credential — on-disk state ends up no worse than before the attempt.
/// - Unlocked concurrent readers can still observe a torn (partial) file
///   during the brief write window; a partial file is healed on the next
///   read via [`read_auth_json_or_empty_recovering_corrupt`] (backup +
///   relogin). This window is inherent to any sub-1×-free single-file
///   replace and is preferable to persisting nothing at all, which would
///   leave every concurrent process with a stale, already-revoked token.
pub(crate) fn write_auth_json(auth_file: &Path, auth_store: &AuthStore) -> std::io::Result<()> {
    write_auth_json_with(auth_file, auth_store, write_auth_json_atomic)
}

/// Dispatch helper: run `atomic`, and on `StorageFull` fall back to an
/// in-place write. Split out (with `atomic` injectable) so the disk-full
/// fallback is unit-testable without an actually-full filesystem.
fn write_auth_json_with(
    auth_file: &Path,
    auth_store: &AuthStore,
    atomic: fn(&Path, &AuthStore) -> std::io::Result<()>,
) -> std::io::Result<()> {
    match atomic(auth_file, auth_store) {
        Err(e) if e.kind() == std::io::ErrorKind::StorageFull => {
            tracing::warn!(
                path = %auth_file.display(),
                "auth: disk full during atomic write, falling back to in-place write"
            );
            // Must reach unified.jsonl: a silent in-memory-only credential
            // (the prior behavior) leaves sibling processes with a stale
            // refresh token and no record of why. Surface it loudly.
            xai_grok_telemetry::unified_log::warn(
                "auth: disk full, falling back to non-atomic in-place write",
                None,
                Some(serde_json::json!({
                    "path": auth_file.display().to_string(),
                })),
            );
            write_auth_json_in_place(auth_file, auth_store)
        }
        other => other,
    }
}

/// Serialize `auth_store` to `path` (truncate + rewrite), owner-only (0o600)
/// and `fsync`'d. Shared core of the atomic path (which targets the temp
/// file) and the in-place fallback (which targets `auth.json` directly).
///
/// Uses streaming `to_writer_pretty` through a `BufWriter` to avoid
/// allocating the entire JSON string in memory — eliminates OOM risk under
/// severe memory pressure.
fn write_store_to(path: &Path, auth_store: &AuthStore) -> std::io::Result<()> {
    use crate::util::secure_file::open_secure_file;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = open_secure_file(path)?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, auth_store)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writer.flush()?;
    writer
        .into_inner()
        .map_err(|e| e.into_error())?
        .sync_all()?;
    // `open_secure_file` mode bits apply only on create; tighten existing paths.
    // Best-effort after durable content: a chmod-only failure must not look
    // like a failed write. The in-place fallback restores the prior snapshot
    // on any `write_store_to` Err, which would discard freshly written tokens.
    // Load path re-tightens on next read.
    if let Err(e) = crate::util::secure_file::ensure_owner_only_permissions(path) {
        tracing::warn!(
            error = %e,
            path = %path.display(),
            "auth: failed to ensure owner-only permissions after write"
        );
    }
    Ok(())
}

/// Atomic write: tmp + rename. Unix `rename(2)` replaces atomically;
/// Windows `rename` requires removing the target first.
fn write_auth_json_atomic(auth_file: &Path, auth_store: &AuthStore) -> std::io::Result<()> {
    let tmp = auth_file.with_extension(format!("json.{}.tmp", std::process::id()));
    write_store_to(&tmp, auth_store)?;
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(auth_file);
    }
    std::fs::rename(&tmp, auth_file)?;
    // Re-assert on the final path (covers rename edge cases / FS quirks).
    // Best-effort: rename already published the new tokens.
    if let Err(e) = crate::util::secure_file::ensure_owner_only_permissions(auth_file) {
        tracing::warn!(
            error = %e,
            path = %auth_file.display(),
            "auth: failed to ensure owner-only permissions after rename"
        );
    }
    Ok(())
}

/// Non-atomic fallback: truncate and rewrite `auth.json` in place.
///
/// Used only when [`write_auth_json_atomic`] fails with `StorageFull`.
/// Opening with truncation first frees the old content's blocks before the
/// new bytes are written, so this needs only the file size in free space
/// rather than the temp-copy approach's file-size-of-headroom.
///
/// Truncation is destructive, so the prior bytes are snapshotted first and
/// restored best-effort if the rewrite fails partway — a failed fallback
/// must not leave an empty/torn file where a parseable (if stale) credential
/// used to be. A partial file that survives (because even the restore failed)
/// is healed on the next read via [`read_auth_json_or_empty_recovering_corrupt`].
fn write_auth_json_in_place(auth_file: &Path, auth_store: &AuthStore) -> std::io::Result<()> {
    write_auth_json_in_place_with(auth_file, auth_store, write_store_to)
}

/// Inner of [`write_auth_json_in_place`] with `write` injectable so the
/// rollback-on-failure path is unit-testable without an actually-full disk.
fn write_auth_json_in_place_with(
    auth_file: &Path,
    auth_store: &AuthStore,
    write: fn(&Path, &AuthStore) -> std::io::Result<()>,
) -> std::io::Result<()> {
    // Snapshot the prior bytes so a torn/empty write can be rolled back to
    // the previous on-disk credential. `None` when the file is absent.
    let prior = std::fs::read(auth_file).ok();
    match write(auth_file, auth_store) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(prior) = prior
                && let Err(restore_err) = restore_prior_bytes(auth_file, &prior)
            {
                tracing::warn!(
                    error = %restore_err,
                    "auth: failed to restore prior auth.json after in-place write failure"
                );
            }
            Err(e)
        }
    }
}

/// Best-effort rollback: rewrite `bytes` (owner-only, `fsync`'d) after a
/// failed in-place write so a torn/empty file does not replace the prior
/// credential.
fn restore_prior_bytes(auth_file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use crate::util::secure_file::open_secure_file;

    let mut file = open_secure_file(auth_file)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    crate::util::secure_file::ensure_owner_only_permissions(auth_file)?;
    Ok(())
}

/// True for the private OAuth metadata sidecar grammar and practical
/// lookalikes (`…::oauth::meta`). These are binding records, never tokens.
fn is_oauth_meta_scope_or_lookalike(scope: &str) -> bool {
    scope.ends_with("::oauth::meta")
}

/// Read a single auth token from `auth.json` by scope key.
/// Falls back to the legacy `https://accounts.x.ai/sign-in` scope key
/// when the requested scope is not found (devbox auth.json migration).
///
/// Rejects private OAuth metadata sidecars (`provider::<id>::oauth::meta`
/// and lookalikes) with `InvalidInput` so a raw key walk cannot surface the
/// marker as if it were a credential.
pub fn read_token_by_scope(grok_home: &Path, scope: &str) -> anyhow::Result<String> {
    if is_oauth_meta_scope_or_lookalike(scope) {
        return Err(anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "private OAuth metadata scope is not a credential",
        )));
    }
    let path = grok_home.join("auth.json");
    let store = read_auth_json(&path).map_err(|_| {
        anyhow::anyhow!(
            "Not logged in. Connect xAI in /providers (or run `grok provider connect xai`)."
        )
    })?;
    lookup_auth(&store, scope).map(|a| a.key).ok_or_else(|| {
        anyhow::anyhow!(
            "Your auth token is invalid. Connect xAI in /providers (or run `grok provider connect xai`)."
        )
    })
}

/// Read the API key from the `xai::api_key` scope in auth.json.
pub fn read_api_key(grok_home: &Path) -> Option<String> {
    let path = grok_home.join("auth.json");
    let map = read_auth_json(&path).ok()?;
    map.get(API_KEY_SCOPE).map(|a| a.key.clone())
}

/// Store a plain API key in auth.json under the `xai::api_key` scope.
///
/// Uses the corrupt-recovery reader so a malformed auth.json (e.g. from a
/// previous crash) can be healed when the user sets an API key.
pub fn store_api_key(grok_home: &Path, api_key: &str) -> std::io::Result<()> {
    let path = grok_home.join("auth.json");
    let mut map = read_auth_json_or_empty_recovering_corrupt(&path)?;
    map.insert(
        API_KEY_SCOPE.to_owned(),
        GrokAuth {
            key: api_key.to_owned(),
            auth_mode: AuthMode::ApiKey,
            ..Default::default()
        },
    );
    write_auth_json(&path, &map)
}

/// Remove the `xai::api_key` scope from auth.json.
pub fn clear_api_key(grok_home: &Path) -> std::io::Result<()> {
    let path = grok_home.join("auth.json");
    if let Ok(mut map) = read_auth_json(&path) {
        map.remove(API_KEY_SCOPE);
        if map.is_empty() {
            let _ = std::fs::remove_file(&path);
        } else {
            write_auth_json(&path, &map)?;
        }
    }
    Ok(())
}

/// Provider-specific API-key scope names. These scopes intentionally share
/// the established owner-only, atomic `auth.json` store while remaining
/// separate from xAI's `xai::api_key` and OAuth entries.
pub const OPENAI_API_KEY_SCOPE: &str = "openai::api_key";
/// ChatGPT / OpenAI subscription OAuth (access + refresh). This may coexist
/// with [`OPENAI_API_KEY_SCOPE`]; the selected model route chooses OAuth for
/// Codex endpoints or the Platform API key for `api.openai.com`.
pub const OPENAI_OAUTH_SCOPE: &str = "openai::oauth";
pub const OPENROUTER_API_KEY_SCOPE: &str = "openrouter::api_key";
/// Direct Anthropic Messages API key (`x-api-key`). Never falls through to xAI.
pub const ANTHROPIC_API_KEY_SCOPE: &str = "anthropic::api_key";
/// Optional OpenAI administration key (organization APIs only).
pub const OPENAI_ADMIN_KEY_SCOPE: &str = "openai::admin_key";

fn validate_provider_scope(scope: &str) -> std::io::Result<()> {
    // Built-in product scopes plus validated per-instance openai_compatible scopes.
    if matches!(
        scope,
        OPENAI_API_KEY_SCOPE
            | OPENROUTER_API_KEY_SCOPE
            | ANTHROPIC_API_KEY_SCOPE
            | OPENAI_ADMIN_KEY_SCOPE
    ) {
        return Ok(());
    }
    if crate::provider_registry::secrets::is_allowed_provider_scope(scope) {
        return Ok(());
    }
    Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
}

/// Marker stored in the `key` field of an API-key binding record so the entry
/// can never be mistaken for token material.
const PROVIDER_API_KEY_META_MARKER: &str = "meta";

/// Secret-free binding for a provider API-key scope. Generation is monotonic
/// across replace/clear so a stale route cannot attribute against a rotated
/// key. Never derived from the secret value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderApiKeyBinding {
    pub generation: u64,
}

fn api_key_meta_scope(scope: &str) -> String {
    format!("{scope}::meta")
}

fn encode_provider_api_key_meta(binding: &ProviderApiKeyBinding) -> GrokAuth {
    GrokAuth {
        key: PROVIDER_API_KEY_META_MARKER.to_owned(),
        auth_mode: AuthMode::ApiKey,
        create_time: chrono::Utc::now(),
        user_id: binding.generation.to_string(),
        ..Default::default()
    }
}

fn decode_provider_api_key_meta(entry: &GrokAuth) -> std::io::Result<ProviderApiKeyBinding> {
    if entry.key != PROVIDER_API_KEY_META_MARKER || entry.refresh_token.is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "malformed provider API-key binding record",
        ));
    }
    let generation = entry
        .user_id
        .parse::<u64>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(ProviderApiKeyBinding { generation })
}

/// Read the secret-free binding generation for a provider API-key scope.
///
/// `None` means a legacy key without metadata (generation contract `0` only).
pub fn read_provider_api_key_binding(
    grok_home: &Path,
    scope: &str,
) -> std::io::Result<Option<ProviderApiKeyBinding>> {
    validate_provider_scope(scope)?;
    let path = grok_home.join("auth.json");
    let store = match read_auth_json(&path) {
        Ok(store) => store,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match store.get(&api_key_meta_scope(scope)) {
        Some(meta) => decode_provider_api_key_meta(meta).map(Some),
        None => Ok(None),
    }
}

/// Read an API key from an explicitly provider-scoped auth entry. Missing
/// stores/scopes are normal; unreadable or malformed stores fail closed.
/// Ordinary reads remain non-destructive and do not require binding metadata.
pub fn read_provider_api_key(grok_home: &Path, scope: &str) -> std::io::Result<Option<String>> {
    validate_provider_scope(scope)?;
    let path = grok_home.join("auth.json");
    match read_auth_json(&path) {
        Ok(store) => Ok(store.get(scope).map(|auth| auth.key.clone())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Atomically store a provider API key under `scope`, while holding the same
/// live `auth.json.lock` used by the normal authentication paths.  Lock
/// contention and stale-lock races are errors so a UI action never clobbers a
/// concurrent OAuth refresh.
///
/// Also bumps the secret-free binding generation for exact-route attribution.
/// Returns the new generation. Does not migrate or copy secrets across scopes.
pub fn store_provider_api_key(
    grok_home: &Path,
    scope: &str,
    api_key: &str,
) -> std::io::Result<u64> {
    validate_provider_scope(scope)?;
    let path = grok_home.join("auth.json");
    let lock = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&path)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
    if !lock.still_live(&path) {
        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    }
    let mut store = read_auth_json_or_empty(&path)?;
    let meta_scope = api_key_meta_scope(scope);
    let prev_generation = match store.get(&meta_scope) {
        Some(meta) => decode_provider_api_key_meta(meta)?.generation,
        None => 0,
    };
    let generation = prev_generation.saturating_add(1).max(1);
    store.insert(
        scope.to_owned(),
        GrokAuth {
            key: api_key.to_owned(),
            auth_mode: AuthMode::ApiKey,
            ..Default::default()
        },
    );
    store.insert(
        meta_scope,
        encode_provider_api_key_meta(&ProviderApiKeyBinding { generation }),
    );
    if !lock.still_live(&path) {
        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    }
    write_auth_json(&path, &store)?;
    Ok(generation)
}

/// Remove one provider API key without affecting any xAI/OAuth scope.
///
/// Bumps the binding generation so a delayed route cannot attribute against
/// the cleared credential; the meta record is retained for monotonicity.
pub fn clear_provider_api_key(grok_home: &Path, scope: &str) -> std::io::Result<()> {
    validate_provider_scope(scope)?;
    let path = grok_home.join("auth.json");
    let lock = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&path)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
    if !lock.still_live(&path) {
        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    }
    let mut store = match read_auth_json(&path) {
        Ok(store) => store,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let had_key = store.remove(scope).is_some();
    let meta_scope = api_key_meta_scope(scope);
    let prev_generation = match store.get(&meta_scope) {
        Some(meta) => decode_provider_api_key_meta(meta)?.generation,
        None => 0,
    };
    if had_key || prev_generation > 0 {
        let generation = prev_generation.saturating_add(1).max(1);
        store.insert(
            meta_scope,
            encode_provider_api_key_meta(&ProviderApiKeyBinding { generation }),
        );
    }
    if !lock.still_live(&path) {
        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    }
    // Keep an empty auth.json rather than deleting it: deletion can race a
    // sibling reader/writer and is not needed to remove the credential.
    write_auth_json(&path, &store)
}

/// Validate a configured-provider OAuth scope (`provider::<id>::oauth`).
///
/// The built-in `openai::oauth` scope and the private `...::oauth::meta`
/// sidecar are never accepted here: configured OAuth must not fall back to
/// (or be written to) the built-in ChatGPT route.
fn validate_provider_oauth_scope(scope: &str) -> std::io::Result<()> {
    if crate::provider_registry::secrets::is_allowed_oauth_scope(scope) {
        return Ok(());
    }
    Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
}

/// Marker stored in the `key` field of a binding-record entry so the entry can
/// never be mistaken for token material (`refresh_token` is also absent).
const PROVIDER_OAUTH_META_MARKER: &str = "meta";

/// Encode a secret-free binding record as a plain `GrokAuth` entry under the
/// private `provider::<id>::oauth::meta` scope. Uses only existing `GrokAuth`
/// fields so older readers parse and preserve it as an opaque sibling entry.
fn encode_provider_oauth_meta(binding: &ProviderOAuthBinding) -> GrokAuth {
    GrokAuth {
        key: PROVIDER_OAUTH_META_MARKER.to_owned(),
        auth_mode: AuthMode::ApiKey,
        create_time: chrono::Utc::now(),
        user_id: binding.generation.to_string(),
        organization_id: binding.incarnation.as_ref().map(|i| i.as_str().to_owned()),
        ..Default::default()
    }
}

/// Decode a binding record, failing closed on any malformed or token-bearing
/// payload so a corrupted sidecar can never expose token material.
fn decode_provider_oauth_meta(
    provider_id: &ProviderId,
    entry: &GrokAuth,
) -> std::io::Result<ProviderOAuthBinding> {
    if entry.key != PROVIDER_OAUTH_META_MARKER || entry.refresh_token.is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "malformed provider OAuth binding record",
        ));
    }
    let generation = entry
        .user_id
        .parse::<u64>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let incarnation = entry
        .organization_id
        .as_deref()
        .map(ProviderIncarnation::new)
        .transpose()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(ProviderOAuthBinding {
        provider_id: provider_id.clone(),
        incarnation,
        generation,
    })
}

/// Read the persisted secret-free binding record for a configured provider's
/// OAuth account. Survives token deletion so generation stays monotonic across
/// logout/login.
pub fn read_provider_oauth_binding(
    grok_home: &Path,
    provider_id: &ProviderId,
) -> std::io::Result<Option<ProviderOAuthBinding>> {
    let meta_scope = crate::provider_registry::secrets::oauth_meta_scope_string(provider_id);
    let path = grok_home.join("auth.json");
    let store = match read_auth_json(&path) {
        Ok(store) => store,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match store.get(&meta_scope) {
        Some(meta) => decode_provider_oauth_meta(provider_id, meta).map(Some),
        None => Ok(None),
    }
}

/// Read the stored configured-provider OAuth credential for an exact provider
/// instance and incarnation.
///
/// `None` means an explicitly incarnation-less route and never matches a
/// stored `Some(incarnation)`. Returns `None` (never the built-in
/// `openai::oauth` credential) when the scope is absent or the stored binding
/// does not match exactly, so a stale or mis-bound entry is never reused.
pub fn read_provider_oauth_auth(
    grok_home: &Path,
    provider_id: &ProviderId,
    incarnation: Option<&ProviderIncarnation>,
) -> std::io::Result<Option<GrokAuth>> {
    let scope = crate::provider_registry::secrets::oauth_scope_string(provider_id);
    validate_provider_oauth_scope(&scope)?;
    let path = grok_home.join("auth.json");
    let store = match read_auth_json(&path) {
        Ok(store) => store,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let Some(token) = store.get(&scope) else {
        return Ok(None);
    };
    // A token written before binding records existed is incarnation-less.
    let stored_incarnation = match store.get(
        &crate::provider_registry::secrets::oauth_meta_scope_string(provider_id),
    ) {
        Some(meta) => decode_provider_oauth_meta(provider_id, meta)?.incarnation,
        None => None,
    };
    if stored_incarnation.as_ref() != incarnation {
        return Ok(None);
    }
    Ok(Some(token.clone()))
}

/// Atomically store a configured-provider OAuth credential, bumping the
/// account's generation.
///
/// Returns the new binding. A store whose incarnation differs from the stored
/// binding is rejected while token material exists, so a stale login/refresh
/// completion can never clobber a live credential for a different incarnation;
/// a new incarnation takes ownership only after the old credential is cleared.
pub fn store_provider_oauth_auth(
    grok_home: &Path,
    provider_id: &ProviderId,
    incarnation: Option<&ProviderIncarnation>,
    auth: &GrokAuth,
) -> std::io::Result<ProviderOAuthBinding> {
    let scope = crate::provider_registry::secrets::oauth_scope_string(provider_id);
    validate_provider_oauth_scope(&scope)?;
    let meta_scope = crate::provider_registry::secrets::oauth_meta_scope_string(provider_id);
    let path = grok_home.join("auth.json");
    let lock = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&path)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
    if !lock.still_live(&path) {
        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    }
    let mut store = read_auth_json_or_empty(&path)?;
    let existing_meta = match store.get(&meta_scope) {
        Some(meta) => Some(decode_provider_oauth_meta(provider_id, meta)?),
        None => None,
    };
    // A present token with no meta is incarnation-less (`None`), matching
    // read/clear. Reject any store with a different incarnation until the
    // exact credential is cleared.
    let stored_incarnation = existing_meta.as_ref().and_then(|m| m.incarnation.as_ref());
    if store.contains_key(&scope) && stored_incarnation != incarnation {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "credential belongs to a different provider incarnation",
        ));
    }
    let generation = existing_meta
        .as_ref()
        .map(|m| m.generation.saturating_add(1))
        .unwrap_or(0);
    let binding = ProviderOAuthBinding {
        provider_id: provider_id.clone(),
        incarnation: incarnation.cloned(),
        generation,
    };
    store.insert(scope, auth.clone());
    store.insert(meta_scope, encode_provider_oauth_meta(&binding));
    if !lock.still_live(&path) {
        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    }
    write_auth_json(&path, &store)?;
    Ok(binding)
}

/// Remove the configured-provider OAuth credential for an exact provider
/// instance and incarnation.
///
/// Only removes when the stored binding matches exactly, so logout never
/// removes a sibling account or a stale incarnation's credential. Rotates the
/// generation and keeps the binding record so the next store increments rather
/// than resets.
pub fn clear_provider_oauth_auth(
    grok_home: &Path,
    provider_id: &ProviderId,
    incarnation: Option<&ProviderIncarnation>,
) -> std::io::Result<()> {
    let scope = crate::provider_registry::secrets::oauth_scope_string(provider_id);
    validate_provider_oauth_scope(&scope)?;
    let meta_scope = crate::provider_registry::secrets::oauth_meta_scope_string(provider_id);
    let path = grok_home.join("auth.json");
    let lock = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&path)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
    if !lock.still_live(&path) {
        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    }
    let mut store = match read_auth_json(&path) {
        Ok(store) => store,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let existing_meta = match store.get(&meta_scope) {
        Some(meta) => Some(decode_provider_oauth_meta(provider_id, meta)?),
        None => None,
    };
    let has_token = store.contains_key(&scope);
    // Only remove when the stored binding matches the requested incarnation
    // exactly; otherwise the credential belongs to another incarnation.
    let matches = match &existing_meta {
        Some(meta) => meta.incarnation.as_ref() == incarnation,
        // A legacy token with no binding record is incarnation-less.
        None => incarnation.is_none(),
    };
    if !has_token || !matches {
        return Ok(());
    }
    store.remove(&scope);
    let generation = existing_meta
        .as_ref()
        .map(|m| m.generation.saturating_add(1))
        .unwrap_or(0);
    let binding = ProviderOAuthBinding {
        provider_id: provider_id.clone(),
        incarnation: incarnation.cloned(),
        generation,
    };
    store.insert(meta_scope, encode_provider_oauth_meta(&binding));
    if !lock.still_live(&path) {
        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    }
    write_auth_json(&path, &store)
}

#[cfg(test)]
mod write_fallback_tests {
    use super::*;

    fn sample_store() -> AuthStore {
        let mut map = AuthStore::new();
        map.insert(
            API_KEY_SCOPE.to_owned(),
            GrokAuth {
                key: "secret-key".to_owned(),
                auth_mode: AuthMode::ApiKey,
                ..Default::default()
            },
        );
        map
    }

    fn read_key(path: &Path) -> Option<String> {
        read_auth_json(path)
            .ok()
            .and_then(|m| m.get(API_KEY_SCOPE).map(|a| a.key.clone()))
    }

    #[test]
    fn provider_scope_mutation_preserves_xai_api_key_and_rejects_unknown_scopes() {
        let dir = tempfile::tempdir().unwrap();
        store_api_key(dir.path(), "xai-key").unwrap();
        store_provider_api_key(dir.path(), OPENAI_API_KEY_SCOPE, "openai-key").unwrap();
        assert_eq!(read_api_key(dir.path()).as_deref(), Some("xai-key"));
        assert_eq!(
            read_provider_api_key(dir.path(), OPENAI_API_KEY_SCOPE)
                .unwrap()
                .as_deref(),
            Some("openai-key")
        );
        assert!(store_provider_api_key(dir.path(), API_KEY_SCOPE, "overwrite-attempt").is_err());
        clear_provider_api_key(dir.path(), OPENAI_API_KEY_SCOPE).unwrap();
        assert_eq!(read_api_key(dir.path()).as_deref(), Some("xai-key"));
    }

    #[test]
    fn provider_write_fails_closed_while_auth_writer_holds_shared_lock() {
        let dir = tempfile::tempdir().unwrap();
        store_api_key(dir.path(), "xai-key").unwrap();
        let auth_path = dir.path().join("auth.json");
        let lock = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&auth_path)
            .expect("test acquires auth lock");
        let error = store_provider_api_key(dir.path(), OPENROUTER_API_KEY_SCOPE, "router-key")
            .expect_err("concurrent auth writer must not be overwritten");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        drop(lock);
        assert_eq!(read_api_key(dir.path()).as_deref(), Some("xai-key"));
        assert_eq!(
            read_provider_api_key(dir.path(), OPENROUTER_API_KEY_SCOPE).unwrap(),
            None
        );
    }

    fn fake_storage_full(_: &Path, _: &AuthStore) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
    }

    fn fake_permission_denied(_: &Path, _: &AuthStore) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    }

    /// Simulates an in-place write that truncates the file (destroying the
    /// old content, as `open_secure_file` does) and then fails partway — the
    /// torn-write case the rollback must recover from.
    fn fake_truncate_then_fail(path: &Path, _: &AuthStore) -> std::io::Result<()> {
        crate::util::secure_file::open_secure_file(path)?; // truncates to 0 bytes
        Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
    }

    #[test]
    fn in_place_write_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json_in_place(&path, &sample_store()).unwrap();
        assert_eq!(read_key(&path).as_deref(), Some("secret-key"));
    }

    #[cfg(unix)]
    #[test]
    fn in_place_write_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json_in_place(&path, &sample_store()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "in-place write must stay 0o600");
    }

    #[cfg(unix)]
    #[test]
    fn write_tightens_preexisting_world_readable_auth_json() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        std::fs::write(&path, b"{}").unwrap();
        let mut loose = std::fs::metadata(&path).unwrap().permissions();
        loose.set_mode(0o644);
        std::fs::set_permissions(&path, loose).unwrap();

        write_auth_json(&path, &sample_store()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "rewrite must tighten preexisting open perms"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_tightens_world_readable_auth_json() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json(&path, &sample_store()).unwrap();
        let mut loose = std::fs::metadata(&path).unwrap().permissions();
        loose.set_mode(0o644);
        std::fs::set_permissions(&path, loose).unwrap();

        let _ = read_auth_json(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "load must tighten open auth.json perms"
        );
    }

    /// A `StorageFull` (ENOSPC) failure on the atomic path must fall back to
    /// the in-place write so the credential still lands on disk.
    #[test]
    fn falls_back_to_in_place_on_storage_full() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json_with(&path, &sample_store(), fake_storage_full).unwrap();
        assert_eq!(
            read_key(&path).as_deref(),
            Some("secret-key"),
            "disk-full atomic write must fall back to a successful in-place write"
        );
    }

    /// Non-ENOSPC errors must propagate unchanged and must NOT trigger the
    /// in-place fallback (e.g. a permission error should not write the file).
    #[test]
    fn propagates_non_storage_full_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let err = write_auth_json_with(&path, &sample_store(), fake_permission_denied).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(!path.exists(), "non-ENOSPC failure must not write the file");
    }

    /// The normal (real atomic) path still works end to end.
    #[test]
    fn atomic_write_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json(&path, &sample_store()).unwrap();
        assert_eq!(read_key(&path).as_deref(), Some("secret-key"));
    }

    /// A fallback write that truncates then fails must roll back to the prior
    /// bytes instead of leaving an empty/torn file — otherwise a second
    /// disk-full failure would destroy a previously-valid credential.
    #[test]
    fn in_place_restores_prior_bytes_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        // Seed a valid prior credential.
        write_auth_json_in_place(&path, &sample_store()).unwrap();
        assert_eq!(read_key(&path).as_deref(), Some("secret-key"));

        let mut replacement = AuthStore::new();
        replacement.insert(
            API_KEY_SCOPE.to_owned(),
            GrokAuth {
                key: "replacement-key".to_owned(),
                auth_mode: AuthMode::ApiKey,
                ..Default::default()
            },
        );
        let err = write_auth_json_in_place_with(&path, &replacement, fake_truncate_then_fail)
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::StorageFull);
        assert_eq!(
            read_key(&path).as_deref(),
            Some("secret-key"),
            "a failed in-place write must restore the prior credential, not leave an empty file"
        );
    }

    /// Rollback after a failed write must keep the file owner-only (0o600).
    #[cfg(unix)]
    #[test]
    fn in_place_restore_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        write_auth_json_in_place(&path, &sample_store()).unwrap();
        let _ = write_auth_json_in_place_with(&path, &sample_store(), fake_truncate_then_fail);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "restored file must stay 0o600");
    }

    // ── Configured OAuth storage ──────────────────────────────────────────

    fn oauth_auth(key: &str) -> GrokAuth {
        GrokAuth {
            key: key.to_owned(),
            auth_mode: AuthMode::Oidc,
            user_id: "chatgpt".to_owned(),
            refresh_token: Some(format!("rt-{key}")),
            ..Default::default()
        }
    }

    fn pid(id: &str) -> ProviderId {
        ProviderId::new(id).unwrap()
    }

    fn incarnation(suffix: u32) -> ProviderIncarnation {
        ProviderIncarnation::new(format!("123e4567-e89b-12d3-a456-42661417{suffix:04}")).unwrap()
    }

    #[test]
    fn oauth_scope_validation_rejects_builtin_and_malformed() {
        assert!(
            validate_provider_oauth_scope(&crate::provider_registry::secrets::oauth_scope_string(
                &pid("foo")
            ))
            .is_ok()
        );
        assert!(
            validate_provider_oauth_scope(OPENAI_OAUTH_SCOPE).is_err(),
            "built-in openai::oauth must never validate as a configured route"
        );
        assert!(validate_provider_oauth_scope(OPENAI_API_KEY_SCOPE).is_err());
        assert!(validate_provider_oauth_scope("provider::BAD::oauth").is_err());
        assert!(validate_provider_oauth_scope("provider::foo::api_key").is_err());
        assert!(
            validate_provider_oauth_scope(
                &crate::provider_registry::secrets::oauth_meta_scope_string(&pid("foo"))
            )
            .is_err()
        );
    }

    #[test]
    fn configured_oauth_siblings_isolated_with_no_builtin_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        store_provider_oauth_auth(home, &pid("alpha"), None, &oauth_auth("a1")).unwrap();
        store_provider_oauth_auth(home, &pid("beta"), None, &oauth_auth("b1")).unwrap();
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), None)
                .unwrap()
                .unwrap()
                .key,
            "a1"
        );
        assert_eq!(
            read_provider_oauth_auth(home, &pid("beta"), None)
                .unwrap()
                .unwrap()
                .key,
            "b1"
        );
        // Built-in scope is never returned from configured helpers.
        let store = read_auth_json(&home.join("auth.json")).unwrap();
        assert!(!store.contains_key(OPENAI_OAUTH_SCOPE));
        clear_provider_oauth_auth(home, &pid("alpha"), None).unwrap();
        assert!(
            read_provider_oauth_auth(home, &pid("alpha"), None)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            read_provider_oauth_auth(home, &pid("beta"), None)
                .unwrap()
                .unwrap()
                .key,
            "b1"
        );
    }

    #[test]
    fn oauth_generation_rotation_affects_only_selected_account() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc_a = incarnation(1);
        let b0 = store_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a), &oauth_auth("a1"))
            .unwrap();
        assert_eq!(b0.generation, 0);
        let b1 = store_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a), &oauth_auth("a2"))
            .unwrap();
        assert_eq!(b1.generation, 1);
        assert!(b1.incarnation.is_some());
        let b_beta =
            store_provider_oauth_auth(home, &pid("beta"), None, &oauth_auth("b1")).unwrap();
        assert_eq!(b_beta.generation, 0);
        assert_eq!(
            read_provider_oauth_binding(home, &pid("alpha"))
                .unwrap()
                .unwrap()
                .generation,
            1
        );
    }

    #[test]
    fn oauth_binding_mismatch_fails_closed_on_read_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc_a = incarnation(2);
        let inc_b = incarnation(3);
        store_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a), &oauth_auth("a1")).unwrap();
        assert!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc_b))
                .unwrap()
                .is_none(),
            "incarnation mismatch must fail closed on read"
        );
        assert!(
            read_provider_oauth_auth(home, &pid("alpha"), None)
                .unwrap()
                .is_none(),
            "None must not match a stored Some(incarnation)"
        );
        clear_provider_oauth_auth(home, &pid("alpha"), Some(&inc_b)).unwrap();
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a))
                .unwrap()
                .unwrap()
                .key,
            "a1"
        );
        clear_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a)).unwrap();
        assert!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn oauth_stale_incarnation_store_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc_a = incarnation(6);
        let inc_b = incarnation(7);
        store_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a), &oauth_auth("a1")).unwrap();
        let err = store_provider_oauth_auth(home, &pid("alpha"), Some(&inc_b), &oauth_auth("b1"))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a))
                .unwrap()
                .unwrap()
                .key,
            "a1",
            "stale store must not replace the fresh credential"
        );
        assert!(store_provider_oauth_auth(home, &pid("alpha"), None, &oauth_auth("n1")).is_err());
        clear_provider_oauth_auth(home, &pid("alpha"), Some(&inc_a)).unwrap();
        let b = store_provider_oauth_auth(home, &pid("alpha"), Some(&inc_b), &oauth_auth("b1"))
            .unwrap();
        assert_eq!(b.generation, 2);
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc_b))
                .unwrap()
                .unwrap()
                .key,
            "b1"
        );
    }

    #[test]
    fn oauth_generation_survives_clear() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = incarnation(8);
        let b0 =
            store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("a1")).unwrap();
        assert_eq!(b0.generation, 0);
        clear_provider_oauth_auth(home, &pid("alpha"), Some(&inc)).unwrap();
        assert!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc))
                .unwrap()
                .is_none(),
            "reading after clear returns absent"
        );
        assert_eq!(
            read_provider_oauth_binding(home, &pid("alpha"))
                .unwrap()
                .unwrap()
                .generation,
            1,
            "binding record survives clear"
        );
        let b1 =
            store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("a2")).unwrap();
        assert_eq!(b1.generation, 2, "next store increments, never resets");
    }

    #[test]
    fn oauth_meta_round_trip_is_secret_free() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = incarnation(9);
        store_provider_oauth_auth(
            home,
            &pid("alpha"),
            Some(&inc),
            &oauth_auth("access-token-value"),
        )
        .unwrap();
        let store = read_auth_json(&home.join("auth.json")).unwrap();
        let meta = store
            .get(&crate::provider_registry::secrets::oauth_meta_scope_string(
                &pid("alpha"),
            ))
            .unwrap();
        assert_eq!(meta.key, "meta");
        assert!(meta.refresh_token.is_none());
        let json = serde_json::to_string(meta).unwrap();
        assert!(!json.contains("access-token-value"));
        assert!(!json.contains("rt-"));
    }

    #[test]
    fn oauth_meta_malformed_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = incarnation(10);
        store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("a1")).unwrap();
        let path = home.join("auth.json");
        let mut store = read_auth_json(&path).unwrap();
        let mut meta = store
            .get(&crate::provider_registry::secrets::oauth_meta_scope_string(
                &pid("alpha"),
            ))
            .unwrap()
            .clone();
        meta.user_id = "not-a-number".into();
        store.insert(
            crate::provider_registry::secrets::oauth_meta_scope_string(&pid("alpha")),
            meta,
        );
        write_auth_json(&path, &store).unwrap();
        assert!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc)).is_err(),
            "malformed metadata must fail closed on read"
        );
        assert!(
            store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("a2")).is_err(),
            "malformed metadata must fail closed on store"
        );
    }

    #[test]
    fn legacy_token_without_meta_is_incarnation_less() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = incarnation(11);
        let path = home.join("auth.json");
        let mut store = read_auth_json_or_empty(&path).unwrap();
        store.insert(
            crate::provider_registry::secrets::oauth_scope_string(&pid("alpha")),
            oauth_auth("legacy"),
        );
        write_auth_json(&path, &store).unwrap();
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), None)
                .unwrap()
                .unwrap()
                .key,
            "legacy"
        );
        assert!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc))
                .unwrap()
                .is_none()
        );
        clear_provider_oauth_auth(home, &pid("alpha"), Some(&inc)).unwrap();
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), None)
                .unwrap()
                .unwrap()
                .key,
            "legacy"
        );
        clear_provider_oauth_auth(home, &pid("alpha"), None).unwrap();
        assert!(
            read_provider_oauth_auth(home, &pid("alpha"), None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn meta_less_token_rejects_some_incarnation_store_until_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = incarnation(13);
        let path = home.join("auth.json");
        let mut store = read_auth_json_or_empty(&path).unwrap();
        store.insert(
            crate::provider_registry::secrets::oauth_scope_string(&pid("alpha")),
            oauth_auth("legacy-live"),
        );
        write_auth_json(&path, &store).unwrap();
        let err =
            store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("clobber"))
                .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), None)
                .unwrap()
                .unwrap()
                .key,
            "legacy-live",
            "stale Some(incarnation) must not clobber a meta-less token"
        );
        assert!(
            read_provider_oauth_binding(home, &pid("alpha"))
                .unwrap()
                .is_none()
        );
        clear_provider_oauth_auth(home, &pid("alpha"), None).unwrap();
        let b = store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("owned"))
            .unwrap();
        assert_eq!(b.incarnation.as_ref(), Some(&inc));
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), Some(&inc))
                .unwrap()
                .unwrap()
                .key,
            "owned"
        );
    }

    #[test]
    fn clear_twice_does_not_rotate_generation() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = incarnation(14);
        store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("a1")).unwrap();
        clear_provider_oauth_auth(home, &pid("alpha"), Some(&inc)).unwrap();
        let gen_after_logout = read_provider_oauth_binding(home, &pid("alpha"))
            .unwrap()
            .unwrap()
            .generation;
        assert_eq!(gen_after_logout, 1);
        clear_provider_oauth_auth(home, &pid("alpha"), Some(&inc)).unwrap();
        assert_eq!(
            read_provider_oauth_binding(home, &pid("alpha"))
                .unwrap()
                .unwrap()
                .generation,
            gen_after_logout,
            "repeated clear must not bump generation"
        );
    }

    #[test]
    fn clear_missing_file_does_not_create_tombstone() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let path = home.join("auth.json");
        assert!(!path.exists());
        clear_provider_oauth_auth(home, &pid("alpha"), None).unwrap();
        assert!(
            !path.exists(),
            "clear of a missing store must not create auth.json"
        );
        assert!(
            read_provider_oauth_binding(home, &pid("alpha"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn read_token_by_scope_rejects_oauth_meta_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = incarnation(15);
        store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("token")).unwrap();
        let meta = crate::provider_registry::secrets::oauth_meta_scope_string(&pid("alpha"));
        let err = read_token_by_scope(home, &meta).unwrap_err();
        let io = err
            .downcast_ref::<std::io::Error>()
            .expect("meta reject is InvalidInput io error");
        assert_eq!(io.kind(), std::io::ErrorKind::InvalidInput);
        for lookalike in [
            "provider::not valid::oauth::meta",
            "anything::oauth::meta",
            "provider::alpha::oauth::meta",
        ] {
            let err = read_token_by_scope(home, lookalike).unwrap_err();
            let io = err.downcast_ref::<std::io::Error>().unwrap();
            assert_eq!(io.kind(), std::io::ErrorKind::InvalidInput, "{lookalike}");
        }
        store_api_key(home, "xai-secret").unwrap();
        assert_eq!(
            read_token_by_scope(home, API_KEY_SCOPE).unwrap(),
            "xai-secret"
        );
    }

    #[test]
    fn oauth_storage_is_non_destructive_to_sibling_scopes() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        store_api_key(home, "xai-key").unwrap();
        store_provider_api_key(home, OPENAI_API_KEY_SCOPE, "openai-key").unwrap();
        store_provider_oauth_auth(home, &pid("alpha"), None, &oauth_auth("a1")).unwrap();
        let path = home.join("auth.json");
        let before = std::fs::read_to_string(&path).unwrap();
        assert!(validate_provider_oauth_scope("provider::BAD::oauth").is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            before,
            "validation failure must not touch auth.json"
        );
        assert!(read_api_key(home).as_deref() == Some("xai-key"));
        assert_eq!(
            read_provider_api_key(home, OPENAI_API_KEY_SCOPE)
                .unwrap()
                .as_deref(),
            Some("openai-key")
        );
    }

    #[test]
    fn older_reader_compatibility() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = incarnation(12);
        store_provider_oauth_auth(home, &pid("alpha"), Some(&inc), &oauth_auth("a1")).unwrap();
        let store = read_auth_json(&home.join("auth.json")).unwrap();
        let meta = store
            .get(&crate::provider_registry::secrets::oauth_meta_scope_string(
                &pid("alpha"),
            ))
            .unwrap();
        assert_eq!(meta.key, "meta");
        assert!(meta.refresh_token.is_none());
        let path = home.join("auth.json");
        let mut store = read_auth_json(&path).unwrap();
        store.remove(&crate::provider_registry::secrets::oauth_meta_scope_string(
            &pid("alpha"),
        ));
        write_auth_json(&path, &store).unwrap();
        assert_eq!(
            read_provider_oauth_auth(home, &pid("alpha"), None)
                .unwrap()
                .unwrap()
                .key,
            "a1"
        );
    }
}
