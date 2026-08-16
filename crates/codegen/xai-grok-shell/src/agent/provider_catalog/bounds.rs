//! Explicit safety bounds for authenticated provider catalog pagination.
//!
//! These limits prevent a pathological or malicious `/models` response from
//! hanging refresh, exhausting memory, or looping forever. Bound exceedance is
//! a hard failure for the current account fetch: the prior complete last-known-
//! good catalog is retained and never replaced by a truncated page set.

use std::time::{Duration, Instant};

/// Default maximum HTTP pages fetched for one account catalog refresh.
pub const DEFAULT_MAX_PAGES: u32 = 50;
/// Default maximum models retained from one account catalog refresh.
pub const DEFAULT_MAX_MODELS: usize = 10_000;
/// Default maximum total response body bytes across all pages (8 MiB).
pub const DEFAULT_MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024;
/// Default maximum bytes accepted for a single page body (2 MiB).
pub const DEFAULT_MAX_PAGE_BYTES: u64 = 2 * 1024 * 1024;
/// Default wall-clock budget for one account's full paginated fetch.
pub const DEFAULT_MAX_DURATION: Duration = Duration::from_secs(60);
/// Default per-request HTTP timeout.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Default page size hint for offset/limit providers (OpenRouter).
pub const DEFAULT_PAGE_SIZE: u32 = 100;

/// Explicit pagination and size bounds for one account catalog fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogFetchBounds {
    pub max_pages: u32,
    pub max_models: usize,
    pub max_total_bytes: u64,
    pub max_page_bytes: u64,
    pub max_duration: Duration,
    pub request_timeout: Duration,
    pub page_size: u32,
}

