//! Process-wide shared `reqwest::Client`s for sampling requests.
//!
//! Sharing one client across all `InferenceClient` instances is safe because
//! the builders below take no config-derived input: auth, extra headers, base
//! URL, and User-Agent are all applied per-request in `InferenceClient::post`.
//! Stale-connection exposure is bounded by HTTP/2 keepalive pings (15s
//! interval, 5s timeout, while idle), the 90s idle-pool eviction, and the
//! first-retry HTTP/1.1 rebuild escape hatch (that client never pools, so
//! every use opens a fresh connection).
//!
//! Wire-level behavior (connection reuse, header isolation, pool-less http1
//! fallback, kill switch) is pinned by the `shared_http_wire` and
//! `shared_http_kill_switch` integration binaries, which own their process
//! environment.

use arc_swap::ArcSwap;
use arc_swap::ArcSwapOption;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

/// Shared sampling clients. `ArcSwapOption` empty until first successful
/// build; reads are wait-free, a failed build stays empty so the next call
/// retries, and a racing loser's freshly built client is dropped.
static SHARED_H2: ArcSwapOption<reqwest::Client> = ArcSwapOption::const_empty();
static SHARED_HTTP1: ArcSwapOption<reqwest::Client> = ArcSwapOption::const_empty();

/// Latch states for the kill switch (resolved once per process).
const LATCH_UNRESOLVED: u8 = 0;
const LATCH_DISABLED: u8 = 1;
const LATCH_ENABLED: u8 = 2;

/// Kill switch: `GROK_SAMPLER_SHARED_CLIENT=0` (or `false`, any case)
/// restores the old behavior of building a fresh `reqwest::Client` per
/// `InferenceClient`. Resolved once per process via a lock-free atomic
/// latch: the environment cannot change externally after spawn, and
/// latching keeps the rollback state consistent with the read-once pool
/// knobs.
fn sharing_disabled() -> bool {
    static DISABLED: AtomicU8 = AtomicU8::new(LATCH_UNRESOLVED);
    match DISABLED.load(Ordering::Acquire) {
        LATCH_DISABLED => true,
        LATCH_ENABLED => false,
        _ => {
            let disabled = match std::env::var("GROK_SAMPLER_SHARED_CLIENT") {
                Ok(v) => v == "0" || v.eq_ignore_ascii_case("false"),
                Err(_) => false,
            };
            if disabled {
                tracing::info!(
                    "sampler HTTP client sharing disabled via GROK_SAMPLER_SHARED_CLIENT"
                );
            }
            DISABLED.store(
                if disabled {
                    LATCH_DISABLED
                } else {
                    LATCH_ENABLED
                },
                Ordering::Release,
            );
            disabled
        }
    }
}

/// Clone the shared client out of `cell`, building it on first use. Build
/// failures are not cached: on `Err` the cell stays empty and the next call
/// retries. Insertion is a lock-free `rcu`: a racing loser re-reads the
/// latest snapshot and adopts the winner's client instead of overwriting.
fn shared(
    cell: &ArcSwapOption<reqwest::Client>,
    build: fn() -> Result<reqwest::Client, reqwest::Error>,
    disabled: bool,
) -> Result<reqwest::Client, reqwest::Error> {
    if disabled {
        return build();
    }
    if let Some(client) = cell.load().as_ref() {
        return Ok((**client).clone());
    }
    let built = build()?;
    let mut adopted: Option<reqwest::Client> = None;
    cell.rcu(|current| {
        if let Some(existing) = current.as_ref() {
            adopted = Some((**existing).clone());
            return current.clone();
        }
        Some(Arc::new(built.clone()))
    });
    Ok(adopted.unwrap_or(built))
}

/// Shared HTTP/2 sampling client (connection pooling + h2 keepalive).
pub(crate) fn client() -> Result<reqwest::Client, reqwest::Error> {
    shared(&SHARED_H2, build_http_client, sharing_disabled())
}

