//! OpenAI-platform catalog adapter with bounded authenticated pagination.
//!
//! Documented shape (OpenAI Platform `GET /v1/models`):
//! ```json
//! { "object": "list", "data": [ { "id": "...", "object": "model",
//!   "created": 0, "owned_by": "..." } ] }
//! ```
//! Pinned OpenAI is a single page (`ListModelsParams` is empty). Compatible
//! gateways may set `has_more: true` with an explicit `after` or `last_id`
//! cursor. Continuation requires **explicit** `has_more == true` plus an
//! explicit cursor — never invent has-more from cursor presence, never
//! synthesize a cursor from the last model id.

use super::bounds::{CatalogBoundError, CatalogFetchBounds, CatalogFetchBudget};
use super::http_body::{effective_request_timeout, read_body_bounded, send_cancellable};
use super::origin::{enforce_same_origin, validate_list_url_origin};
use super::project::{dedupe_and_sort_models, openai_discovered_model};
use super::types::{
    CatalogAccountIdentity, CatalogAdapterError, CatalogFetchSource, CatalogTruncationReason,
    DiscoveredModel, InstanceCatalogResult,
};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize)]
struct OpenAiModelsPage {
    #[serde(default)]
    data: Vec<OpenAiModelRow>,
    #[serde(default)]
    has_more: Option<bool>,
    #[serde(default)]
    after: Option<String>,
    #[serde(default)]
    last_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelRow {
    id: String,
}

/// Live fetch for one OpenAI (or OpenAI-compatible identity-list) account.
pub async fn fetch_openai_catalog(
    models_url: &str,
    bearer_token: &str,
    identity: &CatalogAccountIdentity,
    manual_capabilities: &IndexMap<String, bool>,
    bounds: CatalogFetchBounds,
    registry_generation: u64,
    publication_generation: u64,
    cancel: &CancellationToken,
) -> Result<InstanceCatalogResult, CatalogAdapterError> {
    if bearer_token.trim().is_empty() {
        return Err(CatalogAdapterError::MissingCredential);
    }
    let origin = validate_list_url_origin(models_url, &identity.endpoint_origin)?;
    let client = reqwest::Client::builder()
        .timeout(bounds.request_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| CatalogAdapterError::Transport {
            detail: CatalogAdapterError::sanitize_detail(&e.to_string()),
        })?;

    let mut budget = CatalogFetchBudget::new(bounds);
    let mut models: Vec<DiscoveredModel> = Vec::new();
    let mut after_cursor: Option<String> = None;
    budget.remember_cursor("page:0")?;

    loop {
        if cancel.is_cancelled() {
            return Err(CatalogAdapterError::Cancelled);
        }
        budget.check_deadline()?;

        let page_url = match &after_cursor {
            Some(cursor) => append_after_query(models_url, cursor)?,
            None => models_url.to_owned(),
        };
        enforce_same_origin(&page_url, &origin)?;

        // Apply remaining-deadline-aware timeout per hop.
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
        let page = parse_openai_page(&bytes)?;
        let page_models: Vec<DiscoveredModel> = page
            .data
            .iter()
            .filter_map(|row| openai_discovered_model(identity, &row.id, manual_capabilities))
            .collect();
        budget.record_models(page_models.len())?;
        models.extend(page_models);

        // Official OpenAI: single page. Compatible gateways: continue only
        // when has_more is explicitly true with an explicit cursor.
        if page.has_more != Some(true) {
            break;
        }
        let Some(cursor) = explicit_openai_cursor(&page) else {
            return Err(CatalogAdapterError::Malformed {
                detail: "has_more true without explicit after/last_id cursor".into(),
            });
        };
        if budget.pages_fetched() >= budget.bounds().max_pages {
            return Err(CatalogBoundError::PageCountExceeded {
                max: budget.bounds().max_pages,
            }
            .into());
        }
        budget.remember_cursor(&format!("after:{cursor}"))?;
        after_cursor = Some(cursor);
    }

    let models = dedupe_and_sort_models(models);
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

fn parse_openai_page(bytes: &[u8]) -> Result<OpenAiModelsPage, CatalogAdapterError> {
    if let Ok(page) = serde_json::from_slice::<OpenAiModelsPage>(bytes) {
        return Ok(page);
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| CatalogAdapterError::Malformed {
            detail: "invalid JSON".into(),
        })?;
    let data = value
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| CatalogAdapterError::Malformed {
            detail: "missing data array".into(),
        })?;
    let rows = data
        .iter()
        .filter_map(|m| {
            m.get("id")
                .and_then(|i| i.as_str())
                .map(|id| OpenAiModelRow { id: id.to_owned() })
        })
        .collect();
    Ok(OpenAiModelsPage {
        data: rows,
        has_more: value.get("has_more").and_then(|v| v.as_bool()),
        after: value
            .get("after")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        last_id: value
            .get("last_id")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
    })
}

/// Explicit documented cursor only — never synthesize from last row id.
fn explicit_openai_cursor(page: &OpenAiModelsPage) -> Option<String> {
    page.after
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            page.last_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
}

fn append_after_query(base: &str, after: &str) -> Result<String, CatalogAdapterError> {
    let mut url = reqwest::Url::parse(base).map_err(|_| CatalogAdapterError::InvalidOrigin {
        detail: "models list URL is not absolute".into(),
    })?;
    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != "after")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.push(("after".into(), after.to_owned()));
    url.query_pairs_mut().clear();
    for (k, v) in pairs {
        url.query_pairs_mut().append_pair(&k, &v);
    }
    Ok(url.to_string())
}

