//! Bounded HTTP body reads for catalog adapters.
//!
//! Caps are enforced *before* full allocation and before JSON decode:
//! Content-Length precheck, then chunked accumulation with a hard remaining
//! budget. Cancellation and remaining wall-clock deadline are raced around
//! both `send()` and body reads.

use super::bounds::{CatalogBoundError, CatalogFetchBudget};
use super::types::CatalogAdapterError;
use futures_util::StreamExt;
use reqwest::Response;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Send an HTTP request raced against cancellation and the catalog deadline.
pub async fn send_cancellable(
    request: reqwest::RequestBuilder,
    cancel: &CancellationToken,
    budget: &CatalogFetchBudget,
) -> Result<Response, CatalogAdapterError> {
    let remaining = budget.remaining();
    if remaining.is_zero() {
        return Err(CatalogBoundError::DeadlineExceeded.into());
    }
    if cancel.is_cancelled() {
        return Err(CatalogAdapterError::Cancelled);
    }
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(CatalogAdapterError::Cancelled),
        _ = tokio::time::sleep(remaining) => Err(CatalogBoundError::DeadlineExceeded.into()),
        result = request.send() => {
            result.map_err(|e| CatalogAdapterError::Transport {
                detail: CatalogAdapterError::sanitize_detail(&e.to_string()),
            })
        }
    }
}

/// Read a response body with Content-Length precheck + chunked hard cap.
///
/// Never calls `response.bytes()` unbounded. Records the page against `budget`
/// only after a complete successful bounded read.
pub async fn read_body_bounded(
    response: Response,
    budget: &mut CatalogFetchBudget,
    cancel: &CancellationToken,
) -> Result<Vec<u8>, CatalogAdapterError> {
    let max_page = budget.bounds().max_page_bytes;
    let remaining_total = budget
        .bounds()
        .max_total_bytes
        .saturating_sub(budget.total_bytes());
    let hard_cap = max_page.min(remaining_total);
    if hard_cap == 0 {
        return Err(CatalogBoundError::TotalBytesExceeded {
            got: budget.total_bytes(),
            max: budget.bounds().max_total_bytes,
        }
        .into());
    }

    // Content-Length precheck when present (lying values still hit chunk cap).
    if let Some(declared) = response.content_length() {
        if declared > max_page {
            return Err(CatalogBoundError::PageBytesExceeded {
                got: declared,
                max: max_page,
            }
            .into());
        }
        if declared > remaining_total {
            return Err(CatalogBoundError::TotalBytesExceeded {
                got: budget.total_bytes().saturating_add(declared),
                max: budget.bounds().max_total_bytes,
            }
            .into());
        }
    }

    budget.check_deadline()?;
    if cancel.is_cancelled() {
        return Err(CatalogAdapterError::Cancelled);
    }

    let mut buf: Vec<u8> = Vec::new();
    // Reserve only up to hard_cap so a huge Content-Length cannot allocate.
    if let Some(declared) = response.content_length() {
        let reserve = (declared as usize).min(hard_cap as usize);
        buf.try_reserve_exact(reserve)
            .map_err(|_| CatalogAdapterError::Transport {
                detail: "response body allocation failed".into(),
            })?;
    }

    let mut stream = response.bytes_stream();
    loop {
        let remaining = budget.remaining();
        if remaining.is_zero() {
            return Err(CatalogBoundError::DeadlineExceeded.into());
        }
        let next = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(CatalogAdapterError::Cancelled),
            _ = tokio::time::sleep(remaining) => {
                return Err(CatalogBoundError::DeadlineExceeded.into());
            }
            chunk = stream.next() => chunk,
        };
        match next {
            None => break,
            Some(Err(e)) => {
                return Err(CatalogAdapterError::Transport {
                    detail: CatalogAdapterError::sanitize_detail(&e.to_string()),
                });
            }
            Some(Ok(chunk)) => {
                let add = chunk.len() as u64;
                let new_len = (buf.len() as u64).saturating_add(add);
                if new_len > max_page {
                    return Err(CatalogBoundError::PageBytesExceeded {
                        got: new_len,
                        max: max_page,
                    }
                    .into());
                }
                if new_len > hard_cap {
                    return Err(CatalogBoundError::TotalBytesExceeded {
                        got: budget.total_bytes().saturating_add(new_len),
                        max: budget.bounds().max_total_bytes,
                    }
                    .into());
                }
                buf.extend_from_slice(&chunk);
            }
        }
    }

    budget.record_page(buf.len() as u64)?;
    Ok(buf)
}

/// Effective per-request timeout from the remaining deadline and request timeout.
pub fn effective_request_timeout(budget: &CatalogFetchBudget) -> Duration {
    budget.remaining().min(budget.bounds().request_timeout)
}
