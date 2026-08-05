//! Shared domain types for the chat state actor.

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use xai_grok_inference_types::{ConversationItem, InferenceSettings};

/// Canonical marker for an injected memory-context block. Shared by the
/// emitter in `xai-grok-shell` and the upsert/detection here — a drift would
/// silently break dedup and let blocks accumulate in the prompt prefix.
/// Detection assumes the literal never appears in a system prompt except as
/// an injected block.
pub const MEMORY_CONTEXT_OPEN_TAG: &str = "<memory-context>";

/// Closing tag paired with [`MEMORY_CONTEXT_OPEN_TAG`].
pub const MEMORY_CONTEXT_CLOSE_TAG: &str = "</memory-context>";

/// Identity of the exact conversation range submitted to a background compactor.
///
/// The actor verifies all fields before applying a rolling replacement. The epoch
/// rejects any intervening conversation mutation, while the fingerprint protects
/// against an incorrectly constructed or corrupted source identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactSourceIdentity {
    /// The structural epoch that must match for the splice to succeed.
    pub expected_epoch: u64,
    /// The starting index (inclusive) of the source range in the conversation.
    pub source_start: usize,
    /// The ending index (exclusive) of the source range in the conversation.
    pub source_end: usize,
    /// SHA-256 of the length-delimited JSON encoding of the source items.
    pub source_fingerprint: [u8; 32],
}

impl CompactSourceIdentity {
    /// Construct an identity for `items` at the given epoch and source range.
    pub fn new(
        expected_epoch: u64,
        source_start: usize,
        source_end: usize,
        items: &[ConversationItem],
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            expected_epoch,
            source_start,
            source_end,
            source_fingerprint: fingerprint_conversation_items(items)?,
        })
    }
}

/// Compute a deterministic, unambiguous fingerprint for conversation items.
///
/// Each serialized item is prefixed with its byte length so adjacent JSON values
/// cannot produce an ambiguous concatenation. Serialization errors are returned
/// rather than panicking inside the chat-state actor.
pub fn fingerprint_conversation_items(
    items: &[ConversationItem],
) -> Result<[u8; 32], serde_json::Error> {
    let mut hasher = Sha256::new();
    for item in items {
        let encoded = serde_json::to_vec(item)?;
        hasher.update((encoded.len() as u64).to_le_bytes());
        hasher.update(encoded);
    }
    Ok(hasher.finalize().into())
}

/// Configuration for the ChatStateActor at spawn time.
#[derive(Debug, Clone)]
pub struct ChatStateConfig {
    /// Initial conversation items to populate the state with.
    pub initial_conversation: Vec<ConversationItem>,
    /// Sampling configuration (model, context window, etc.).
    pub inference_settings: InferenceSettings,
}

/// Immutable snapshot of the actor's state (for forking, rewind).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStateSnapshot {
    /// The full conversation history.
    pub conversation: Vec<ConversationItem>,
    /// Current inference settings (JSON key kept as `sampling_config` for snapshot compat).
    #[serde(rename = "sampling_config")]
    pub inference_settings: InferenceSettings,
    /// Current prompt index (incremented per user turn).
    pub prompt_index: usize,
    /// Accumulated token usage.
    pub total_tokens: u64,
    /// Bytes/4 estimate of the conversation as of the last `record_token_usage`.
    /// `0` means unknown (pre-field snapshot); restore re-estimates instead.
    #[serde(default)]
    pub estimate_at_last_response: u64,
    /// File paths the agent has edited.
    pub agent_edited_paths: BTreeSet<String>,
    /// Cached prompt texts for rewind preview.
    pub prompt_texts: Vec<String>,
    /// Timestamp when the current stream started (epoch ms).
    pub stream_start_ms: Option<i64>,
    /// Timestamp when the current turn started (epoch ms).
    pub turn_start_ms: Option<i64>,
    /// Prompt index at which the last compaction occurred.
    pub last_compaction_prompt_index: Option<usize>,
    /// Opaque credential secrets (API key, optional extra auth, client version).
    #[serde(default)]
    pub credentials: Credentials,
    /// Epoch counter incremented on each structural change (replace_conversation).
    /// Used to detect stale snapshots; starts at 0 with serde default.
    #[serde(default)]
    pub structural_epoch: u64,
}

/// Metadata for session notifications (timing info).
#[derive(Debug, Clone)]
pub struct NotificationMeta {
    /// Timestamp when the current stream started (epoch ms).
    pub stream_start_ms: Option<i64>,
    /// Timestamp when the current turn started (epoch ms).
    pub turn_start_ms: Option<i64>,
}