/// Parse a single OpenAI page body into discovered models (test / LKG helper).
pub fn parse_openai_models_body(
    body: &[u8],
    identity: &CatalogAccountIdentity,
    manual: &IndexMap<String, bool>,
) -> Result<Vec<DiscoveredModel>, CatalogAdapterError> {
    let page = parse_openai_page(body)?;
    let models = page
        .data
        .iter()
        .filter_map(|row| openai_discovered_model(identity, &row.id, manual))
        .collect();
    Ok(dedupe_and_sort_models(models))
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
            instance_id: ProviderId::new("openai").unwrap(),
            kind: ProviderKind::OpenAi,
            api_surface: ApiSurface::OpenAiPlatform,
            credential_route: CredentialRoute::ApiKey,
            endpoint_origin: "https://api.openai.com".into(),
            org_project_fingerprint: String::new(),
            incarnation: ProviderIncarnation::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap(),
            credential_binding_id: CredentialBindingId::new("11111111-2222-3333-4444-555555555555")
                .unwrap(),
            is_built_in_compatibility: true,
        }
    }

    #[test]
    fn parses_single_page_identity_list() {
        let body = br#"{"object":"list","data":[{"id":"gpt-4o"},{"id":"gpt-4o"},{"id":"o1"}]}"#;
        let models = parse_openai_models_body(body, &id(), &IndexMap::new()).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].canonical_selection_id, "openai:gpt-4o");
    }

    #[test]
    fn last_id_without_has_more_is_not_continuation() {
        let page = OpenAiModelsPage {
            data: vec![OpenAiModelRow { id: "m1".into() }],
            has_more: Some(false),
            after: None,
            last_id: Some("m1".into()),
        };
        // has_more false → no continuation even with last_id present.
        assert_ne!(page.has_more, Some(true));
        assert!(explicit_openai_cursor(&page).is_some()); // cursor exists but unused
    }

    #[test]
    fn has_more_without_cursor_is_malformed_path() {
        let page = OpenAiModelsPage {
            data: vec![OpenAiModelRow { id: "m1".into() }],
            has_more: Some(true),
            after: None,
            last_id: None,
        };
        assert!(explicit_openai_cursor(&page).is_none());
    }
}
