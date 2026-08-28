//! OpenRouter-native catalog adapter with offset/limit and links.next pagination.
//!
//! Documented shape (`GET /api/v1/models`):
//! ```json
//! {
//!   "data": [ { "id", "name", … } ],
//!   "links": { "next": "https://openrouter.ai/api/v1/models?offset=…" },
//!   "total_count": 123
//! }
//! ```
//! `data` must be a JSON array. `links.next` is restricted to the same models
//! list path and PR7-normalized origin.

use super::bounds::{CatalogBoundError, CatalogFetchBounds, CatalogFetchBudget};
use super::http_body::{effective_request_timeout, read_body_bounded, send_cancellable};
use super::origin::{
    enforce_same_origin, list_path_of, validate_list_url_origin, validate_models_next_url,
};
use super::project::{
    conservative_openrouter_context_window, conservative_openrouter_max_output_ceiling,
    dedupe_and_sort_models, openrouter_discovered_model,
};
use super::types::{
    CatalogAccountIdentity, CatalogAdapterError, CatalogFetchSource, CatalogTruncationReason,
    DiscoveredModel, InstanceCatalogResult,
};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize)]
struct OpenRouterPage {
    #[serde(default)]
    data: Vec<OpenRouterModelRow>,
    #[serde(default)]
    links: Option<OpenRouterLinks>,
    #[serde(default)]
    total_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterLinks {
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelRow {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    top_provider: Option<OpenRouterTopProvider>,
    #[serde(default)]
    architecture: Option<OpenRouterArchitecture>,
    #[serde(default)]
    supported_parameters: Vec<String>,
    #[serde(default)]
    default_parameters: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    reasoning_effort_options: Option<Vec<String>>,
    #[serde(default)]
    reasoning: Option<OpenRouterReasoningMetadata>,
    #[serde(default)]
    per_request_limits: Option<OpenRouterPerRequestLimits>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterTopProvider {
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    max_completion_tokens: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    is_moderated: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
    #[serde(default)]
    output_modalities: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterPerRequestLimits {
    #[serde(default)]
    #[allow(dead_code)]
    prompt_tokens: Option<Value>,
    #[serde(default)]
    completion_tokens: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterReasoningMetadata {
    #[serde(default)]
    supported_efforts: OpenRouterSupportedEfforts,
    #[serde(default)]
    default_effort: Option<String>,
    #[serde(default)]
    mandatory: bool,
    #[serde(default)]
    #[allow(dead_code)]
    supports_max_tokens: Option<bool>,
}

#[derive(Debug, Default)]
enum OpenRouterSupportedEfforts {
    #[default]
    Omitted,
    Unrestricted,
    Exact(Vec<String>),
}

impl<'de> Deserialize<'de> for OpenRouterSupportedEfforts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<Vec<String>>::deserialize(deserializer)
            .map(|value| value.map_or(Self::Unrestricted, Self::Exact))
    }
}

/// Bounded multi-page OpenRouter list fetch that returns one assembled
/// `{"data":[...]}` body with **verbatim** model objects from each page.
///
/// Used by production `ProviderManager` so the authoritative
/// `parse_openrouter_catalog` projection (effort selection, tools, modalities)
/// remains the single projector. Pagination / size / cancel bounds still apply.
pub async fn fetch_openrouter_bounded_list_body(
    models_url: &str,
    bearer_token: &str,
    expected_origin: &str,
    bounds: CatalogFetchBounds,
    cancel: &CancellationToken,
) -> Result<Vec<u8>, CatalogAdapterError> {
    if bearer_token.trim().is_empty() {
        return Err(CatalogAdapterError::MissingCredential);
    }
    let origin = validate_list_url_origin(models_url, expected_origin)?;
    let list_path = list_path_of(models_url)?;
    let client = reqwest::Client::builder()
        .timeout(bounds.request_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| CatalogAdapterError::Transport {
            detail: CatalogAdapterError::sanitize_detail(&e.to_string()),
        })?;

    let mut budget = CatalogFetchBudget::new(bounds);
    let mut raw_models: Vec<Value> = Vec::new();
    let mut next_url: Option<String> = None;
    let mut offset: u32 = 0;
    let page_size = bounds.page_size;
    let mut pagination_active = false;
    budget.remember_cursor("offset:0")?;

    loop {
        if cancel.is_cancelled() {
            return Err(CatalogAdapterError::Cancelled);
        }
        budget.check_deadline()?;

        let page_url = if let Some(ref absolute) = next_url {
            enforce_same_origin(absolute, &origin)?;
            absolute.clone()
        } else {
            with_offset_limit(
                models_url,
                offset,
                page_size,
                pagination_active || offset > 0,
            )?
        };
        enforce_same_origin(&page_url, &origin)?;

        let timeout = effective_request_timeout(&budget);
        let request = client
            .get(&page_url)
            .timeout(timeout)
            .bearer_auth(bearer_token.trim());
        let response = send_cancellable(request, cancel, &budget).await?;
        let status = response.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(CatalogAdapterError::AuthFailure {
                status: status.as_u16(),
            });
        }
        if !status.is_success() {
            return Err(CatalogAdapterError::Transport {
                detail: format!("HTTP {}", status.as_u16()),
            });
        }

        let bytes = read_body_bounded(response, &mut budget, cancel).await?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| CatalogAdapterError::Malformed {
                detail: "invalid JSON".into(),
            })?;
        let data = value
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| CatalogAdapterError::Malformed {
                detail: "data is not an array".into(),
            })?;
        let page_count = data.len();
        budget.record_models(page_count)?;
        raw_models.extend(data.iter().cloned());

        // Prefer absolute links.next when present (path-restricted).
        if let Some(next_raw) = value
            .get("links")
            .and_then(|l| l.get("next"))
            .and_then(|n| n.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let (safe_next, cursor_key) = validate_models_next_url(next_raw, &origin, &list_path)?;
            if budget.pages_fetched() >= budget.bounds().max_pages {
                return Err(CatalogBoundError::PageCountExceeded {
                    max: budget.bounds().max_pages,
                }
                .into());
            }
            budget.remember_cursor(&cursor_key)?;
            next_url = Some(safe_next);
            pagination_active = true;
            continue;
        }

        next_url = None;
        if page_count == 0 {
            break;
        }
        let total_count = value.get("total_count").and_then(|v| v.as_i64());
        let more_by_total = total_count
            .is_some_and(|total| total > 0 && (offset as i64) + (page_count as i64) < total);
        if more_by_total {
            if budget.pages_fetched() >= budget.bounds().max_pages {
                return Err(CatalogBoundError::PageCountExceeded {
                    max: budget.bounds().max_pages,
                }
                .into());
            }
            offset = offset.saturating_add(page_count as u32);
            budget.remember_cursor(&format!("offset:{offset}"))?;
            pagination_active = true;
            continue;
        }
        break;
    }