/// Configuration for tool-result pruning.
///
/// Prunes old, large tool results from the conversation to reclaim context space.
/// Two modes: soft trim (keep head + tail) and hard clear (replace entirely).
#[derive(Debug, Clone)]
pub struct PruningConfig {
    /// Whether pruning is enabled.
    pub enabled: bool,
    /// Number of recent turns whose tool results are never pruned.
    pub keep_last_n_turns: usize,
    /// Character threshold above which old tool results are soft-trimmed.
    pub soft_trim_threshold: usize,
    /// Characters to keep from the start of a soft-trimmed result.
    pub soft_trim_head: usize,
    /// Characters to keep from the end of a soft-trimmed result.
    pub soft_trim_tail: usize,
    /// Turn age after which tool results are hard-cleared (replaced with placeholder).
    pub hard_clear_age_turns: usize,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            keep_last_n_turns: 3,
            soft_trim_threshold: 4000,
            soft_trim_head: 1500,
            soft_trim_tail: 1500,
            hard_clear_age_turns: 10,
        }
    }
}

/// Where the session's current api_key came from.
/// Determines whether the key can be refreshed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// From AuthManager (grok login, OIDC, external binary). Refreshable.
    #[default]
    SessionToken,
    /// From user config ([model.*] api_key, env_key, XAI_API_KEY). Not refreshable.
    ApiKey,
}

/// Credential/secret fields that the actor stores opaquely.
///
/// These are fields from the shell's full `Config` that aren't part of
/// `xai_grok_inference_types::InferenceSettings` (which is secret-free).
/// The actor just stores and returns them — it never interprets them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Credentials {
    /// API key for authentication.
    pub api_key: Option<String>,
    /// Whether this is a session token (refreshable) or user-provided api key.
    #[serde(default)]
    pub auth_type: AuthType,
    /// Optional extra auth material forwarded with requests when present.
    pub alpha_test_key: Option<String>,
    /// Client version string.
    pub client_version: Option<String>,
}

/// The messages captured during a single conversation turn.
///
/// Produced by `TakeTurnMessages` after a `BeginTurnCapture`/message-push cycle.
#[derive(Debug, Clone)]
pub struct TurnCapture {
    /// The ordered sequence of messages appended during this turn.
    pub messages: Vec<ConversationItem>,
    /// Whether compaction (conversation replacement) occurred mid-turn.
    pub compaction_occurred: bool,
}

/// Item counts for a conversation, broken down by role.
///
/// Returned by `get_conversation_counts()` — avoids cloning the conversation
/// when only role counts and total length are needed (e.g. for telemetry).
#[derive(Debug, Clone, Default)]
pub struct ConversationCounts {
    /// Total number of items in the conversation.
    pub total: usize,
    /// Number of `User` items.
    pub user: usize,
    /// Number of `Assistant` items.
    pub assistant: usize,
    /// Number of `ToolResult` items.
    pub tool_result: usize,
}