/// Shared HTTP/1.1 fallback client. Pool-less by construction, so sharing it
/// is behaviorally identical to building a fresh one.
pub(crate) fn client_http1() -> Result<reqwest::Client, reqwest::Error> {
    shared(&SHARED_HTTP1, build_http_client_http1, sharing_disabled())
}

/// Identity of one persistent per-provider HTTP pool.
///
/// The sampling clients above are shared across every provider because
/// sampling policy is uniform. Platform (embeddings/rerank), retrieval, and
/// catalog traffic instead gets one pool per `(pool family, provider)`
/// identity, so each provider's connections persist independently and can
/// be tuned separately.
///
/// The key carries every client-level policy scalar that binds at
/// connection-open time (`connect_timeout`, HTTP/1.1-only). Everything else
/// that used to be client-level (request timeout, User-Agent) is applied
/// per request by the callers, so it must NOT be part of the key.
///
/// The first caller for a key builds the client; every later caller clones
/// the same `reqwest::Client` (cheap, `Arc`-backed), so all requests for
/// that provider share one warm pool for the whole process lifetime. This
/// mirrors the `HubConnectionPool` "first opener wins" precedent: callers
/// that need different connection policy must use a distinct key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ProviderPoolKey {
    /// Pool family, e.g. `platform`, `retrieval`, `anthropic-catalog`.
    pool: &'static str,
    /// Provider identity within the family (`provider_id`, route instance,
    /// or normalized base URL).
    provider: String,
    /// Provider-registry id used for per-provider tuning lookup. `None`
    /// means the pool family's env knobs / defaults apply.
    config_key: Option<String>,
    /// Client-level connect timeout, in seconds. Distinct timeouts get
    /// distinct pools because `connect_timeout` binds at open time.
    connect_timeout_secs: u64,
    /// HTTP/1.1-only pools skip HTTP/2 keepalive configuration.
    http1_only: bool,
}

impl ProviderPoolKey {
    pub(crate) fn new(
        pool: &'static str,
        provider: impl Into<String>,
        connect_timeout: Duration,
        http1_only: bool,
    ) -> Self {
        Self {
            pool,
            provider: provider.into(),
            config_key: None,
            connect_timeout_secs: connect_timeout.as_secs(),
            http1_only,
        }
    }

    /// Pool key whose tuning resolves against a provider-registry id
    /// (e.g. the `anthropic-catalog` pool tunes via the built-in
    /// `anthropic` entry).
    pub(crate) fn new_with_config_key(
        pool: &'static str,
        provider: impl Into<String>,
        config_key: &str,
        connect_timeout: Duration,
        http1_only: bool,
    ) -> Self {
        Self {
            config_key: Some(config_key.to_owned()),
            ..Self::new(pool, provider, connect_timeout, http1_only)
        }
    }

    /// `GROK_POOL_<POOL>_...` environment-variable family for this pool.
    fn env_family(&self) -> String {
        env_family(self.pool)
    }

    /// Id used for per-provider tuning lookup: the explicit config key when
    /// set, else the pool identity itself.
    fn tuning_id(&self) -> &str {
        self.config_key.as_deref().unwrap_or(&self.provider)
    }
}

/// Tuning override for one provider's pools (from `[model_providers.<id>]`).
///
/// Each field is optional: a configured field wins, an absent field falls
/// back to the pool's env knobs / built-in defaults. `max_idle` and
/// `idle_timeout_secs` size the reqwest connection pool; `connect_timeout_secs`
/// overrides the connect timeout used when opening that provider's pool
/// clients (it becomes part of the pool key).
///
/// `http1_only` pins a provider's pools to HTTP/1.1 (no HTTP/2 keepalive
/// window). It is part of the pool key, so a provider that pins HTTP/1.1
/// gets a distinct pool from one whose h2 transport is healthy. `None`
/// (the default) lets the pool family / env decide.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderPoolTuning {
    pub max_idle: Option<u32>,
    pub idle_timeout_secs: Option<u64>,
    pub connect_timeout_secs: Option<u64>,
    /// Pin this provider's pools to HTTP/1.1 (`None` = family default).
    pub http1_only: Option<bool>,
}