    serde_json::to_vec(&serde_json::json!({ "data": raw_models })).map_err(|_| {
        CatalogAdapterError::Malformed {
            detail: "failed to reassemble openrouter catalog body".into(),
        }
    })
}

/// Live fetch for one OpenRouter account with provider-specific pagination.
///
/// When `zdr` is `Some(true)`, this instance fetches `GET /models?zdr=true`
/// (that instance only) and stamps `supports_zdr` on every retained row.
/// When `zdr` is false/unset the full list is fetched. In both cases a
/// best-effort `GET /endpoints/zdr` hop tags (or optionally intersects)
/// ZDR-capable slugs. Failure of the optional endpoints hop never fails
/// the models catalog.
pub async fn fetch_openrouter_catalog(
    models_url: &str,
    bearer_token: &str,
    identity: &CatalogAccountIdentity,
    manual_capabilities: &IndexMap<String, bool>,
    zdr: Option<bool>,
    bounds: CatalogFetchBounds,
    registry_generation: u64,
    publication_generation: u64,
    cancel: &CancellationToken,
) -> Result<InstanceCatalogResult, CatalogAdapterError> {
    let zdr_filtered = zdr == Some(true);
    let list_url = if zdr_filtered {
        with_zdr_query(models_url)?
    } else {
        models_url.to_owned()
    };
    // Reuse the bounded body fetch so pagination/bounds stay single-sourced,
    // then project through the adapter's typed path for multi-account catalog.
    let body = fetch_openrouter_bounded_list_body(
        &list_url,
        bearer_token,
        &identity.endpoint_origin,
        bounds,
        cancel,
    )
    .await?;
    let page = parse_openrouter_page(&body)?;
    let zdr_slugs =
        fetch_zdr_endpoint_slugs(&list_url, bearer_token, identity, bounds, cancel).await;
    let models = dedupe_and_sort_models(project_page(
        &page,
        identity,
        manual_capabilities,
        &zdr_slugs,
        zdr_filtered,
    ));
    Ok(InstanceCatalogResult {
        provider_instance_id: identity.instance_id.as_str().to_owned(),
        provider_kind: identity.kind,
        api_surface: identity.api_surface,
        credential_route: identity.credential_route,
        endpoint_origin: identity.endpoint_origin.clone(),
        org_project_fingerprint: identity.org_project_fingerprint.clone(),
        incarnation: Some(identity.incarnation.clone()),
        credential_binding_id: Some(identity.credential_binding_id.clone()),
        registry_generation,
        catalog_generation: 0,
        publication_generation,
        source: CatalogFetchSource::Live,
        truncation: CatalogTruncationReason::Complete,
        models,
        diagnostic: None,
    })
}

