//! Rolling-compaction job and result types.
//!
//! These values move through a bounded capacity-one Tokio lane. They contain
//! a snapshot solely as immutable sampling input; `xai-chat-state` remains
//! the only authoritative live conversation and applies results with CAS.
//!
//! Media enrichment (plan §14.3) is wired in `acp_session_impl/rolling.rs`:
//! `sample_rolling_source` enriches `source_items` exactly once per job via
//! the canonical preflight (`SessionActor::prepare_compaction_source`) and
//! reuses that enriched snapshot for chunks, bisections, merges, and route
//! fallbacks. The `identity` fingerprint stays on the raw items so CAS
//! staleness detection is unaffected by enrichment.

use xai_grok_inference_types::ConversationItem;

#[derive(Debug)]
pub(crate) struct RollingCompactionJob {
    pub identity: xai_chat_state::CompactSourceIdentity,
    pub source_items: Vec<ConversationItem>,
    pub compactor_input_capacity: u64,
    pub prompt_index: usize,
    pub original_user_info: Option<String>,
}

#[derive(Debug)]
pub(crate) struct RollingCompactionResult {
    pub identity: xai_chat_state::CompactSourceIdentity,
    pub summary: Result<String, String>,
    pub prompt_index: usize,
    pub original_user_info: Option<String>,
}