impl ProviderPoolTuning {
    fn clamped_max_idle(&self) -> Option<usize> {
        self.max_idle
            .map(|v| usize::try_from(v).unwrap_or(0).min(64))
    }

    fn clamped_idle_timeout_secs(&self) -> Option<u64> {
        self.idle_timeout_secs.map(|v| v.clamp(1, 3600))
    }

    fn clamped_connect_timeout_secs(&self) -> Option<u64> {
        self.connect_timeout_secs.map(|v| v.clamp(1, 120))
    }
}

static PROVIDER_POOL_TUNING: OnceLock<ArcSwap<HashMap<String, ProviderPoolTuning>>> =
    OnceLock::new();

fn provider_pool_tuning() -> &'static ArcSwap<HashMap<String, ProviderPoolTuning>> {
    PROVIDER_POOL_TUNING.get_or_init(|| ArcSwap::from_pointee(HashMap::new()))
}

/// Install per-provider pool overrides (provider-registry id → knobs).
///
/// Called once per process after the provider registry loads; the swap is
/// lock-free (`ArcSwap::store`). Later calls replace the complete map.
pub fn configure_provider_pool_tuning(overrides: HashMap<String, ProviderPoolTuning>) {
    provider_pool_tuning().store(Arc::new(overrides));
}

/// Effective connect timeout for one provider's pools: the configured
/// override wins over the caller's policy value, so the pool key embeds
/// the timeout the client will actually be built with.
pub fn effective_provider_connect_timeout(
    provider_hint: &str,
    policy_connect_timeout: Duration,
) -> Duration {
    if let Some(tuning) = provider_pool_tuning().load().get(provider_hint)
        && let Some(secs) = tuning.clamped_connect_timeout_secs()
    {
        return Duration::from_secs(secs);
    }
    policy_connect_timeout
}

/// Whether `provider_hint` has a configured per-provider pool override.
///
/// A provider with registered tuning routes sampling through its own
/// `ProviderPoolKey` (custom pool sizing / connect timeout / HTTP/1.1-only)
/// rather than the process-wide shared sampling client.
pub(crate) fn provider_pool_has_tuning(provider_hint: &str) -> bool {
    provider_pool_tuning().load().contains_key(provider_hint)
}

/// Uppercased pool-family name used in `GROK_POOL_<FAMILY>_*` env knobs.
fn env_family(pool: &str) -> String {
    pool.to_uppercase()
}

static PROVIDER_CLIENTS: OnceLock<ArcSwap<HashMap<ProviderPoolKey, reqwest::Client>>> =
    OnceLock::new();

fn provider_clients() -> &'static ArcSwap<HashMap<ProviderPoolKey, reqwest::Client>> {
    PROVIDER_CLIENTS.get_or_init(|| ArcSwap::from_pointee(HashMap::new()))
}

/// Registry defaults. Platform/retrieval calls are short and bursty
/// (embedding batches, rerank turns), so keep more idle slots than the
/// sampling client's default of 8.
const PROVIDER_POOL_DEFAULT_MAX_IDLE: usize = 8;
const PROVIDER_POOL_DEFAULT_IDLE_TIMEOUT_SECS: u64 = 90;

/// Per-pool knob resolution, most specific wins per field: configured
/// per-provider override (via [`configure_provider_pool_tuning`]), then the
/// `GROK_POOL_<POOL>_<NAME>` env family, then the global `GROK_POOL_<NAME>`
/// knob, then the built-in default.
fn resolve_pool_knobs(key: &ProviderPoolKey) -> (usize, u64) {
    let tuning = provider_pool_tuning().load().get(key.tuning_id()).copied();
    let max_idle = tuning
        .and_then(|t| t.clamped_max_idle())
        .or_else(|| {
            env_knob_u64(&format!("GROK_POOL_{}_MAX_IDLE", key.env_family()))
                .map(|v| v.min(64) as usize)
        })
        .or_else(|| env_knob_u64("GROK_POOL_MAX_IDLE").map(|v| v.min(64) as usize))
        .unwrap_or(PROVIDER_POOL_DEFAULT_MAX_IDLE);
    let idle_timeout_secs = tuning
        .and_then(|t| t.clamped_idle_timeout_secs())
        .or_else(|| env_knob_u64(&format!("GROK_POOL_{}_IDLE_TIMEOUT_SECS", key.env_family())))
        .or_else(|| env_knob_u64("GROK_POOL_IDLE_TIMEOUT_SECS"))
        .unwrap_or(PROVIDER_POOL_DEFAULT_IDLE_TIMEOUT_SECS);
    (max_idle, idle_timeout_secs)
}

