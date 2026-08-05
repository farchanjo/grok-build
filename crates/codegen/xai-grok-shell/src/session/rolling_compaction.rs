//! Rolling-compaction job and result types.
//!
//! These values move through a bounded capacity-one Tokio lane. They contain a
//! snapshot solely as immutable sampling input; `xai-chat-state` remains the
//! only authoritative live conversation and applies results with CAS.

use std::sync::Arc;

use xai_chat_state::compaction_utils::CompactionMediaDescriptors;
use xai_grok_inference_types::ConversationItem;

#[derive(Debug)]
pub(crate) struct RollingCompactionJob {
    pub identity: xai_chat_state::CompactSourceIdentity,
    pub source_items: Vec<ConversationItem>,
    pub compactor_input_capacity: u64,
    pub prompt_index: usize,
    pub original_user_info: Option<String>,
    /// Frozen media descriptors for every chunk/route of this job.
    pub media_descriptors: Arc<CompactionMediaDescriptors>,
}

#[derive(Debug)]
pub(crate) struct RollingCompactionResult {
    pub identity: xai_chat_state::CompactSourceIdentity,
    pub summary: Result<String, String>,
    pub prompt_index: usize,
    pub original_user_info: Option<String>,
}