impl Default for CatalogFetchBounds {
    fn default() -> Self {
        Self {
            max_pages: DEFAULT_MAX_PAGES,
            max_models: DEFAULT_MAX_MODELS,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_page_bytes: DEFAULT_MAX_PAGE_BYTES,
            max_duration: DEFAULT_MAX_DURATION,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl CatalogFetchBounds {
    /// Override the request timeout (for example from provider config).
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Override the overall wall-clock deadline.
    pub fn with_max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = duration;
        self
    }
}

/// Running counters for one in-flight paginated fetch.
#[derive(Debug, Clone)]
pub struct CatalogFetchBudget {
    bounds: CatalogFetchBounds,
    started: Instant,
    deadline: Instant,
    pages: u32,
    models: usize,
    total_bytes: u64,
    /// Cursor / next-URL / offset values already observed (loop detection).
    seen_cursors: std::collections::BTreeSet<String>,
}

impl CatalogFetchBudget {
    pub fn new(bounds: CatalogFetchBounds) -> Self {
        let started = Instant::now();
        let deadline = started + bounds.max_duration;
        Self {
            bounds,
            started,
            deadline,
            pages: 0,
            models: 0,
            total_bytes: 0,
            seen_cursors: std::collections::BTreeSet::new(),
        }
    }

    pub fn bounds(&self) -> CatalogFetchBounds {
        self.bounds
    }

    pub fn pages_fetched(&self) -> u32 {
        self.pages
    }

    pub fn models_collected(&self) -> usize {
        self.models
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Remaining time until the overall deadline (zero when expired).
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// Record one successful page body. Fails when page/total/time bounds trip.
    pub fn record_page(&mut self, page_bytes: u64) -> Result<(), CatalogBoundError> {
        if Instant::now() >= self.deadline {
            return Err(CatalogBoundError::DeadlineExceeded);
        }
        if page_bytes > self.bounds.max_page_bytes {
            return Err(CatalogBoundError::PageBytesExceeded {
                got: page_bytes,
                max: self.bounds.max_page_bytes,
            });
        }
        let next_total = self.total_bytes.saturating_add(page_bytes);
        if next_total > self.bounds.max_total_bytes {
            return Err(CatalogBoundError::TotalBytesExceeded {
                got: next_total,
                max: self.bounds.max_total_bytes,
            });
        }
        let next_pages = self.pages.saturating_add(1);
        if next_pages > self.bounds.max_pages {
            return Err(CatalogBoundError::PageCountExceeded {
                max: self.bounds.max_pages,
            });
        }
        self.pages = next_pages;
        self.total_bytes = next_total;
        Ok(())
    }

    /// Record models admitted from one page. Fails when the model cap is hit
    /// mid-page (caller must not publish a partial account catalog).
    pub fn record_models(&mut self, additional: usize) -> Result<(), CatalogBoundError> {
        let next = self.models.saturating_add(additional);
        if next > self.bounds.max_models {
            return Err(CatalogBoundError::ModelCountExceeded {
                got: next,
                max: self.bounds.max_models,
            });
        }
        self.models = next;
        Ok(())
    }

    /// Remember a pagination cursor/URL/offset. Returns an error on loops.
    pub fn remember_cursor(&mut self, cursor: &str) -> Result<(), CatalogBoundError> {
        if cursor.is_empty() {
            return Ok(());
        }
        if !self.seen_cursors.insert(cursor.to_owned()) {
            return Err(CatalogBoundError::CursorLoop);
        }
        Ok(())
    }

    pub fn check_deadline(&self) -> Result<(), CatalogBoundError> {
        if Instant::now() >= self.deadline {
            Err(CatalogBoundError::DeadlineExceeded)
        } else {
            Ok(())
        }
    }
}

/// Bound or loop failure during catalog pagination. Never carries secrets,
/// custom URLs with credentials, or raw response bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogBoundError {
    PageCountExceeded { max: u32 },
    ModelCountExceeded { got: usize, max: usize },
    PageBytesExceeded { got: u64, max: u64 },
    TotalBytesExceeded { got: u64, max: u64 },
    DeadlineExceeded,
    CursorLoop,
}

impl std::fmt::Display for CatalogBoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PageCountExceeded { max } => {
                write!(f, "catalog page count exceeded bound ({max})")
            }
            Self::ModelCountExceeded { got, max } => {
                write!(f, "catalog model count {got} exceeded bound ({max})")
            }
            Self::PageBytesExceeded { got, max } => {
                write!(f, "catalog page size {got} bytes exceeded bound ({max})")
            }
            Self::TotalBytesExceeded { got, max } => {
                write!(
                    f,
                    "catalog total response size {got} bytes exceeded bound ({max})"
                )
            }
            Self::DeadlineExceeded => write!(f, "catalog fetch deadline exceeded"),
            Self::CursorLoop => write!(f, "catalog pagination cursor/URL loop detected"),
        }
    }
}

impl std::error::Error for CatalogBoundError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_count_bound_trips() {
        let mut budget = CatalogFetchBudget::new(CatalogFetchBounds {
            max_pages: 2,
            ..CatalogFetchBounds::default()
        });
        budget.record_page(10).unwrap();
        budget.record_page(10).unwrap();
        assert!(matches!(
            budget.record_page(10),
            Err(CatalogBoundError::PageCountExceeded { max: 2 })
        ));
    }

    #[test]
    fn cursor_loop_detected() {
        let mut budget = CatalogFetchBudget::new(CatalogFetchBounds::default());
        budget.remember_cursor("after=abc").unwrap();
        assert_eq!(
            budget.remember_cursor("after=abc"),
            Err(CatalogBoundError::CursorLoop)
        );
    }

    #[test]
    fn model_count_bound_trips() {
        let mut budget = CatalogFetchBudget::new(CatalogFetchBounds {
            max_models: 3,
            ..CatalogFetchBounds::default()
        });
        budget.record_models(2).unwrap();
        assert!(matches!(
            budget.record_models(2),
            Err(CatalogBoundError::ModelCountExceeded { .. })
        ));
    }
}