/// Info returned when auto-compact threshold is exceeded.
#[derive(Debug, Clone)]
pub struct AutoCompactTrigger {
    /// Current total token count.
    pub total_tokens: u64,
    /// Model's context window size.
    pub context_window: NonZeroU64,
    /// Current utilization as a percentage (0–100).
    pub utilization_percent: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trips_through_serde_json() {
        let snapshot = ChatStateSnapshot {
            conversation: vec![],
            inference_settings: InferenceSettings {
                base_url: "https://api.example.com".to_string(),
                model: "test-model".to_string(),
                max_completion_tokens: None,
                temperature: None,
                top_p: None,
                api_backend: Default::default(),
                extra_headers: Default::default(),
                context_window: NonZeroU64::new(128_000).unwrap(),
                reasoning_effort: None,
                stream_tool_calls: None,
                supports_native_schema: None,
                supports_strict_tools: None,
                supports_image_input: None,
                supports_audio_input: None,
                supports_video_input: None,
            },
            prompt_index: 0,
            total_tokens: 0,
            estimate_at_last_response: 0,
            agent_edited_paths: BTreeSet::new(),
            prompt_texts: vec![],
            stream_start_ms: None,
            turn_start_ms: None,
            last_compaction_prompt_index: None,
            credentials: Credentials::default(),
            structural_epoch: 0,
        };

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let deserialized: ChatStateSnapshot = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.prompt_index, 0);
        assert_eq!(deserialized.total_tokens, 0);
        assert!(deserialized.conversation.is_empty());
        assert!(deserialized.agent_edited_paths.is_empty());
        assert!(deserialized.last_compaction_prompt_index.is_none());
        assert_eq!(deserialized.structural_epoch, 0);
    }

    #[test]
    fn snapshot_round_trips_native_schema_capability() {
        let snapshot = ChatStateSnapshot {
            conversation: vec![],
            inference_settings: InferenceSettings {
                base_url: "https://api.anthropic.com/v1".to_string(),
                model: "claude-sonnet-5".to_string(),
                max_completion_tokens: Some(8192),
                temperature: None,
                top_p: None,
                api_backend: xai_grok_inference_types::ApiBackend::Messages,
                extra_headers: Default::default(),
                context_window: NonZeroU64::new(200_000).unwrap(),
                reasoning_effort: Some(xai_grok_inference_types::ReasoningEffort::High),
                stream_tool_calls: None,
                supports_native_schema: Some(true),
                supports_strict_tools: Some(false),
                supports_image_input: None,
                supports_audio_input: None,
                supports_video_input: None,
            },
            prompt_index: 1,
            total_tokens: 10,
            estimate_at_last_response: 0,
            agent_edited_paths: BTreeSet::new(),
            prompt_texts: vec!["hi".into()],
            stream_start_ms: None,
            turn_start_ms: None,
            last_compaction_prompt_index: None,
            credentials: Credentials::default(),
            structural_epoch: 0,
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        let deserialized: ChatStateSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            deserialized.inference_settings.supports_native_schema,
            Some(true)
        );
        assert_eq!(
            deserialized.inference_settings.supports_strict_tools,
            Some(false)
        );
        assert!(
            deserialized
                .inference_settings
                .effective_supports_native_schema(),
            "restored Messages + supports_native_schema=true selects native schema"
        );
        assert_eq!(
            deserialized.inference_settings.api_backend,
            xai_grok_inference_types::ApiBackend::Messages
        );
    }

    #[test]
    fn snapshot_round_trips_with_data() {
        use xai_grok_inference_types::ConversationItem;

        let snapshot = ChatStateSnapshot {
            conversation: vec![
                ConversationItem::system("You are a helpful assistant."),
                ConversationItem::user("Hello!"),
                ConversationItem::assistant("Hi there!"),
            ],
            inference_settings: InferenceSettings {
                base_url: "https://api.example.com".to_string(),
                model: "grok-3".to_string(),
                max_completion_tokens: Some(4096),
                temperature: Some(0.7),
                top_p: None,
                api_backend: Default::default(),
                extra_headers: Default::default(),
                context_window: NonZeroU64::new(128_000).unwrap(),
                reasoning_effort: None,
                stream_tool_calls: None,
                supports_native_schema: None,
                supports_strict_tools: None,
                supports_image_input: None,
                supports_audio_input: None,
                supports_video_input: None,
            },
            prompt_index: 5,
            total_tokens: 1234,
            estimate_at_last_response: 900,
            agent_edited_paths: BTreeSet::from([
                "src/main.rs".to_string(),
                "src/lib.rs".to_string(),
            ]),
            prompt_texts: vec!["first prompt".to_string(), "second prompt".to_string()],
            stream_start_ms: Some(1234567890),
            turn_start_ms: Some(1234567800),
            last_compaction_prompt_index: Some(2),
            credentials: Credentials::default(),
            structural_epoch: 42,
        };

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let deserialized: ChatStateSnapshot = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.prompt_index, 5);
        assert_eq!(deserialized.total_tokens, 1234);
        assert_eq!(deserialized.conversation.len(), 3);
        assert_eq!(deserialized.agent_edited_paths.len(), 2);
        assert_eq!(deserialized.prompt_texts.len(), 2);
        assert_eq!(deserialized.stream_start_ms, Some(1234567890));
        assert_eq!(deserialized.turn_start_ms, Some(1234567800));
        assert_eq!(deserialized.last_compaction_prompt_index, Some(2));
        assert_eq!(deserialized.structural_epoch, 42);
    }

    #[test]
    fn snapshot_structural_epoch_defaults_to_zero_for_legacy() {
        // Simulate a legacy snapshot without structural_epoch field
        let legacy_json = r#"{
            "conversation": [],
            "sampling_config": {
                "base_url": "https://api.example.com",
                "model": "test-model",
                "context_window": 128000
            },
            "prompt_index": 0,
            "total_tokens": 0,
            "agent_edited_paths": [],
            "prompt_texts": []
        }"#;
        let deserialized: ChatStateSnapshot =
            serde_json::from_str(legacy_json).expect("deserialize");
        assert_eq!(
            deserialized.structural_epoch, 0,
            "legacy snapshots should default to 0"
        );
    }

    #[test]
    fn compact_source_identity_fingerprints_source_items() {
        let items = [ConversationItem::user("hello")];
        let identity = CompactSourceIdentity::new(5, 1, 2, &items).expect("fingerprint");

        assert_eq!(identity.expected_epoch, 5);
        assert_eq!(identity.source_start, 1);
        assert_eq!(identity.source_end, 2);
        assert_eq!(
            identity.source_fingerprint,
            fingerprint_conversation_items(&items).expect("fingerprint")
        );
    }

    #[test]
    fn item_boundaries_contribute_to_fingerprint() {
        let split = [ConversationItem::user("a"), ConversationItem::user("b")];
        let joined = [ConversationItem::user("ab")];

        assert_ne!(
            fingerprint_conversation_items(&split).expect("fingerprint"),
            fingerprint_conversation_items(&joined).expect("fingerprint")
        );
    }
}