/// Read and parse one `GROK_POOL_*`-style environment knob.
fn env_knob_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

/// Names of the per-pool environment knobs, for tests and diagnostics.
#[cfg(test)]
fn pool_env_names(pool: &str) -> (String, String) {
    let family = env_family(pool);
    (
        format!("GROK_POOL_{family}_MAX_IDLE"),
        format!("GROK_POOL_{family}_IDLE_TIMEOUT_SECS"),
    )
}

/// Clone the persistent per-provider pool client for `key`, building it on
/// first use. Reads are wait-free (`ArcSwap::load` bumps an `Arc` refcount;
/// no lock is ever held on the read path). On a miss the client is built
/// without any lock and inserted via `rcu`, whose closure re-reads the
/// latest snapshot on contention — concurrent first-builds never lose
/// updates, and a race loser adopts the winner's client. Build failures are
/// never cached. The only lock in this path is the container's one-shot
/// `OnceLock` initialization.
pub(crate) fn provider_client(
    key: ProviderPoolKey,
    configure: impl FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
) -> Result<reqwest::Client, reqwest::Error> {
    let map = provider_clients();
    if let Some(client) = map.load().get(&key) {
        return Ok(client.clone());
    }
    let built = build_provider_client(&key, configure)?;
    let mut adopted: Option<reqwest::Client> = None;
    map.rcu(|current| {
        if let Some(existing) = current.get(&key) {
            adopted = Some(existing.clone());
            return Arc::clone(current);
        }
        let mut next = (**current).clone();
        next.insert(key.clone(), built.clone());
        Arc::new(next)
    });
    Ok(adopted.unwrap_or(built))
}

/// Names of all live provider pools. Diagnostics and tests only.
#[cfg(test)]
pub(crate) fn provider_pool_names() -> Vec<String> {
    provider_clients()
        .load()
        .keys()
        .map(|key| format!("{}/{}", key.pool, key.provider))
        .collect()
}

/// Names of live `sampling`-family provider pools, as `"sampling/<provider>"`.
///
/// Diagnostics and tests only: observes which providers routed sampling
/// through their own per-provider pool (vs. the process-wide shared client).
pub fn sampling_pool_names() -> Vec<String> {
    provider_clients()
        .load()
        .iter()
        .filter(|(key, _)| key.pool == "sampling")
        .map(|(key, _)| format!("sampling/{}", key.provider))
        .collect()
}

/// Truthy environment knob: `1`, `true`, or `yes` (any case).
fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

/// `GROK_HTTP2_ADAPTIVE_WINDOW` truthy (`1` / `true` / `yes`) enables the
/// reqwest HTTP/2 adaptive-window flow-control knob.
fn http2_adaptive_window_enabled() -> bool {
    env_truthy("GROK_HTTP2_ADAPTIVE_WINDOW")
}

