//! Shared origin / next-URL validation using PR7 `normalize_endpoint_origin`.

use super::types::CatalogAdapterError;
use crate::provider_registry::normalize_endpoint_origin;

/// Normalize and compare a models-list URL against the account endpoint origin.
pub fn validate_list_url_origin(
    models_url: &str,
    expected_origin: &str,
) -> Result<String, CatalogAdapterError> {
    let origin =
        normalize_endpoint_origin(models_url).map_err(|e| CatalogAdapterError::InvalidOrigin {
            detail: e.to_string(),
        })?;
    if !origin.eq_ignore_ascii_case(expected_origin) {
        return Err(CatalogAdapterError::OriginEscape);
    }
    Ok(origin)
}

/// Same-origin check via PR7 normalization (scheme/host/port, no userinfo).
pub fn enforce_same_origin(url: &str, origin: &str) -> Result<(), CatalogAdapterError> {
    let page_origin =
        normalize_endpoint_origin(url).map_err(|_| CatalogAdapterError::OriginEscape)?;
    if !page_origin.eq_ignore_ascii_case(origin) {
        return Err(CatalogAdapterError::OriginEscape);
    }
    Ok(())
}

/// Validate an OpenRouter `links.next` URL: same origin, same list path prefix,
/// http(s), no userinfo. Returns a redacted cursor key for loop detection
/// (path + sorted safe query keys only — never raw secret-like values).
pub fn validate_models_next_url(
    next: &str,
    origin: &str,
    list_path: &str,
) -> Result<(String, String), CatalogAdapterError> {
    let url = reqwest::Url::parse(next).map_err(|_| CatalogAdapterError::OriginEscape)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(CatalogAdapterError::OriginEscape);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(CatalogAdapterError::OriginEscape);
    }
    enforce_same_origin(next, origin)?;
    let path = url.path();
    // Restrict to the models list path (exact or trailing slash only).
    let allowed = list_path.trim_end_matches('/');
    let got = path.trim_end_matches('/');
    if got != allowed {
        // Also allow /api/v1/models style when list path ends with /models.
        if !(allowed.ends_with("/models") && got.ends_with("/models") && got == allowed) {
            return Err(CatalogAdapterError::OriginEscape);
        }
    }
    // Reject query keys that look like secrets; allow only pagination keys.
    let mut safe_pairs: Vec<(String, String)> = Vec::new();
    for (k, v) in url.query_pairs() {
        let key = k.to_ascii_lowercase();
        if matches!(
            key.as_str(),
            "token" | "key" | "api_key" | "apikey" | "access_token" | "secret" | "authorization"
        ) {
            return Err(CatalogAdapterError::OriginEscape);
        }
        if matches!(
            key.as_str(),
            "offset" | "limit" | "after" | "before" | "page" | "cursor" | "zdr"
        ) {
            safe_pairs.push((key, v.into_owned()));
        }
        // Drop unknown query params from the followed URL reconstruction.
    }
    safe_pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut rebuilt = url.clone();
    rebuilt.set_query(None);
    if !safe_pairs.is_empty() {
        let mut ser = String::new();
        for (i, (k, v)) in safe_pairs.iter().enumerate() {
            if i > 0 {
                ser.push('&');
            }
            ser.push_str(k);
            ser.push('=');
            ser.push_str(v);
        }
        rebuilt.set_query(Some(&ser));
    }
    let cursor_key = format!("next:{}?{}", rebuilt.path(), rebuilt.query().unwrap_or(""));
    Ok((rebuilt.to_string(), cursor_key))
}

/// Path component of a models list URL (for next-link restriction).
pub fn list_path_of(models_url: &str) -> Result<String, CatalogAdapterError> {
    let url = reqwest::Url::parse(models_url).map_err(|_| CatalogAdapterError::InvalidOrigin {
        detail: "models list URL is not absolute".into(),
    })?;
    Ok(url.path().to_owned())
}
