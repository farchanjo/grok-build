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
use super::project::{dedupe_and_sort_models, openrouter_discovered_model};
use super::types::{
    CatalogAccountIdentity, CatalogAdapterError, CatalogFetchSource, CatalogTruncationReason,
    DiscoveredModel, InstanceCatalogResult,
};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;
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
}

#[derive(Debug, Deserialize)]
struct OpenRouterTopProvider {
    #[serde(default)]
    max_completion_tokens: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterReasoningMetadata {
    #[serde(default)]
    supported_efforts: OpenRouterSupportedEfforts,
    #[serde(default)]
    default_effort: Option<String>,
    #[serde(default)]
    mandatory: bool,
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
pub async fn fetch_openrouter_catalog(
    models_url: &str,
    bearer_token: &str,
    identity: &CatalogAccountIdentity,
    manual_capabilities: &IndexMap<String, bool>,
    bounds: CatalogFetchBounds,
    registry_generation: u64,
    publication_generation: u64,
    cancel: &CancellationToken,
) -> Result<InstanceCatalogResult, CatalogAdapterError> {
    // Reuse the bounded body fetch so pagination/bounds stay single-sourced,
    // then project through the adapter's typed path for multi-account catalog.
    let body = fetch_openrouter_bounded_list_body(
        models_url,
        bearer_token,
        &identity.endpoint_origin,
        bounds,
        cancel,
    )
    .await?;
    let page = parse_openrouter_page(&body)?;
    let models = dedupe_and_sort_models(project_page(&page, identity, manual_capabilities));
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
) -> Vec<DiscoveredModel> {
    page.data
        .iter()
        .filter_map(|row| {
            let (efforts, default_effort, supports_reasoning) = effort_metadata(row);
            let modalities = row
                .architecture
                .as_ref()
                .map(|a| a.input_modalities.as_slice())
                .unwrap_or(&[]);
            let max_tokens = row
                .top_provider
                .as_ref()
                .and_then(|t| t.max_completion_tokens)
                .and_then(|n| u32::try_from(n).ok());
            openrouter_discovered_model(
                identity,
                &row.id,
                row.name.clone(),
                row.description.clone(),
                row.context_length,
                max_tokens,
                &row.supported_parameters,
                modalities,
                efforts,
                default_effort,
                supports_reasoning,
                manual,
            )
        })
        .collect()
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
    let page = parse_openrouter_page(body)?;
    Ok(dedupe_and_sort_models(project_page(
        &page, identity, manual,
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
        assert_eq!(models[0].capabilities.supports_embeddings, None);
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
