//! Monitor rate limiting — re-exported from `xai-file-utils`.
//!
//! The limiter itself lives in `xai_file_utils::rate_limiter` so the asset
//! transfer jobs and the monitor share one implementation instead of two
//! copies that drift. This module keeps the historical
//! `monitor::rate_limiter::*` paths working for the monitor tool.

pub use xai_file_utils::rate_limiter::{
    AUTO_KILL_THRESHOLD_MS, MonitorRateLimiter, RATE_LIMIT_REFILL_MS, RateLimitOutcome,
    SuppressionTracker, TokenBucket,
};