fn project_page(
    page: &OpenRouterPage,
    identity: &CatalogAccountIdentity,
    manual: &IndexMap<String, bool>,
    zdr_slugs: &HashSet<String>,
    zdr_filtered: bool,
) -> Vec<DiscoveredModel> {
    page.data
        .iter()
        .filter_map(|row| {
            let upstream = row.id.trim();
            if zdr_filtered && !zdr_slugs.is_empty() && !zdr_slugs.contains(upstream) {
                // Optional intersection with GET /endpoints/zdr when both
                // lists are available. An empty/failed endpoints hop keeps
                // the ?zdr=true models list intact.
                return None;
            }
            let (efforts, default_effort, supports_reasoning) = effort_metadata(row);
            let architecture = row.architecture.as_ref();
            let input_modalities = architecture
                .map(|a| a.input_modalities.as_slice())
                .unwrap_or(&[]);
            let output_modalities = architecture
                .map(|a| a.output_modalities.as_slice())
                .unwrap_or(&[]);
            let routed_window = row.top_provider.as_ref().and_then(|t| t.context_length);
            let top_max = row
                .top_provider
                .as_ref()
                .and_then(|t| t.max_completion_tokens);
            let per_request_max = row
                .per_request_limits
                .as_ref()
                .and_then(|l| positive_token_count(l.completion_tokens.as_ref()));
            let ceiling = conservative_openrouter_max_output_ceiling(top_max, per_request_max);
            let supports_zdr = if zdr_filtered {
                Some(true)
            } else if zdr_slugs.contains(upstream) {
                Some(true)
            } else {
                None
            };
            openrouter_discovered_model(
                identity,
                &row.id,
                row.name.clone(),
                row.description.clone(),
                conservative_openrouter_context_window(row.context_length, routed_window),
                ceiling,
                &row.supported_parameters,
                input_modalities,
                output_modalities,
                efforts,
                default_effort,
                supports_reasoning,
                supports_zdr,
                manual,
            )
        })
        .collect()
}

fn positive_token_count(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    match value {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().and_then(positive_f64_tokens)),
        Value::String(s) => s.trim().parse().ok().filter(|n| *n > 0),
        _ => None,
    }
}

fn positive_f64_tokens(n: f64) -> Option<u64> {
    if n.is_finite() && n > 0.0 && n <= u64::MAX as f64 {
        Some(n as u64)
    } else {
        None
    }
}