/// `GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE` parsed as a positive `u32`.
/// The HTTP/2 spec caps this at 2^31-1; a zero or unparseable value leaves
/// the reqwest default untouched.
fn http2_initial_stream_window() -> Option<u32> {
    std::env::var("GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|&n| n > 0)
}

/// Sampling client pool max idle: `GROK_POOL_MAX_IDLE` override, default 8.
///
/// Parallel background subagents stream concurrently (admission allows 32),
/// so the sampler keeps 8 idle slots per host instead of the old
/// conservative 2: two sampling streams per slot keep every retry and
/// concurrent request on a warm connection instead of paying a fresh
/// TCP+TLS handshake.
fn sampling_pool_max_idle() -> usize {
    parse_sampling_pool_max_idle(std::env::var("GROK_POOL_MAX_IDLE").ok().as_deref())
}

/// Sampling connect timeout, matching the shared client's policy default:
/// `GROK_CONNECT_TIMEOUT_SECS` override, else 10 s.
pub(crate) fn sampling_connect_timeout() -> Duration {
    Duration::from_secs(
        std::env::var("GROK_CONNECT_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),
    )
}

/// Pure parse of the sampling pool max-idle knob, for tests and diagnostics.
fn parse_sampling_pool_max_idle(raw: Option<&str>) -> usize {
    raw.and_then(|v| v.parse().ok()).unwrap_or(8)
}

/// Single-flight SSE stream reconnect gate: `GROK_HTTP_STREAM_RECONNECT`
/// truthy (`1` / `true` / `yes`). Off by default.
pub(crate) fn stream_reconnect_enabled() -> bool {
    env_truthy("GROK_HTTP_STREAM_RECONNECT")
}

/// Bounded per-stream preamble capture for mid-stream error classification.
/// Large enough for a provider error envelope; small enough to never be a
/// memory concern per in-flight stream.
pub(crate) const STREAM_PREAMBLE_CAPTURE_LIMIT: usize = 16 * 1024;

/// Fields logged by the post-build `http_client_built` event.
struct HttpClientBuiltFields {
    pool_max_idle: usize,
    idle_timeout_secs: u64,
    connect_timeout_secs: u64,
    http2_adaptive_window: bool,
    initial_stream_window: Option<u32>,
}

/// Emit the post-build observability event (see Phase D3). The
/// `initial_stream_window` field is present only when the knob applied.
fn emit_http_client_built(fields: HttpClientBuiltFields) {
    match fields.initial_stream_window {
        Some(window) => tracing::info!(
            target: crate::inference_log::TARGET,
            event = "http_client_built",
            pool_max_idle = fields.pool_max_idle,
            idle_timeout_secs = fields.idle_timeout_secs,
            connect_timeout_secs = fields.connect_timeout_secs,
            http2_adaptive_window = fields.http2_adaptive_window,
            initial_stream_window = window,
            "HTTP client built"
        ),
        None => tracing::info!(
            target: crate::inference_log::TARGET,
            event = "http_client_built",
            pool_max_idle = fields.pool_max_idle,
            idle_timeout_secs = fields.idle_timeout_secs,
            connect_timeout_secs = fields.connect_timeout_secs,
            http2_adaptive_window = fields.http2_adaptive_window,
            "HTTP client built"
        ),
    }
}

fn build_provider_client(
    key: &ProviderPoolKey,
    configure: impl FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
) -> Result<reqwest::Client, reqwest::Error> {
    let (pool_max_idle, idle_timeout_secs) = resolve_pool_knobs(key);
    let http2_adaptive_window = http2_adaptive_window_enabled();
    let initial_stream_window = http2_initial_stream_window();
    let builder = crate::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .pool_max_idle_per_host(pool_max_idle)
        .pool_idle_timeout(Duration::from_secs(idle_timeout_secs))
        .connect_timeout(Duration::from_secs(key.connect_timeout_secs))
        .tcp_nodelay(true);
    let builder = if key.http1_only {
        builder.http1_only()
    } else {
        // Keep sockets warm between bursts: ping while idle so a pooled
        // connection survives long gaps between retrieval turns.
        let mut builder = builder
            .http2_keep_alive_interval(Duration::from_secs(15))
            .http2_keep_alive_timeout(Duration::from_secs(5))
            .http2_keep_alive_while_idle(true);
        if http2_adaptive_window {
            builder = builder.http2_adaptive_window(true);
            tracing::debug!(
                target: crate::inference_log::TARGET,
                event = "http2_multiplex_knob",
                knob = "adaptive_window",
                "provider pool client: HTTP/2 adaptive window enabled via GROK_HTTP2_ADAPTIVE_WINDOW"
            );
        }
        if let Some(window) = initial_stream_window {
            builder = builder.http2_initial_stream_window_size(window);
            tracing::debug!(
                target: crate::inference_log::TARGET,
                event = "http2_multiplex_knob",
                knob = "initial_stream_window",
                window,
                "provider pool client: HTTP/2 initial stream window set via GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE"
            );
        }
        builder
    };
    tracing::info!(
        target: crate::inference_log::TARGET,
        event = "provider_pool_build",
        pool = %key.pool,
        provider = %key.provider,
        pool_max_idle,
        idle_timeout_secs,
        http1_only = key.http1_only,
        "building persistent per-provider HTTP pool"
    );
    let built = configure(builder).build();
    if let Ok(_) = &built {
        emit_http_client_built(HttpClientBuiltFields {
            pool_max_idle,
            idle_timeout_secs,
            connect_timeout_secs: key.connect_timeout_secs,
            // HTTP/1.1-only pools never apply the HTTP/2 knobs; report what
            // was actually configured, not what the env asked for.
            http2_adaptive_window: !key.http1_only && http2_adaptive_window,
            initial_stream_window: if key.http1_only {
                None
            } else {
                initial_stream_window
            },
        });
    }
    built
}

/// Build a `reqwest::Client` for sampling with HTTP/2 + connection pooling.
/// Env knobs are read once, when the shared client is first built.
fn build_http_client() -> Result<reqwest::Client, reqwest::Error> {
    let pool_max_idle: usize = sampling_pool_max_idle();
    let pool_idle_timeout_secs: u64 = std::env::var("GROK_POOL_IDLE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);
    let connect_timeout_secs: u64 = std::env::var("GROK_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let http2_adaptive_window = http2_adaptive_window_enabled();
    let initial_stream_window = http2_initial_stream_window();

    let mut builder = crate::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .pool_max_idle_per_host(pool_max_idle)
        .pool_idle_timeout(Duration::from_secs(pool_idle_timeout_secs))
        .connect_timeout(Duration::from_secs(connect_timeout_secs))
        .tcp_nodelay(true)
        // HTTP/2 keep-alive: ping every 15s, timeout after 5s.
        .http2_keep_alive_interval(Duration::from_secs(15))
        .http2_keep_alive_timeout(Duration::from_secs(5))
        .http2_keep_alive_while_idle(true);
    if http2_adaptive_window {
        builder = builder.http2_adaptive_window(true);
        tracing::debug!(
            target: crate::inference_log::TARGET,
            event = "http2_multiplex_knob",
            knob = "adaptive_window",
            "sampling client: HTTP/2 adaptive window enabled via GROK_HTTP2_ADAPTIVE_WINDOW"
        );
    }
    if let Some(window) = initial_stream_window {
        builder = builder.http2_initial_stream_window_size(window);
        tracing::debug!(
            target: crate::inference_log::TARGET,
            event = "http2_multiplex_knob",
            knob = "initial_stream_window",
            window,
            "sampling client: HTTP/2 initial stream window set via GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE"
        );
    }

    let built = builder.build();
    if let Ok(_) = &built {
        emit_http_client_built(HttpClientBuiltFields {
            pool_max_idle,
            idle_timeout_secs: pool_idle_timeout_secs,
            connect_timeout_secs,
            http2_adaptive_window,
            initial_stream_window,
        });
    }
    built
}

/// Build a `reqwest::Client` constrained to HTTP/1.1 with pooling disabled.
/// Used as a fallback after HTTP/2 transport failures.
fn build_http_client_http1() -> Result<reqwest::Client, reqwest::Error> {
    let connect_timeout_secs: u64 = std::env::var("GROK_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    crate::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .pool_max_idle_per_host(0)
        .pool_idle_timeout(Duration::from_secs(0))
        .connect_timeout(Duration::from_secs(connect_timeout_secs))
        .tcp_nodelay(true)
        .http1_only()
        .build()
}

#[cfg(test)]
mod tests {
    use arc_swap::ArcSwapOption;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::shared;

    static BUILD_CALLS: AtomicUsize = AtomicUsize::new(0);

    /// Fails on the first call (a real `reqwest::Error`, no I/O), then builds.
    fn flaky_build() -> Result<reqwest::Client, reqwest::Error> {
        if BUILD_CALLS.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(reqwest::Proxy::all("not a proxy url").unwrap_err());
        }
        reqwest::Client::builder().build()
    }

    #[test]
    fn shared_does_not_cache_build_failures() {
        static CELL: ArcSwapOption<reqwest::Client> = ArcSwapOption::const_empty();
        assert!(shared(&CELL, flaky_build, false).is_err());
        assert!(CELL.load().is_none(), "failure must leave the cell empty");
        assert!(shared(&CELL, flaky_build, false).is_ok());
        assert!(CELL.load().is_some(), "success must populate the cell");
        assert!(shared(&CELL, flaky_build, false).is_ok());
        assert_eq!(
            BUILD_CALLS.load(Ordering::SeqCst),
            2,
            "third call must reuse the cached client, not rebuild"
        );
    }

    #[test]
    fn shared_disabled_bypasses_cell() {
        static CELL: ArcSwapOption<reqwest::Client> = ArcSwapOption::const_empty();
        assert!(shared(&CELL, || reqwest::Client::builder().build(), true).is_ok());
        assert!(
            CELL.load().is_none(),
            "disabled mode must never touch the cell"
        );
    }

    fn unique_provider(prefix: &str) -> String {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        format!("{prefix}-{}", SEQ.fetch_add(1, Ordering::SeqCst))
    }

    #[test]
    fn provider_client_builds_once_per_key() {
        let provider = unique_provider("registry");
        let builds = AtomicUsize::new(0);
        let configure = |builder: reqwest::ClientBuilder| {
            builds.fetch_add(1, Ordering::SeqCst);
            builder
        };
        let key = |http1_only| {
            super::ProviderPoolKey::new(
                "platform",
                provider.clone(),
                Duration::from_secs(10),
                http1_only,
            )
        };
        let _first = super::provider_client(key(false), configure).unwrap();
        let _second = super::provider_client(key(false), configure).expect("hit path");
        assert_eq!(
            builds.load(Ordering::SeqCst),
            1,
            "second call with the same key must reuse the built client"
        );
        // Distinct connection policy ⇒ distinct pool ⇒ distinct build.
        let _http1 = super::provider_client(key(true), configure).unwrap();
        assert_eq!(
            builds.load(Ordering::SeqCst),
            2,
            "a different key must build its own pool"
        );
        assert!(
            super::provider_pool_names()
                .iter()
                .any(|name| *name == format!("platform/{provider}")),
            "pool must be registered under pool/provider identity"
        );
    }

    #[test]
    fn provider_pool_env_names_are_per_pool() {
        let (max_idle, idle_timeout) = super::pool_env_names("platform");
        assert_eq!(max_idle, "GROK_POOL_PLATFORM_MAX_IDLE");
        assert_eq!(idle_timeout, "GROK_POOL_PLATFORM_IDLE_TIMEOUT_SECS");
        let (max_idle, idle_timeout) = super::pool_env_names("retrieval");
        assert_eq!(max_idle, "GROK_POOL_RETRIEVAL_MAX_IDLE");
        assert_eq!(idle_timeout, "GROK_POOL_RETRIEVAL_IDLE_TIMEOUT_SECS");
    }

    /// The registry is lock-free on the read path: N threads racing the
    /// same key must all observe a client and the registry must end with
    /// exactly one entry for that key (no lost updates, no duplicates).
    #[test]
    fn provider_client_concurrent_first_build_never_loses_updates() {
        let provider = unique_provider("concurrent");
        let key = || {
            super::ProviderPoolKey::new(
                "platform",
                provider.clone(),
                Duration::from_secs(10),
                false,
            )
        };
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let key = key();
                std::thread::spawn(move || super::provider_client(key, |b| b))
            })
            .collect();
        let built: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().unwrap().unwrap())
            .collect();
        assert_eq!(built.len(), 8, "every racer must observe a client");
        let pool_count = super::provider_pool_names()
            .iter()
            .filter(|name| name.contains(&provider))
            .count();
        assert_eq!(
            pool_count, 1,
            "concurrent first-builds must not lose updates or duplicate entries"
        );
    }

    /// Serialize env-touching tests in this module: the mutex is process-local
    /// to the lib test binary, and only these tests mutate the knobs below,
    /// so `unsafe set_var`/`remove_var` is sound while the lock is held.
    static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// RAII guard restoring a single env var on drop (Phase D knob tests).
    struct EnvVarGuard {
        key: &'static str,
        prior: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let prior = std::env::var_os(key);
            // SAFETY: held under ENV_TEST_LOCK; no other test in this binary
            // mutates these knobs concurrently.
            unsafe { std::env::set_var(key, value) };
            Self { key, prior }
        }

        fn unset(key: &'static str) -> Self {
            let prior = std::env::var_os(key);
            // SAFETY: see [`EnvVarGuard::set`].
            unsafe { std::env::remove_var(key) };
            Self { key, prior }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: see [`EnvVarGuard::set`].
            match self.prior.take() {
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn sampling_pool_max_idle_defaults_to_eight() {
        assert_eq!(super::parse_sampling_pool_max_idle(None), 8);
        assert_eq!(super::parse_sampling_pool_max_idle(Some("0")), 0);
        assert_eq!(super::parse_sampling_pool_max_idle(Some("12")), 12);
        assert_eq!(super::parse_sampling_pool_max_idle(Some("not-a-number")), 8);
    }

    #[test]
    fn http2_multiplex_knobs_parse_from_env() {
        let _lock = ENV_TEST_LOCK.lock().unwrap();

        let adaptive_on = EnvVarGuard::set("GROK_HTTP2_ADAPTIVE_WINDOW", "1");
        assert!(super::http2_adaptive_window_enabled());
        drop(adaptive_on);

        let adaptive_true = EnvVarGuard::set("GROK_HTTP2_ADAPTIVE_WINDOW", "true");
        assert!(super::http2_adaptive_window_enabled());
        drop(adaptive_true);

        let adaptive_yes = EnvVarGuard::set("GROK_HTTP2_ADAPTIVE_WINDOW", "yes");
        assert!(super::http2_adaptive_window_enabled());
        drop(adaptive_yes);

        let adaptive_off = EnvVarGuard::set("GROK_HTTP2_ADAPTIVE_WINDOW", "0");
        assert!(!super::http2_adaptive_window_enabled());
        drop(adaptive_off);

        let adaptive_absent = EnvVarGuard::unset("GROK_HTTP2_ADAPTIVE_WINDOW");
        assert!(!super::http2_adaptive_window_enabled());
        drop(adaptive_absent);

        let window = EnvVarGuard::set("GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE", "1048576");
        assert_eq!(super::http2_initial_stream_window(), Some(1_048_576));
        drop(window);

        let window_zero = EnvVarGuard::set("GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE", "0");
        assert_eq!(super::http2_initial_stream_window(), None);
        drop(window_zero);

        let window_bad = EnvVarGuard::set("GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE", "abc");
        assert_eq!(super::http2_initial_stream_window(), None);
        drop(window_bad);

        let window_absent = EnvVarGuard::unset("GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE");
        assert_eq!(super::http2_initial_stream_window(), None);
    }

    #[test]
    fn build_http_client_honors_http2_multiplex_knobs() {
        let _lock = ENV_TEST_LOCK.lock().unwrap();
        let _adaptive = EnvVarGuard::set("GROK_HTTP2_ADAPTIVE_WINDOW", "1");
        let _window = EnvVarGuard::set("GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE", "1048576");
        let client = super::build_http_client()
            .expect("knob-bearing client must build offline (no network)");
        drop(client);
    }

    #[test]
    fn stream_reconnect_is_off_by_default() {
        let _lock = ENV_TEST_LOCK.lock().unwrap();
        let _absent = EnvVarGuard::unset("GROK_HTTP_STREAM_RECONNECT");
        assert!(!super::stream_reconnect_enabled());
        drop(_absent);

        let _on = EnvVarGuard::set("GROK_HTTP_STREAM_RECONNECT", "1");
        assert!(super::stream_reconnect_enabled());
    }
}