fn with_zdr_query(models_url: &str) -> Result<String, CatalogAdapterError> {
    let mut url =
        reqwest::Url::parse(models_url).map_err(|_| CatalogAdapterError::InvalidOrigin {
            detail: "models list URL is not absolute".into(),
        })?;
    let already = url
        .query_pairs()
        .any(|(k, v)| k.eq_ignore_ascii_case("zdr") && v.eq_ignore_ascii_case("true"));
    if !already {
        url.query_pairs_mut().append_pair("zdr", "true");
    }
    Ok(url.to_string())
}

fn endpoints_zdr_url(models_url: &str) -> Result<String, CatalogAdapterError> {
    let mut url =
        reqwest::Url::parse(models_url).map_err(|_| CatalogAdapterError::InvalidOrigin {
            detail: "models list URL is not absolute".into(),
        })?;
    let path = url.path().trim_end_matches('/');
    let Some(prefix) = path.strip_suffix("/models") else {
        return Err(CatalogAdapterError::InvalidOrigin {
            detail: "cannot derive /endpoints/zdr from models list path".into(),
        });
    };
    url.set_path(&format!("{prefix}/endpoints/zdr"));
    url.set_query(None);
    Ok(url.to_string())
}

fn parse_zdr_endpoint_slugs(body: &[u8]) -> HashSet<String> {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return HashSet::new();
    };
    let Some(data) = value.get("data").and_then(|d| d.as_array()) else {
        return HashSet::new();
    };
    data.iter()
        .filter_map(|item| {
            item.get("model_id")
                .or_else(|| item.get("id"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
        .collect()
}

async fn fetch_zdr_endpoint_slugs(
    models_url: &str,
    bearer_token: &str,
    identity: &CatalogAccountIdentity,
    bounds: CatalogFetchBounds,
    cancel: &CancellationToken,
) -> HashSet<String> {
    if cancel.is_cancelled() {
        return HashSet::new();
    }
    let Ok(url) = endpoints_zdr_url(models_url) else {
        return HashSet::new();
    };
    if super::origin::enforce_same_origin(&url, &identity.endpoint_origin).is_err() {
        return HashSet::new();
    }
    let Ok(client) = reqwest::Client::builder()
        .timeout(bounds.request_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        return HashSet::new();
    };
    let mut budget = CatalogFetchBudget::new(bounds);
    let request = client
        .get(&url)
        .timeout(effective_request_timeout(&budget))
        .bearer_auth(bearer_token.trim());
    let Ok(response) = send_cancellable(request, cancel, &budget).await else {
        return HashSet::new();
    };
    if !response.status().is_success() {
        return HashSet::new();
    }
    let Ok(bytes) = read_body_bounded(response, &mut budget, cancel).await else {
        return HashSet::new();
    };
    parse_zdr_endpoint_slugs(&bytes)
}

fn effort_metadata(model: &OpenRouterModelRow) -> (Vec<String>, Option<String>, bool) {
    use xai_grok_inference_types::ReasoningEffort;

    const ALL: &[&str] = &["max", "xhigh", "high", "medium", "low", "minimal", "none"];
    let normalize = |values: &[String]| {
        values
            .iter()
            .filter_map(|raw| {
                let value = raw.trim().to_ascii_lowercase();
                value.parse::<ReasoningEffort>().ok()?;
                Some(value)
            })
            .collect::<Vec<_>>()
    };

    let param_supports = model.supported_parameters.iter().any(|p| {
        matches!(
            p.to_ascii_lowercase().as_str(),
            "reasoning" | "reasoning_effort" | "include_reasoning"
        )
    });

    if let Some(reasoning) = &model.reasoning {
        let default = reasoning
            .default_effort
            .as_deref()
            .map(str::trim)
            .filter(|v| v.parse::<ReasoningEffort>().is_ok())
            .map(str::to_ascii_lowercase);
        return match &reasoning.supported_efforts {
            OpenRouterSupportedEfforts::Omitted => (Vec::new(), None, param_supports),
            OpenRouterSupportedEfforts::Unrestricted if reasoning.mandatory => {
                let efforts = ALL
                    .iter()
                    .copied()
                    .filter(|v| *v != "none")
                    .map(str::to_owned)
                    .collect();
                (efforts, default, true)
            }
            OpenRouterSupportedEfforts::Unrestricted => (Vec::new(), default, true),
            OpenRouterSupportedEfforts::Exact(values) => {
                let mut efforts = normalize(values);
                if reasoning.mandatory {
                    efforts.retain(|v| v != "none");
                }
                let default = default.filter(|v| efforts.iter().any(|e| e == v));
                let supports = !efforts.is_empty() || param_supports;
                (efforts, default, supports)
            }
        };
    }

    let legacy = model
        .reasoning_effort_options
        .as_deref()
        .map(normalize)
        .unwrap_or_default();
    if !legacy.is_empty() {
        let default = model
            .default_parameters
            .as_ref()
            .and_then(|params| {
                params
                    .get("reasoning_effort")
                    .or_else(|| params.get("reasoningEffort"))
            })
            .and_then(|v| v.as_str())
            .map(str::to_ascii_lowercase)
            .filter(|v| legacy.iter().any(|e| e == v));
        return (legacy, default, true);
    }
    (Vec::new(), None, param_supports)
}

fn parse_openrouter_page(bytes: &[u8]) -> Result<OpenRouterPage, CatalogAdapterError> {
    // Prefer typed parse when data is an array of model objects.
    if let Ok(page) = serde_json::from_slice::<OpenRouterPage>(bytes) {
        return Ok(page);
    }
    // Fallback: require data as array only (never invent object-map models).
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| CatalogAdapterError::Malformed {
            detail: "invalid JSON".into(),
        })?;
    let data_val = value
        .get("data")
        .ok_or_else(|| CatalogAdapterError::Malformed {
            detail: "missing data".into(),
        })?;
    let arr = data_val
        .as_array()
        .ok_or_else(|| CatalogAdapterError::Malformed {
            detail: "data is not an array".into(),
        })?;
    let rows: Vec<OpenRouterModelRow> = arr
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();
    let links = value.get("links").and_then(|l| {
        Some(OpenRouterLinks {
            next: l.get("next").and_then(|n| n.as_str()).map(str::to_owned),
        })
    });
    Ok(OpenRouterPage {
        data: rows,
        links,
        total_count: value.get("total_count").and_then(|v| v.as_i64()),
    })
}

fn with_offset_limit(
    base: &str,
    offset: u32,
    limit: u32,
    include_limit: bool,
) -> Result<String, CatalogAdapterError> {
    let mut url = reqwest::Url::parse(base).map_err(|_| CatalogAdapterError::InvalidOrigin {
        detail: "models list URL is not absolute".into(),
    })?;
    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != "offset" && k != "limit")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    // Bare URL for classic single-shot; attach limit/offset once pagination is active.
    if offset > 0 || include_limit {
        if offset > 0 {
            pairs.push(("offset".into(), offset.to_string()));
        }
        pairs.push(("limit".into(), limit.to_string()));
        url.query_pairs_mut().clear();
        for (k, v) in pairs {
            url.query_pairs_mut().append_pair(&k, &v);
        }
    }
    Ok(url.to_string())
}

/// Parse a single OpenRouter page body into discovered models.
pub fn parse_openrouter_models_body(
    body: &[u8],
    identity: &CatalogAccountIdentity,
    manual: &IndexMap<String, bool>,
) -> Result<Vec<DiscoveredModel>, CatalogAdapterError> {
    parse_openrouter_models_body_with_zdr(body, identity, manual, &HashSet::new(), false)
}

fn parse_openrouter_models_body_with_zdr(
    body: &[u8],
    identity: &CatalogAccountIdentity,
    manual: &IndexMap<String, bool>,
    zdr_slugs: &HashSet<String>,
    zdr_filtered: bool,
) -> Result<Vec<DiscoveredModel>, CatalogAdapterError> {
    let page = parse_openrouter_page(body)?;
    Ok(dedupe_and_sort_models(project_page(
        &page,
        identity,
        manual,
        zdr_slugs,
        zdr_filtered,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_registry::{
        ApiSurface, CredentialBindingId, CredentialRoute, ProviderId, ProviderIncarnation,
        ProviderKind,
    };

    fn id() -> CatalogAccountIdentity {
        CatalogAccountIdentity {
            instance_id: ProviderId::new("openrouter").unwrap(),
            kind: ProviderKind::OpenRouter,
            api_surface: ApiSurface::OpenRouterNative,
            credential_route: CredentialRoute::ApiKey,
            endpoint_origin: "https://openrouter.ai".into(),
            org_project_fingerprint: String::new(),
            incarnation: ProviderIncarnation::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap(),
            credential_binding_id: CredentialBindingId::new("11111111-2222-3333-4444-555555555555")
                .unwrap(),
            is_built_in_compatibility: true,
        }
    }

    #[test]
    fn parses_rich_openrouter_shape() {
        let body = br#"{
          "data": [
            {
              "id": "acme/reasoner",
              "name": "Acme Reasoner",
              "context_length": 262144,
              "top_provider": { "max_completion_tokens": 8192 },
              "architecture": { "input_modalities": ["text", "image"] },
              "supported_parameters": ["tools", "reasoning_effort"]
            }
          ]
        }"#;
        let models = parse_openrouter_models_body(body, &id(), &IndexMap::new()).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].canonical_selection_id, "openrouter:acme/reasoner");
        assert_eq!(models[0].context_window, Some(262144));
        assert_eq!(models[0].max_completion_tokens, None);
        assert_eq!(models[0].max_output_ceiling, Some(8192));
        assert_eq!(models[0].capabilities.max_output_ceiling, Some(8192));
        assert_eq!(models[0].capabilities.supports_tools, Some(true));
        assert_eq!(models[0].capabilities.supports_image_input, Some(true));
        assert_eq!(models[0].capabilities.supports_embeddings, None);
    }

    #[test]
    fn prefers_smaller_routed_provider_context_window() {
        let body = br#"{
          "data": [
            {
              "id": "z-ai/glm-5.3-flash",
              "name": "Z.ai: GLM 5.3 Flash",
              "context_length": 1310720,
              "top_provider": {
                "context_length": 1048576,
                "max_completion_tokens": 131072
              }
            }
          ]
        }"#;
        let models = parse_openrouter_models_body(body, &id(), &IndexMap::new()).unwrap();
        assert_eq!(models[0].context_window, Some(1_048_576));
        assert_eq!(models[0].max_completion_tokens, None);
        assert_eq!(models[0].max_output_ceiling, Some(131072));
    }

    #[test]
    fn ceiling_is_min_of_top_provider_and_per_request_limits() {
        let body = br#"{
          "data": [
            {
              "id": "acme/capped",
              "context_length": 262144,
              "top_provider": { "max_completion_tokens": 131072 },
              "per_request_limits": { "prompt_tokens": 200000, "completion_tokens": 8192 }
            }
          ]
        }"#;
        let models = parse_openrouter_models_body(body, &id(), &IndexMap::new()).unwrap();
        assert_eq!(models[0].context_window, Some(262144));
        assert_eq!(models[0].max_completion_tokens, None);
        assert_eq!(models[0].max_output_ceiling, Some(8192));
    }

    #[test]
    fn does_not_invent_window_or_ceiling_when_absent() {
        let body = br#"{
          "data": [{ "id": "acme/unknown", "supported_parameters": [] }]
        }"#;
        let models = parse_openrouter_models_body(body, &id(), &IndexMap::new()).unwrap();
        assert_eq!(models[0].context_window, None);
        assert_eq!(models[0].max_completion_tokens, None);
        assert_eq!(models[0].max_output_ceiling, None);
    }

    #[test]
    fn projects_modalities_native_schema_and_never_guesses_embeddings() {
        let body = br#"{
          "data": [
            {
              "id": "acme/multimodal",
              "architecture": {
                "input_modalities": ["text", "image", "file", "audio", "video"],
                "output_modalities": ["text", "embeddings"]
              },
              "supported_parameters": ["tools", "tool_choice", "structured_outputs", "response_format"]
            }
          ]
        }"#;
        let models = parse_openrouter_models_body(body, &id(), &IndexMap::new()).unwrap();
        let caps = &models[0].capabilities;
        assert_eq!(caps.supports_tools, Some(true));
        assert_eq!(caps.supports_image_input, Some(true));
        assert_eq!(caps.supports_file_input, Some(true));
        assert_eq!(caps.supports_audio_input, Some(true));
        assert_eq!(caps.supports_video_input, Some(true));
        assert_eq!(caps.output_has_text, Some(true));
        assert_eq!(caps.supports_native_schema, Some(true));
        assert_eq!(caps.supports_embeddings, None);
        assert_eq!(caps.supports_rerank, None);
    }

    #[test]
    fn tags_supports_zdr_from_endpoint_list_without_filtering_full_catalog() {
        let body = br#"{
          "data": [
            { "id": "acme/zdr" },
            { "id": "acme/open" }
          ]
        }"#;
        let zdr = HashSet::from(["acme/zdr".to_owned()]);
        let models =
            parse_openrouter_models_body_with_zdr(body, &id(), &IndexMap::new(), &zdr, false)
                .unwrap();
        assert_eq!(models.len(), 2);
        let by_id: std::collections::HashMap<_, _> = models
            .iter()
            .map(|m| (m.upstream_model_id.as_str(), m))
            .collect();
        assert_eq!(by_id["acme/zdr"].capabilities.supports_zdr, Some(true));
        assert_eq!(by_id["acme/open"].capabilities.supports_zdr, None);
    }

    #[test]
    fn zdr_filtered_list_intersects_endpoint_slugs_when_present() {
        let body = br#"{
          "data": [
            { "id": "acme/zdr" },
            { "id": "acme/other" }
          ]
        }"#;
        let zdr = HashSet::from(["acme/zdr".to_owned()]);
        let models =
            parse_openrouter_models_body_with_zdr(body, &id(), &IndexMap::new(), &zdr, true)
                .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].upstream_model_id, "acme/zdr");
        assert_eq!(models[0].capabilities.supports_zdr, Some(true));
    }

    #[test]
    fn with_zdr_query_appends_true_once() {
        let url = with_zdr_query("https://openrouter.ai/api/v1/models").unwrap();
        assert!(url.contains("zdr=true"));
        let again = with_zdr_query(&url).unwrap();
        assert_eq!(again.matches("zdr=true").count(), 1);
    }

    #[test]
    fn endpoints_zdr_url_replaces_models_path() {
        assert_eq!(
            endpoints_zdr_url("https://openrouter.ai/api/v1/models").unwrap(),
            "https://openrouter.ai/api/v1/endpoints/zdr"
        );
        assert_eq!(
            endpoints_zdr_url("http://127.0.0.1:9/models").unwrap(),
            "http://127.0.0.1:9/endpoints/zdr"
        );
    }

    #[test]
    fn rejects_data_object_map() {
        let body = br#"{"data":{"foo":{"id":"x"}}}"#;
        let err = parse_openrouter_models_body(body, &id(), &IndexMap::new()).unwrap_err();
        assert!(matches!(err, CatalogAdapterError::Malformed { .. }));
    }

    #[test]
    fn rejects_credential_embedded_url() {
        use super::super::origin::validate_list_url_origin;
        let err = validate_list_url_origin(
            "https://user:pass@openrouter.ai/api/v1/models",
            "https://openrouter.ai",
        );
        assert!(err.is_err());
    }
}
