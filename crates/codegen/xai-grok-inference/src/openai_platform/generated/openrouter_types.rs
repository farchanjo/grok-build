//! Schema-derived types from pinned OpenAPI. DO NOT EDIT BY HAND.

use crate::openai_platform::transport::SseEvent;
use serde::{Deserialize, Serialize};

/// Generated union `BaseInputsV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BaseInputsV1ItemUnion {
    Variant0(BaseInputsV1ItemV0),
    Variant1(OrOpenAIResponseInputMessageItem),
    Variant2(OrOpenAIResponseFunctionToolCallOutput),
    Variant3(OrOpenAIResponseFunctionToolCall),
    Variant4(OrOutputItemImageGenerationCall),
    Variant5(OrOutputMessage),
    Variant6(OrOpenAIResponseCustomToolCall),
    Variant7(OrOpenAIResponseCustomToolCallOutput),
    Variant8(OrApplyPatchCallItem),
    Variant9(OrApplyPatchCallOutputItem),
    Unknown(serde_json::Value),
}
impl Default for BaseInputsV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `BaseInputsV1ItemV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BaseInputsV1ItemV0 {
    pub content: BaseInputsV1ItemV0ContentUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<BaseInputsV1ItemV0PhaseUnion>,
    pub role: BaseInputsV1ItemV0RoleUnion,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<BaseInputsV1ItemV0TypeEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `BaseInputsV1ItemV0ContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BaseInputsV1ItemV0ContentUnion {
    Variant0(Vec<BaseInputsV1ItemV0ContentV0ItemUnion>),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for BaseInputsV1ItemV0ContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `BaseInputsV1ItemV0ContentV0ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BaseInputsV1ItemV0ContentV0ItemUnion {
    Variant0(OrInputText),
    Variant1(OrInputImage),
    Variant2(OrInputFile),
    Variant3(OrInputAudio),
    Unknown(serde_json::Value),
}
impl Default for BaseInputsV1ItemV0ContentV0ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `BaseInputsV1ItemV0PhaseUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BaseInputsV1ItemV0PhaseUnion {
    Variant0(BaseInputsV1ItemV0PhaseV0Enum),
    Variant1(BaseInputsV1ItemV0PhaseV1Enum),
    Unknown(serde_json::Value),
}
impl Default for BaseInputsV1ItemV0PhaseUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `BaseInputsV1ItemV0PhaseV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseInputsV1ItemV0PhaseV0Enum {
    #[serde(rename = "commentary")]
    Commentary,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for BaseInputsV1ItemV0PhaseV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `BaseInputsV1ItemV0PhaseV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseInputsV1ItemV0PhaseV1Enum {
    #[serde(rename = "final_answer")]
    FinalAnswer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for BaseInputsV1ItemV0PhaseV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `BaseInputsV1ItemV0RoleUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BaseInputsV1ItemV0RoleUnion {
    Variant0(BaseInputsV1ItemV0RoleV0Enum),
    Variant1(BaseInputsV1ItemV0RoleV1Enum),
    Variant2(BaseInputsV1ItemV0RoleV2Enum),
    Variant3(BaseInputsV1ItemV0RoleV3Enum),
    Unknown(serde_json::Value),
}
impl Default for BaseInputsV1ItemV0RoleUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `BaseInputsV1ItemV0RoleV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseInputsV1ItemV0RoleV0Enum {
    #[serde(rename = "user")]
    User,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for BaseInputsV1ItemV0RoleV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `BaseInputsV1ItemV0RoleV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseInputsV1ItemV0RoleV1Enum {
    #[serde(rename = "system")]
    System,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for BaseInputsV1ItemV0RoleV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `BaseInputsV1ItemV0RoleV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseInputsV1ItemV0RoleV2Enum {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for BaseInputsV1ItemV0RoleV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `BaseInputsV1ItemV0RoleV3Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseInputsV1ItemV0RoleV3Enum {
    #[serde(rename = "developer")]
    Developer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for BaseInputsV1ItemV0RoleV3Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `BaseInputsV1ItemV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseInputsV1ItemV0TypeEnum {
    #[serde(rename = "message")]
    Message,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for BaseInputsV1ItemV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Typed params for `POST /workspaces/{id}/members/add` (`bulkAddWorkspaceMembers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkAddWorkspaceMembersParams {
    pub id: String,
    pub body: OrBulkAddWorkspaceMembersRequest,
}

/// JSON result for `bulkAddWorkspaceMembers`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkAddWorkspaceMembersResult {
    #[serde(flatten)]
    pub body: OrBulkAddWorkspaceMembersResponse,
}

/// Typed params for `POST /guardrails/{id}/assignments/keys` (`bulkAssignKeysToGuardrail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkAssignKeysToGuardrailParams {
    pub id: String,
    pub body: OrBulkAssignKeysRequest,
}

/// JSON result for `bulkAssignKeysToGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkAssignKeysToGuardrailResult {
    #[serde(flatten)]
    pub body: OrBulkAssignKeysResponse,
}

/// Typed params for `POST /guardrails/{id}/assignments/members` (`bulkAssignMembersToGuardrail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkAssignMembersToGuardrailParams {
    pub id: String,
    pub body: OrBulkAssignMembersRequest,
}

/// JSON result for `bulkAssignMembersToGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkAssignMembersToGuardrailResult {
    #[serde(flatten)]
    pub body: OrBulkAssignMembersResponse,
}

/// Typed params for `POST /workspaces/{id}/members/remove` (`bulkRemoveWorkspaceMembers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkRemoveWorkspaceMembersParams {
    pub id: String,
    pub body: OrBulkRemoveWorkspaceMembersRequest,
}

/// JSON result for `bulkRemoveWorkspaceMembers`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkRemoveWorkspaceMembersResult {
    #[serde(flatten)]
    pub body: OrBulkRemoveWorkspaceMembersResponse,
}

/// Typed params for `POST /guardrails/{id}/assignments/keys/remove` (`bulkUnassignKeysFromGuardrail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkUnassignKeysFromGuardrailParams {
    pub id: String,
    pub body: OrBulkUnassignKeysRequest,
}

/// JSON result for `bulkUnassignKeysFromGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkUnassignKeysFromGuardrailResult {
    #[serde(flatten)]
    pub body: OrBulkUnassignKeysResponse,
}

/// Typed params for `POST /guardrails/{id}/assignments/members/remove` (`bulkUnassignMembersFromGuardrail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkUnassignMembersFromGuardrailParams {
    pub id: String,
    pub body: OrBulkUnassignMembersRequest,
}

/// JSON result for `bulkUnassignMembersFromGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BulkUnassignMembersFromGuardrailResult {
    #[serde(flatten)]
    pub body: OrBulkUnassignMembersResponse,
}

/// Generated object `ChatFunctionToolV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatFunctionToolV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrChatContentCacheControl>,
    pub function: ChatFunctionToolV0Function,
    #[serde(rename = "type")]
    pub type_: ChatFunctionToolV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ChatFunctionToolV0Function`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatFunctionToolV0Function {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ChatFunctionToolV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatFunctionToolV0TypeEnum {
    #[serde(rename = "function")]
    Function,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ChatFunctionToolV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `ChatToolChoiceV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatToolChoiceV0Enum {
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ChatToolChoiceV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `ChatToolChoiceV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatToolChoiceV1Enum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ChatToolChoiceV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `ChatToolChoiceV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatToolChoiceV2Enum {
    #[serde(rename = "required")]
    Required,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ChatToolChoiceV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Typed params for `POST /audio/speech` (`createAudioSpeech`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateAudioSpeechParams {
    pub body: OrSpeechRequest,
}

/// Binary result for `createAudioSpeech`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAudioSpeechResult {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

/// Typed params for `POST /audio/transcriptions` (`createAudioTranscriptions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateAudioTranscriptionsParams {
    pub body: OrSTTRequest,
}

/// JSON result for `createAudioTranscriptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateAudioTranscriptionsResult {
    #[serde(flatten)]
    pub body: OrSTTResponse,
}

/// Generated object `CreateAuthKeysCodeBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateAuthKeysCodeBody {
    pub callback_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_challenge: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_challenge_method: Option<CreateAuthKeysCodeBodyCodeChallengeMethodEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_cloud: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_limit_type: Option<CreateAuthKeysCodeBodyUsageLimitTypeEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CreateAuthKeysCodeBodyCodeChallengeMethodEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateAuthKeysCodeBodyCodeChallengeMethodEnum {
    #[serde(rename = "S256")]
    S256,
    #[serde(rename = "plain")]
    Plain,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateAuthKeysCodeBodyCodeChallengeMethodEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `CreateAuthKeysCodeBodyUsageLimitTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateAuthKeysCodeBodyUsageLimitTypeEnum {
    #[serde(rename = "daily")]
    Daily,
    #[serde(rename = "weekly")]
    Weekly,
    #[serde(rename = "monthly")]
    Monthly,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateAuthKeysCodeBodyUsageLimitTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Typed params for `POST /auth/keys/code` (`createAuthKeysCode`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateAuthKeysCodeParams {
    pub body: CreateAuthKeysCodeBody,
}

/// JSON result for `createAuthKeysCode`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateAuthKeysCodeResult {
    #[serde(flatten)]
    pub body: CreateAuthKeysCodeResultBody,
}

/// Generated object `CreateAuthKeysCodeResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateAuthKeysCodeResultBody {
    pub data: CreateAuthKeysCodeResultBodyData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateAuthKeysCodeResultBodyData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateAuthKeysCodeResultBodyData {
    pub app_id: i64,
    pub created_at: String,
    pub id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /byok` (`createBYOKKey`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateBYOKKeyParams {
    pub body: OrCreateBYOKKeyRequest,
}

/// JSON result for `createBYOKKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateBYOKKeyResult {
    #[serde(flatten)]
    pub body: OrCreateBYOKKeyResponse,
}

/// Typed params for `POST /credits/coinbase` (`createCoinbaseCharge`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateCoinbaseChargeParams {}

/// JSON result for `createCoinbaseCharge`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateCoinbaseChargeResult {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateEmbeddingsBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<CreateEmbeddingsBodyEncodingFormatEnum>,
    pub input: CreateEmbeddingsBodyInputUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<CreateEmbeddingsBodyProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CreateEmbeddingsBodyEncodingFormatEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateEmbeddingsBodyEncodingFormatEnum {
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "base64")]
    Base64,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateEmbeddingsBodyEncodingFormatEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `CreateEmbeddingsBodyInputUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateEmbeddingsBodyInputUnion {
    Variant0(String),
    Variant1(Vec<String>),
    Variant2(Vec<f64>),
    Variant3(Vec<Vec<f64>>),
    Variant4(Vec<CreateEmbeddingsBodyInputV4Item>),
    Unknown(serde_json::Value),
}
impl Default for CreateEmbeddingsBodyInputUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `CreateEmbeddingsBodyInputV4Item`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsBodyInputV4Item {
    pub content: Vec<CreateEmbeddingsBodyInputV4ItemContentItemUnion>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `CreateEmbeddingsBodyInputV4ItemContentItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateEmbeddingsBodyInputV4ItemContentItemUnion {
    Variant0(CreateEmbeddingsBodyInputV4ItemContentItemV0),
    Variant1(CreateEmbeddingsBodyInputV4ItemContentItemV1),
    Variant2(OrContentPartInputAudio),
    Variant3(OrContentPartInputVideo),
    Variant4(OrContentPartInputFile),
    Unknown(serde_json::Value),
}
impl Default for CreateEmbeddingsBodyInputV4ItemContentItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `CreateEmbeddingsBodyInputV4ItemContentItemV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsBodyInputV4ItemContentItemV0 {
    pub text: String,
    #[serde(rename = "type")]
    pub type_: CreateEmbeddingsBodyInputV4ItemContentItemV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CreateEmbeddingsBodyInputV4ItemContentItemV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateEmbeddingsBodyInputV4ItemContentItemV0TypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateEmbeddingsBodyInputV4ItemContentItemV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `CreateEmbeddingsBodyInputV4ItemContentItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsBodyInputV4ItemContentItemV1 {
    pub image_url: CreateEmbeddingsBodyInputV4ItemContentItemV1ImageUrl,
    #[serde(rename = "type")]
    pub type_: CreateEmbeddingsBodyInputV4ItemContentItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateEmbeddingsBodyInputV4ItemContentItemV1ImageUrl`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsBodyInputV4ItemContentItemV1ImageUrl {
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CreateEmbeddingsBodyInputV4ItemContentItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateEmbeddingsBodyInputV4ItemContentItemV1TypeEnum {
    #[serde(rename = "image_url")]
    ImageUrl,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateEmbeddingsBodyInputV4ItemContentItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `CreateEmbeddingsBodyProvider`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsBodyProvider {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<CreateEmbeddingsBodyProviderDataCollectionEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<CreateEmbeddingsBodyProviderIgnoreItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<CreateEmbeddingsBodyProviderMaxPrice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<CreateEmbeddingsBodyProviderOnlyItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<CreateEmbeddingsBodyProviderOrderItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_max_latency: Option<OrPreferredMaxLatency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_min_throughput: Option<OrPreferredMinThroughput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<OrQuantization>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<CreateEmbeddingsBodyProviderSortUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CreateEmbeddingsBodyProviderDataCollectionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateEmbeddingsBodyProviderDataCollectionEnum {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "allow")]
    Allow,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateEmbeddingsBodyProviderDataCollectionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `CreateEmbeddingsBodyProviderIgnoreItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateEmbeddingsBodyProviderIgnoreItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for CreateEmbeddingsBodyProviderIgnoreItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `CreateEmbeddingsBodyProviderMaxPrice`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsBodyProviderMaxPrice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `CreateEmbeddingsBodyProviderOnlyItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateEmbeddingsBodyProviderOnlyItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for CreateEmbeddingsBodyProviderOnlyItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `CreateEmbeddingsBodyProviderOrderItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateEmbeddingsBodyProviderOrderItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for CreateEmbeddingsBodyProviderOrderItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `CreateEmbeddingsBodyProviderSortUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateEmbeddingsBodyProviderSortUnion {
    Variant0(OrProviderSort),
    Variant1(OrProviderSortConfig),
    Unknown(serde_json::Value),
}
impl Default for CreateEmbeddingsBodyProviderSortUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Typed params for `POST /embeddings` (`createEmbeddings`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsParams {
    pub body: CreateEmbeddingsBody,
}

/// JSON result for `createEmbeddings`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsResult {
    #[serde(flatten)]
    pub body: CreateEmbeddingsResultBody,
}

/// Generated object `CreateEmbeddingsResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsResultBody {
    pub data: Vec<CreateEmbeddingsResultBodyDataItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub model: String,
    pub object: CreateEmbeddingsResultBodyObjectEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<CreateEmbeddingsResultBodyUsage>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateEmbeddingsResultBodyDataItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsResultBodyDataItem {
    pub embedding: CreateEmbeddingsResultBodyDataItemEmbeddingUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    pub object: CreateEmbeddingsResultBodyDataItemObjectEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `CreateEmbeddingsResultBodyDataItemEmbeddingUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateEmbeddingsResultBodyDataItemEmbeddingUnion {
    Variant0(Vec<f64>),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for CreateEmbeddingsResultBodyDataItemEmbeddingUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `CreateEmbeddingsResultBodyDataItemObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateEmbeddingsResultBodyDataItemObjectEnum {
    #[serde(rename = "embedding")]
    Embedding,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateEmbeddingsResultBodyDataItemObjectEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `CreateEmbeddingsResultBodyObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateEmbeddingsResultBodyObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateEmbeddingsResultBodyObjectEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `CreateEmbeddingsResultBodyUsage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsResultBodyUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_details: Option<OrCostDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_byok: Option<bool>,
    pub prompt_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<CreateEmbeddingsResultBodyUsagePromptTokensDetails>,
    pub total_tokens: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateEmbeddingsResultBodyUsagePromptTokensDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsResultBodyUsagePromptTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// SSE event stream for `createEmbeddings` (all frames preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingsSseResult {
    pub events: Vec<SseEvent>,
}

/// Typed params for `POST /guardrails` (`createGuardrail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateGuardrailParams {
    pub body: OrCreateGuardrailRequest,
}

/// JSON result for `createGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateGuardrailResult {
    #[serde(flatten)]
    pub body: OrCreateGuardrailResponse,
}

/// Typed params for `POST /images` (`createImages`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateImagesParams {
    pub body: OrImageGenerationRequest,
}

/// JSON result for `createImages`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateImagesResult {
    #[serde(flatten)]
    pub body: OrImageGenerationResponse,
}

/// SSE event stream for `createImages` (all frames preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateImagesSseResult {
    pub events: Vec<SseEvent>,
}

/// Generated object `CreateKeysBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateKeysBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_byok_in_limit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reset: Option<CreateKeysBodyLimitResetEnum>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CreateKeysBodyLimitResetEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateKeysBodyLimitResetEnum {
    #[serde(rename = "daily")]
    Daily,
    #[serde(rename = "weekly")]
    Weekly,
    #[serde(rename = "monthly")]
    Monthly,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateKeysBodyLimitResetEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Typed params for `POST /keys` (`createKeys`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateKeysParams {
    pub body: CreateKeysBody,
}

/// JSON result for `createKeys`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateKeysResult {
    #[serde(flatten)]
    pub body: CreateKeysResultBody,
}

/// Generated object `CreateKeysResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateKeysResultBody {
    pub data: CreateKeysResultBodyData,
    pub key: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateKeysResultBodyData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateKeysResultBodyData {
    pub byok_usage: f64,
    pub byok_usage_daily: f64,
    pub byok_usage_monthly: f64,
    pub byok_usage_weekly: f64,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_user_id: Option<String>,
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub hash: String,
    pub include_byok_in_limit: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_remaining: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reset: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub usage: f64,
    pub usage_daily: f64,
    pub usage_monthly: f64,
    pub usage_weekly: f64,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /messages` (`createMessages`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateMessagesParams {
    pub body: OrMessagesRequest,
}

/// JSON result for `createMessages`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateMessagesResult {
    #[serde(flatten)]
    pub body: OrMessagesResult,
}

/// SSE event stream for `createMessages` (all frames preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateMessagesSseResult {
    pub events: Vec<SseEvent>,
}

/// Typed params for `POST /observability/destinations` (`createObservabilityDestination`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateObservabilityDestinationParams {
    pub body: OrCreateObservabilityDestinationRequest,
}

/// JSON result for `createObservabilityDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateObservabilityDestinationResult {
    #[serde(flatten)]
    pub body: OrCreateObservabilityDestinationResponse,
}

/// Typed params for `POST /presets/{slug}/chat/completions` (`createPresetsChatCompletions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatePresetsChatCompletionsParams {
    pub slug: String,
    pub body: OrChatRequest,
}

/// JSON result for `createPresetsChatCompletions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatePresetsChatCompletionsResult {
    #[serde(flatten)]
    pub body: OrCreatePresetFromInferenceResponse,
}

/// Typed params for `POST /presets/{slug}/messages` (`createPresetsMessages`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatePresetsMessagesParams {
    pub slug: String,
    pub body: OrMessagesRequest,
}

/// JSON result for `createPresetsMessages`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatePresetsMessagesResult {
    #[serde(flatten)]
    pub body: OrCreatePresetFromInferenceResponse,
}

/// Typed params for `POST /presets/{slug}/responses` (`createPresetsResponses`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatePresetsResponsesParams {
    pub slug: String,
    pub body: OrResponsesRequest,
}

/// JSON result for `createPresetsResponses`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatePresetsResponsesResult {
    #[serde(flatten)]
    pub body: OrCreatePresetFromInferenceResponse,
}

/// Generated object `CreateRerankBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankBody {
    pub documents: Vec<CreateRerankBodyDocumentsItemUnion>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<CreateRerankBodyProvider>,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_n: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `CreateRerankBodyDocumentsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateRerankBodyDocumentsItemUnion {
    Variant0(String),
    Variant1(CreateRerankBodyDocumentsItemV1),
    Unknown(serde_json::Value),
}
impl Default for CreateRerankBodyDocumentsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `CreateRerankBodyDocumentsItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankBodyDocumentsItemV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateRerankBodyProvider`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankBodyProvider {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<CreateRerankBodyProviderDataCollectionEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<CreateRerankBodyProviderIgnoreItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<CreateRerankBodyProviderMaxPrice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<CreateRerankBodyProviderOnlyItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<CreateRerankBodyProviderOrderItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_max_latency: Option<OrPreferredMaxLatency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_min_throughput: Option<OrPreferredMinThroughput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<OrQuantization>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<CreateRerankBodyProviderSortUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CreateRerankBodyProviderDataCollectionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateRerankBodyProviderDataCollectionEnum {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "allow")]
    Allow,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for CreateRerankBodyProviderDataCollectionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `CreateRerankBodyProviderIgnoreItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateRerankBodyProviderIgnoreItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for CreateRerankBodyProviderIgnoreItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `CreateRerankBodyProviderMaxPrice`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankBodyProviderMaxPrice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `CreateRerankBodyProviderOnlyItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateRerankBodyProviderOnlyItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for CreateRerankBodyProviderOnlyItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `CreateRerankBodyProviderOrderItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateRerankBodyProviderOrderItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for CreateRerankBodyProviderOrderItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `CreateRerankBodyProviderSortUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateRerankBodyProviderSortUnion {
    Variant0(OrProviderSort),
    Variant1(OrProviderSortConfig),
    Unknown(serde_json::Value),
}
impl Default for CreateRerankBodyProviderSortUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Typed params for `POST /rerank` (`createRerank`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankParams {
    pub body: CreateRerankBody,
}

/// JSON result for `createRerank`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankResult {
    #[serde(flatten)]
    pub body: CreateRerankResultBody,
}

/// Generated object `CreateRerankResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankResultBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub results: Vec<CreateRerankResultBodyResultsItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<CreateRerankResultBodyUsage>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateRerankResultBodyResultsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankResultBodyResultsItem {
    pub document: CreateRerankResultBodyResultsItemDocument,
    pub index: i64,
    pub relevance_score: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateRerankResultBodyResultsItemDocument`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankResultBodyResultsItemDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CreateRerankResultBodyUsage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankResultBodyUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_units: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// SSE event stream for `createRerank` (all frames preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateRerankSseResult {
    pub events: Vec<SseEvent>,
}

/// Typed params for `POST /responses` (`createResponses`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateResponsesParams {
    pub body: OrResponsesRequest,
}

/// JSON result for `createResponses`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateResponsesResult {
    #[serde(flatten)]
    pub body: OrOpenResponsesResult,
}

/// SSE event stream for `createResponses` (all frames preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateResponsesSseResult {
    pub events: Vec<SseEvent>,
}

/// Typed params for `POST /videos` (`createVideos`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateVideosParams {
    pub body: OrVideoGenerationRequest,
}

/// JSON result for `createVideos`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateVideosResult {
    #[serde(flatten)]
    pub body: OrVideoGenerationResponse,
}

/// Typed params for `POST /workspaces` (`createWorkspace`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateWorkspaceParams {
    pub body: OrCreateWorkspaceRequest,
}

/// JSON result for `createWorkspace`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateWorkspaceResult {
    #[serde(flatten)]
    pub body: OrCreateWorkspaceResponse,
}

/// Typed params for `DELETE /byok/{id}` (`deleteBYOKKey`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteBYOKKeyParams {
    pub id: String,
}

/// JSON result for `deleteBYOKKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteBYOKKeyResult {
    #[serde(flatten)]
    pub body: OrDeleteBYOKKeyResponse,
}

/// Typed params for `DELETE /files/{file_id}` (`deleteFile`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteFileParams {
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// JSON result for `deleteFile`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteFileResult {
    #[serde(flatten)]
    pub body: OrFileDeleteResponse,
}

/// Typed params for `DELETE /guardrails/{id}` (`deleteGuardrail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteGuardrailParams {
    pub id: String,
}

/// JSON result for `deleteGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteGuardrailResult {
    #[serde(flatten)]
    pub body: OrDeleteGuardrailResponse,
}

/// Typed params for `DELETE /keys/{hash}` (`deleteKeys`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteKeysParams {
    pub hash: String,
}

/// JSON result for `deleteKeys`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteKeysResult {
    #[serde(flatten)]
    pub body: DeleteKeysResultBody,
}

/// Generated object `DeleteKeysResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteKeysResultBody {
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `DELETE /observability/destinations/{id}` (`deleteObservabilityDestination`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteObservabilityDestinationParams {
    pub id: String,
}

/// JSON result for `deleteObservabilityDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteObservabilityDestinationResult {
    #[serde(flatten)]
    pub body: OrDeleteObservabilityDestinationResponse,
}

/// Typed params for `DELETE /workspaces/{id}/budgets/{interval}` (`deleteWorkspaceBudget`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteWorkspaceBudgetParams {
    pub id: String,
    pub interval: String,
}

/// JSON result for `deleteWorkspaceBudget`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteWorkspaceBudgetResult {
    #[serde(flatten)]
    pub body: OrDeleteWorkspaceBudgetResponse,
}

/// Typed params for `DELETE /workspaces/{id}` (`deleteWorkspace`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteWorkspaceParams {
    pub id: String,
}

/// JSON result for `deleteWorkspace`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteWorkspaceResult {
    #[serde(flatten)]
    pub body: OrDeleteWorkspaceResponse,
}

/// Typed params for `GET /files/{file_id}/content` (`downloadFileContent`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DownloadFileContentParams {
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// Binary result for `downloadFileContent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadFileContentResult {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

/// Generated object `ExchangeAuthCodeForAPIKeyBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExchangeAuthCodeForAPIKeyBody {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_challenge_method: Option<ExchangeAuthCodeForAPIKeyBodyCodeChallengeMethodEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_verifier: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ExchangeAuthCodeForAPIKeyBodyCodeChallengeMethodEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExchangeAuthCodeForAPIKeyBodyCodeChallengeMethodEnum {
    #[serde(rename = "S256")]
    S256,
    #[serde(rename = "plain")]
    Plain,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ExchangeAuthCodeForAPIKeyBodyCodeChallengeMethodEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Typed params for `POST /auth/keys` (`exchangeAuthCodeForAPIKey`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExchangeAuthCodeForAPIKeyParams {
    pub body: ExchangeAuthCodeForAPIKeyBody,
}

/// JSON result for `exchangeAuthCodeForAPIKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExchangeAuthCodeForAPIKeyResult {
    #[serde(flatten)]
    pub body: ExchangeAuthCodeForAPIKeyResultBody,
}

/// Generated object `ExchangeAuthCodeForAPIKeyResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExchangeAuthCodeForAPIKeyResultBody {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `GET /analytics/meta` (`getAnalyticsMeta`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAnalyticsMetaParams {}

/// JSON result for `getAnalyticsMeta`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAnalyticsMetaResult {
    #[serde(flatten)]
    pub body: GetAnalyticsMetaResultBody,
}

/// Generated object `GetAnalyticsMetaResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAnalyticsMetaResultBody {
    pub data: GetAnalyticsMetaResultBodyData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GetAnalyticsMetaResultBodyData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAnalyticsMetaResultBodyData {
    pub dimensions: Vec<GetAnalyticsMetaResultBodyDataDimensionsItem>,
    pub granularities: Vec<GetAnalyticsMetaResultBodyDataGranularitiesItem>,
    pub metrics: Vec<GetAnalyticsMetaResultBodyDataMetricsItem>,
    pub operators: Vec<GetAnalyticsMetaResultBodyDataOperatorsItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GetAnalyticsMetaResultBodyDataDimensionsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAnalyticsMetaResultBodyDataDimensionsItem {
    pub display_label: String,
    pub name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GetAnalyticsMetaResultBodyDataGranularitiesItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAnalyticsMetaResultBodyDataGranularitiesItem {
    pub display_label: String,
    pub name: GetAnalyticsMetaResultBodyDataGranularitiesItemNameEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GetAnalyticsMetaResultBodyDataGranularitiesItemNameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetAnalyticsMetaResultBodyDataGranularitiesItemNameEnum {
    #[serde(rename = "minute")]
    Minute,
    #[serde(rename = "hour")]
    Hour,
    #[serde(rename = "day")]
    Day,
    #[serde(rename = "week")]
    Week,
    #[serde(rename = "month")]
    Month,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetAnalyticsMetaResultBodyDataGranularitiesItemNameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `GetAnalyticsMetaResultBodyDataMetricsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAnalyticsMetaResultBodyDataMetricsItem {
    pub display_format: GetAnalyticsMetaResultBodyDataMetricsItemDisplayFormatEnum,
    pub display_label: String,
    pub is_rate: bool,
    pub name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GetAnalyticsMetaResultBodyDataMetricsItemDisplayFormatEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetAnalyticsMetaResultBodyDataMetricsItemDisplayFormatEnum {
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "currency")]
    Currency,
    #[serde(rename = "percent")]
    Percent,
    #[serde(rename = "latency")]
    Latency,
    #[serde(rename = "throughput")]
    Throughput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetAnalyticsMetaResultBodyDataMetricsItemDisplayFormatEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `GetAnalyticsMetaResultBodyDataOperatorsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAnalyticsMetaResultBodyDataOperatorsItem {
    pub name: GetAnalyticsMetaResultBodyDataOperatorsItemNameEnum,
    pub value_type: GetAnalyticsMetaResultBodyDataOperatorsItemValueTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GetAnalyticsMetaResultBodyDataOperatorsItemNameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetAnalyticsMetaResultBodyDataOperatorsItemNameEnum {
    #[serde(rename = "eq")]
    Eq,
    #[serde(rename = "neq")]
    Neq,
    #[serde(rename = "in")]
    In,
    #[serde(rename = "not_in")]
    NotIn,
    #[serde(rename = "gt")]
    Gt,
    #[serde(rename = "gte")]
    Gte,
    #[serde(rename = "lt")]
    Lt,
    #[serde(rename = "lte")]
    Lte,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetAnalyticsMetaResultBodyDataOperatorsItemNameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetAnalyticsMetaResultBodyDataOperatorsItemValueTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetAnalyticsMetaResultBodyDataOperatorsItemValueTypeEnum {
    #[serde(rename = "scalar")]
    Scalar,
    #[serde(rename = "array")]
    Array,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetAnalyticsMetaResultBodyDataOperatorsItemValueTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Typed params for `GET /datasets/app-rankings` (`getAppRankings`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAppRankingsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<GetAppRankingsParamsCategoryEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subcategory: Option<GetAppRankingsParamsSubcategoryEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<GetAppRankingsParamsSortEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

/// Generated string enum `GetAppRankingsParamsCategoryEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetAppRankingsParamsCategoryEnum {
    #[serde(rename = "coding")]
    Coding,
    #[serde(rename = "creative")]
    Creative,
    #[serde(rename = "productivity")]
    Productivity,
    #[serde(rename = "entertainment")]
    Entertainment,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetAppRankingsParamsCategoryEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetAppRankingsParamsSortEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetAppRankingsParamsSortEnum {
    #[serde(rename = "popular")]
    Popular,
    #[serde(rename = "trending")]
    Trending,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetAppRankingsParamsSortEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetAppRankingsParamsSubcategoryEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetAppRankingsParamsSubcategoryEnum {
    #[serde(rename = "cli-agent")]
    CliAgent,
    #[serde(rename = "ide-extension")]
    IdeExtension,
    #[serde(rename = "cloud-agent")]
    CloudAgent,
    #[serde(rename = "programming-app")]
    ProgrammingApp,
    #[serde(rename = "native-app-builder")]
    NativeAppBuilder,
    #[serde(rename = "creative-writing")]
    CreativeWriting,
    #[serde(rename = "video-gen")]
    VideoGen,
    #[serde(rename = "image-gen")]
    ImageGen,
    #[serde(rename = "audio-gen")]
    AudioGen,
    #[serde(rename = "roleplay")]
    Roleplay,
    #[serde(rename = "game")]
    Game,
    #[serde(rename = "writing-assistant")]
    WritingAssistant,
    #[serde(rename = "general-chat")]
    GeneralChat,
    #[serde(rename = "personal-agent")]
    PersonalAgent,
    #[serde(rename = "legal")]
    Legal,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetAppRankingsParamsSubcategoryEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// JSON result for `getAppRankings`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetAppRankingsResult {
    #[serde(flatten)]
    pub body: OrAppRankingsResponse,
}

/// Typed params for `GET /byok/{id}` (`getBYOKKey`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetBYOKKeyParams {
    pub id: String,
}

/// JSON result for `getBYOKKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetBYOKKeyResult {
    #[serde(flatten)]
    pub body: OrGetBYOKKeyResponse,
}

/// Typed params for `GET /benchmarks` (`getBenchmarks`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetBenchmarksParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<GetBenchmarksParamsSourceEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<GetBenchmarksParamsTaskTypeEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arena: Option<GetBenchmarksParamsArenaEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
}

/// Generated string enum `GetBenchmarksParamsArenaEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetBenchmarksParamsArenaEnum {
    #[serde(rename = "models")]
    Models,
    #[serde(rename = "builders")]
    Builders,
    #[serde(rename = "agents")]
    Agents,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetBenchmarksParamsArenaEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetBenchmarksParamsSourceEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetBenchmarksParamsSourceEnum {
    #[serde(rename = "artificial-analysis")]
    ArtificialAnalysis,
    #[serde(rename = "design-arena")]
    DesignArena,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetBenchmarksParamsSourceEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetBenchmarksParamsTaskTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetBenchmarksParamsTaskTypeEnum {
    #[serde(rename = "coding")]
    Coding,
    #[serde(rename = "intelligence")]
    Intelligence,
    #[serde(rename = "agentic")]
    Agentic,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetBenchmarksParamsTaskTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// JSON result for `getBenchmarks`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetBenchmarksResult {
    #[serde(flatten)]
    pub body: OrUnifiedBenchmarksResponse,
}

/// Typed params for `GET /credits` (`getCredits`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCreditsParams {}

/// JSON result for `getCredits`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCreditsResult {
    #[serde(flatten)]
    pub body: GetCreditsResultBody,
}

/// Generated object `GetCreditsResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCreditsResultBody {
    pub data: GetCreditsResultBodyData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GetCreditsResultBodyData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCreditsResultBodyData {
    pub total_credits: f64,
    pub total_usage: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `GET /key` (`getCurrentKey`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCurrentKeyParams {}

/// JSON result for `getCurrentKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCurrentKeyResult {
    #[serde(flatten)]
    pub body: GetCurrentKeyResultBody,
}

/// Generated object `GetCurrentKeyResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCurrentKeyResultBody {
    pub data: GetCurrentKeyResultBodyData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GetCurrentKeyResultBodyData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCurrentKeyResultBodyData {
    pub byok_usage: f64,
    pub byok_usage_daily: f64,
    pub byok_usage_monthly: f64,
    pub byok_usage_weekly: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub include_byok_in_limit: bool,
    pub is_free_tier: bool,
    pub is_management_key: bool,
    pub is_provisioning_key: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_remaining: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reset: Option<String>,
    pub rate_limit: GetCurrentKeyResultBodyDataRateLimit,
    pub usage: f64,
    pub usage_daily: f64,
    pub usage_monthly: f64,
    pub usage_weekly: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GetCurrentKeyResultBodyDataRateLimit`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCurrentKeyResultBodyDataRateLimit {
    pub interval: String,
    pub note: String,
    pub requests: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `GET /files/{file_id}` (`getFileMetadata`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetFileMetadataParams {
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// JSON result for `getFileMetadata`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetFileMetadataResult {
    #[serde(flatten)]
    pub body: OrFileMetadata,
}

/// Typed params for `GET /generation` (`getGeneration`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetGenerationParams {
    pub id: String,
}

/// JSON result for `getGeneration`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetGenerationResult {
    #[serde(flatten)]
    pub body: OrGenerationResponse,
}

/// Typed params for `GET /guardrails/{id}` (`getGuardrail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetGuardrailParams {
    pub id: String,
}

/// JSON result for `getGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetGuardrailResult {
    #[serde(flatten)]
    pub body: OrGetGuardrailResponse,
}

/// Typed params for `GET /keys/{hash}` (`getKey`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetKeyParams {
    pub hash: String,
}

/// JSON result for `getKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetKeyResult {
    #[serde(flatten)]
    pub body: GetKeyResultBody,
}

/// Generated object `GetKeyResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetKeyResultBody {
    pub data: GetKeyResultBodyData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GetKeyResultBodyData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetKeyResultBodyData {
    pub byok_usage: f64,
    pub byok_usage_daily: f64,
    pub byok_usage_monthly: f64,
    pub byok_usage_weekly: f64,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_user_id: Option<String>,
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub hash: String,
    pub include_byok_in_limit: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_remaining: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reset: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub usage: f64,
    pub usage_daily: f64,
    pub usage_monthly: f64,
    pub usage_weekly: f64,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `GET /model/{author}/{slug}` (`getModel`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetModelParams {
    pub author: String,
    pub slug: String,
}

/// JSON result for `getModel`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetModelResult {
    #[serde(flatten)]
    pub body: OrModelResponse,
}

/// Typed params for `GET /models` (`getModels`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetModelsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<GetModelsParamsCategoryEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_parameters: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<GetModelsParamsSortEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_modalities: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_authors: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub providers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distillable: Option<GetModelsParamsDistillableEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zdr: Option<GetModelsParamsZdrEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<GetModelsParamsRegionEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_output_price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_age_days: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age_days: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_intelligence_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_intelligence_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_coding_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_coding_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_agentic_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_agentic_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_tool_success_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_success_rate: Option<f64>,
}

/// Generated string enum `GetModelsParamsCategoryEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetModelsParamsCategoryEnum {
    #[serde(rename = "programming")]
    Programming,
    #[serde(rename = "roleplay")]
    Roleplay,
    #[serde(rename = "marketing")]
    Marketing,
    #[serde(rename = "marketing/seo")]
    MarketingSeo,
    #[serde(rename = "technology")]
    Technology,
    #[serde(rename = "science")]
    Science,
    #[serde(rename = "translation")]
    Translation,
    #[serde(rename = "legal")]
    Legal,
    #[serde(rename = "finance")]
    Finance,
    #[serde(rename = "health")]
    Health,
    #[serde(rename = "trivia")]
    Trivia,
    #[serde(rename = "academia")]
    Academia,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetModelsParamsCategoryEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetModelsParamsDistillableEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetModelsParamsDistillableEnum {
    #[serde(rename = "true")]
    True,
    #[serde(rename = "false")]
    False,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetModelsParamsDistillableEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetModelsParamsRegionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetModelsParamsRegionEnum {
    #[serde(rename = "eu")]
    Eu,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetModelsParamsRegionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetModelsParamsSortEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetModelsParamsSortEnum {
    #[serde(rename = "most-popular")]
    MostPopular,
    #[serde(rename = "newest")]
    Newest,
    #[serde(rename = "top-weekly")]
    TopWeekly,
    #[serde(rename = "pricing-low-to-high")]
    PricingLowToHigh,
    #[serde(rename = "pricing-high-to-low")]
    PricingHighToLow,
    #[serde(rename = "context-high-to-low")]
    ContextHighToLow,
    #[serde(rename = "throughput-high-to-low")]
    ThroughputHighToLow,
    #[serde(rename = "latency-low-to-high")]
    LatencyLowToHigh,
    #[serde(rename = "intelligence-high-to-low")]
    IntelligenceHighToLow,
    #[serde(rename = "coding-high-to-low")]
    CodingHighToLow,
    #[serde(rename = "agentic-high-to-low")]
    AgenticHighToLow,
    #[serde(rename = "design-arena-elo-high-to-low")]
    DesignArenaEloHighToLow,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetModelsParamsSortEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetModelsParamsZdrEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetModelsParamsZdrEnum {
    #[serde(rename = "true")]
    True,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetModelsParamsZdrEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// JSON result for `getModels`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetModelsResult {
    #[serde(flatten)]
    pub body: OrModelsListResponse,
}

/// Typed params for `GET /observability/destinations/{id}` (`getObservabilityDestination`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetObservabilityDestinationParams {
    pub id: String,
}

/// JSON result for `getObservabilityDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetObservabilityDestinationResult {
    #[serde(flatten)]
    pub body: OrGetObservabilityDestinationResponse,
}

/// Typed params for `GET /presets/{slug}` (`getPreset`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetPresetParams {
    pub slug: String,
}

/// JSON result for `getPreset`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetPresetResult {
    #[serde(flatten)]
    pub body: OrGetPresetResponse,
}

/// Typed params for `GET /presets/{slug}/versions/{version}` (`getPresetVersion`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetPresetVersionParams {
    pub slug: String,
    pub version: String,
}

/// JSON result for `getPresetVersion`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetPresetVersionResult {
    #[serde(flatten)]
    pub body: OrGetPresetVersionResponse,
}

/// Typed params for `GET /datasets/rankings-daily` (`getRankingsDaily`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetRankingsDailyParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<GetRankingsDailyParamsPeriodEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modality: Option<GetRankingsDailyParamsModalityEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_bucket: Option<GetRankingsDailyParamsContextBucketEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<GetRankingsDailyParamsCategoryEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_type: Option<GetRankingsDailyParamsLanguageTypeEnum>,
}

/// Generated string enum `GetRankingsDailyParamsCategoryEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetRankingsDailyParamsCategoryEnum {
    #[serde(rename = "programming")]
    Programming,
    #[serde(rename = "roleplay")]
    Roleplay,
    #[serde(rename = "marketing")]
    Marketing,
    #[serde(rename = "marketing/seo")]
    MarketingSeo,
    #[serde(rename = "technology")]
    Technology,
    #[serde(rename = "science")]
    Science,
    #[serde(rename = "translation")]
    Translation,
    #[serde(rename = "legal")]
    Legal,
    #[serde(rename = "finance")]
    Finance,
    #[serde(rename = "health")]
    Health,
    #[serde(rename = "trivia")]
    Trivia,
    #[serde(rename = "academia")]
    Academia,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetRankingsDailyParamsCategoryEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetRankingsDailyParamsContextBucketEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetRankingsDailyParamsContextBucketEnum {
    #[serde(rename = "1K")]
    T1K,
    #[serde(rename = "10K")]
    T10K,
    #[serde(rename = "100K")]
    T100K,
    #[serde(rename = "1M")]
    T1M,
    #[serde(rename = "10M")]
    T10M,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetRankingsDailyParamsContextBucketEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetRankingsDailyParamsLanguageTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetRankingsDailyParamsLanguageTypeEnum {
    #[serde(rename = "natural")]
    Natural,
    #[serde(rename = "programming")]
    Programming,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetRankingsDailyParamsLanguageTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetRankingsDailyParamsModalityEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetRankingsDailyParamsModalityEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "image_output")]
    ImageOutput,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "tool_calling")]
    ToolCalling,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetRankingsDailyParamsModalityEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `GetRankingsDailyParamsPeriodEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetRankingsDailyParamsPeriodEnum {
    #[serde(rename = "day")]
    Day,
    #[serde(rename = "week")]
    Week,
    #[serde(rename = "month")]
    Month,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetRankingsDailyParamsPeriodEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// JSON result for `getRankingsDaily`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetRankingsDailyResult {
    #[serde(flatten)]
    pub body: OrRankingsDailyResponse,
}

/// Typed params for `GET /classifications/task` (`getTaskClassifications`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetTaskClassificationsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<GetTaskClassificationsParamsWindowEnum>,
}

/// Generated string enum `GetTaskClassificationsParamsWindowEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetTaskClassificationsParamsWindowEnum {
    #[serde(rename = "7d")]
    T7d,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for GetTaskClassificationsParamsWindowEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// JSON result for `getTaskClassifications`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetTaskClassificationsResult {
    #[serde(flatten)]
    pub body: OrTaskClassificationResponse,
}

/// Typed params for `GET /activity` (`getUserActivity`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetUserActivityParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// JSON result for `getUserActivity`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetUserActivityResult {
    #[serde(flatten)]
    pub body: OrActivityResponse,
}

/// Typed params for `GET /videos/{jobId}` (`getVideos`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetVideosParams {
    pub job_id: String,
}

/// JSON result for `getVideos`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetVideosResult {
    #[serde(flatten)]
    pub body: OrVideoGenerationResponse,
}

/// Typed params for `GET /workspaces/{id}` (`getWorkspace`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetWorkspaceParams {
    pub id: String,
}

/// JSON result for `getWorkspace`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetWorkspaceResult {
    #[serde(flatten)]
    pub body: OrGetWorkspaceResponse,
}

/// Generated union `InputsV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputsV1ItemUnion {
    Variant0(OrReasoningItem),
    Variant1(OrEasyInputMessage),
    Variant2(OrInputMessageItem),
    Variant3(OrFunctionCallItem),
    Variant4(OrFunctionCallOutputItem),
    Variant5(OrApplyPatchCallItem),
    Variant6(OrApplyPatchCallOutputItem),
    Variant7(InputsV1ItemV7),
    Variant8(InputsV1ItemV8),
    Variant9(OrOutputFunctionCallItem),
    Variant10(OrOutputCustomToolCallItem),
    Variant11(OrOutputWebSearchCallItem),
    Variant12(OrOutputFileSearchCallItem),
    Variant13(OrOutputImageGenerationCallItem),
    Variant14(OrOutputCodeInterpreterCallItem),
    Variant15(OrOutputComputerCallItem),
    Variant16(OrOutputDatetimeItem),
    Variant17(OrOutputWebSearchServerToolItem),
    Variant18(OrOutputCodeInterpreterServerToolItem),
    Variant19(OrOutputFileSearchServerToolItem),
    Variant20(OrOutputImageGenerationServerToolItem),
    Variant21(OrOutputBrowserUseServerToolItem),
    Variant22(OrOutputBashServerToolItem),
    Variant23(OrOutputTextEditorServerToolItem),
    Variant24(OrOutputApplyPatchServerToolItem),
    Variant25(OrOutputWebFetchServerToolItem),
    Variant26(OrOutputToolSearchServerToolItem),
    Variant27(OrOutputMemoryServerToolItem),
    Variant28(OrOutputMcpServerToolItem),
    Variant29(OrOutputSearchModelsServerToolItem),
    Variant30(OrOutputFusionServerToolItem),
    Variant31(OrOutputAdvisorServerToolItem),
    Variant32(OrOutputSubagentServerToolItem),
    Variant33(OrOutputFilesServerToolItem),
    Variant34(OrLocalShellCallItem),
    Variant35(OrLocalShellCallOutputItem),
    Variant36(OrShellCallItem),
    Variant37(OrShellCallOutputItem),
    Variant38(OrMcpListToolsItem),
    Variant39(OrMcpApprovalRequestItem),
    Variant40(OrMcpApprovalResponseItem),
    Variant41(OrMcpCallItem),
    Variant42(OrCustomToolCallItem),
    Variant43(OrCustomToolCallOutputItem),
    Variant44(OrCompactionItem),
    Variant45(OrContextCompactionItem),
    Variant46(OrItemReferenceItem),
    Variant47(OrAdditionalToolsItem),
    Variant48(OrAgentMessageItem),
    Unknown(serde_json::Value),
}
impl Default for InputsV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `InputsV1ItemV7`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InputsV1ItemV7 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<InputsV1ItemV7ContentUnion>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `InputsV1ItemV7ContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputsV1ItemV7ContentUnion {
    Variant0(Vec<InputsV1ItemV7ContentV0ItemUnion>),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for InputsV1ItemV7ContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `InputsV1ItemV7ContentV0ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputsV1ItemV7ContentV0ItemUnion {
    Variant0(OrResponseOutputText),
    Variant1(OrOpenAIResponsesRefusalContent),
    Unknown(serde_json::Value),
}
impl Default for InputsV1ItemV7ContentV0ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `InputsV1ItemV8`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InputsV1ItemV8 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Vec<OrReasoningSummaryText>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `GET /byok` (`listBYOKKeys`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListBYOKKeysParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ListBYOKKeysParamsProviderEnum>,
}

/// Generated string enum `ListBYOKKeysParamsProviderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListBYOKKeysParamsProviderEnum {
    #[serde(rename = "ai21")]
    Ai21,
    #[serde(rename = "aion-labs")]
    AionLabs,
    #[serde(rename = "akashml")]
    Akashml,
    #[serde(rename = "alibaba")]
    Alibaba,
    #[serde(rename = "amazon-bedrock")]
    AmazonBedrock,
    #[serde(rename = "amazon-nova")]
    AmazonNova,
    #[serde(rename = "ambient")]
    Ambient,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "arcee-ai")]
    ArceeAi,
    #[serde(rename = "atlas-cloud")]
    AtlasCloud,
    #[serde(rename = "avian")]
    Avian,
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "baidu")]
    Baidu,
    #[serde(rename = "baseten")]
    Baseten,
    #[serde(rename = "black-forest-labs")]
    BlackForestLabs,
    #[serde(rename = "byteplus")]
    Byteplus,
    #[serde(rename = "cerebras")]
    Cerebras,
    #[serde(rename = "chutes")]
    Chutes,
    #[serde(rename = "cirrascale")]
    Cirrascale,
    #[serde(rename = "clarifai")]
    Clarifai,
    #[serde(rename = "cloudflare")]
    Cloudflare,
    #[serde(rename = "cohere")]
    Cohere,
    #[serde(rename = "coreweave")]
    Coreweave,
    #[serde(rename = "crusoe")]
    Crusoe,
    #[serde(rename = "darkbloom")]
    Darkbloom,
    #[serde(rename = "decart")]
    Decart,
    #[serde(rename = "deepgram")]
    Deepgram,
    #[serde(rename = "deepinfra")]
    Deepinfra,
    #[serde(rename = "deepseek")]
    Deepseek,
    #[serde(rename = "dekallm")]
    Dekallm,
    #[serde(rename = "digitalocean")]
    Digitalocean,
    #[serde(rename = "featherless")]
    Featherless,
    #[serde(rename = "fireworks")]
    Fireworks,
    #[serde(rename = "fish-audio")]
    FishAudio,
    #[serde(rename = "friendli")]
    Friendli,
    #[serde(rename = "gmicloud")]
    Gmicloud,
    #[serde(rename = "google-ai-studio")]
    GoogleAiStudio,
    #[serde(rename = "google-vertex")]
    GoogleVertex,
    #[serde(rename = "groq")]
    Groq,
    #[serde(rename = "heygen")]
    Heygen,
    #[serde(rename = "inception")]
    Inception,
    #[serde(rename = "inceptron")]
    Inceptron,
    #[serde(rename = "inferact-vllm")]
    InferactVllm,
    #[serde(rename = "inference-net")]
    InferenceNet,
    #[serde(rename = "infermatic")]
    Infermatic,
    #[serde(rename = "inflection")]
    Inflection,
    #[serde(rename = "io-net")]
    IoNet,
    #[serde(rename = "ionstream")]
    Ionstream,
    #[serde(rename = "krea")]
    Krea,
    #[serde(rename = "liquid")]
    Liquid,
    #[serde(rename = "mancer")]
    Mancer,
    #[serde(rename = "mara")]
    Mara,
    #[serde(rename = "meta")]
    Meta,
    #[serde(rename = "minimax")]
    Minimax,
    #[serde(rename = "mistral")]
    Mistral,
    #[serde(rename = "modelrun")]
    Modelrun,
    #[serde(rename = "modular")]
    Modular,
    #[serde(rename = "moonshotai")]
    Moonshotai,
    #[serde(rename = "morph")]
    Morph,
    #[serde(rename = "ncompass")]
    Ncompass,
    #[serde(rename = "nebius")]
    Nebius,
    #[serde(rename = "nex-agi")]
    NexAgi,
    #[serde(rename = "nextbit")]
    Nextbit,
    #[serde(rename = "novita")]
    Novita,
    #[serde(rename = "nvidia")]
    Nvidia,
    #[serde(rename = "open-inference")]
    OpenInference,
    #[serde(rename = "openai")]
    Openai,
    #[serde(rename = "parasail")]
    Parasail,
    #[serde(rename = "perceptron")]
    Perceptron,
    #[serde(rename = "perplexity")]
    Perplexity,
    #[serde(rename = "phala")]
    Phala,
    #[serde(rename = "poolside")]
    Poolside,
    #[serde(rename = "quiver")]
    Quiver,
    #[serde(rename = "recraft")]
    Recraft,
    #[serde(rename = "reka")]
    Reka,
    #[serde(rename = "relace")]
    Relace,
    #[serde(rename = "runway")]
    Runway,
    #[serde(rename = "sail-research")]
    SailResearch,
    #[serde(rename = "sakana")]
    Sakana,
    #[serde(rename = "sakana-ai")]
    SakanaAi,
    #[serde(rename = "sambanova")]
    Sambanova,
    #[serde(rename = "seed")]
    Seed,
    #[serde(rename = "siliconflow")]
    Siliconflow,
    #[serde(rename = "sourceful")]
    Sourceful,
    #[serde(rename = "stepfun")]
    Stepfun,
    #[serde(rename = "streamlake")]
    Streamlake,
    #[serde(rename = "switchpoint")]
    Switchpoint,
    #[serde(rename = "tencent")]
    Tencent,
    #[serde(rename = "tenstorrent")]
    Tenstorrent,
    #[serde(rename = "together")]
    Together,
    #[serde(rename = "upstage")]
    Upstage,
    #[serde(rename = "venice")]
    Venice,
    #[serde(rename = "wafer")]
    Wafer,
    #[serde(rename = "wandb")]
    Wandb,
    #[serde(rename = "wandb-legacy")]
    WandbLegacy,
    #[serde(rename = "xai")]
    Xai,
    #[serde(rename = "xiaomi")]
    Xiaomi,
    #[serde(rename = "z-ai")]
    ZAi,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ListBYOKKeysParamsProviderEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// JSON result for `listBYOKKeys`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListBYOKKeysResult {
    #[serde(flatten)]
    pub body: OrListBYOKKeysResponse,
}

/// Typed params for `GET /embeddings/models` (`listEmbeddingsModels`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListEmbeddingsModelsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listEmbeddingsModels`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListEmbeddingsModelsResult {
    #[serde(flatten)]
    pub body: OrModelsListResponse,
}

/// Typed params for `GET /models/{author}/{slug}/endpoints` (`listEndpoints`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListEndpointsParams {
    pub author: String,
    pub slug: String,
}

/// JSON result for `listEndpoints`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListEndpointsResult {
    #[serde(flatten)]
    pub body: ListEndpointsResultBody,
}

/// Generated object `ListEndpointsResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListEndpointsResultBody {
    pub data: OrListEndpointsResponse,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `GET /endpoints/zdr` (`listEndpointsZdr`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListEndpointsZdrParams {}

/// JSON result for `listEndpointsZdr`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListEndpointsZdrResult {
    #[serde(flatten)]
    pub body: ListEndpointsZdrResultBody,
}

/// Generated object `ListEndpointsZdrResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListEndpointsZdrResultBody {
    pub data: Vec<OrPublicEndpoint>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `GET /files` (`listFiles`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListFilesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// JSON result for `listFiles`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListFilesResult {
    #[serde(flatten)]
    pub body: OrFileListResponse,
}

/// Typed params for `GET /generation/content` (`listGenerationContent`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGenerationContentParams {
    pub id: String,
}

/// JSON result for `listGenerationContent`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGenerationContentResult {
    #[serde(flatten)]
    pub body: OrGenerationContentResponse,
}

/// Typed params for `GET /guardrails/{id}/assignments/keys` (`listGuardrailKeyAssignments`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGuardrailKeyAssignmentsParams {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listGuardrailKeyAssignments`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGuardrailKeyAssignmentsResult {
    #[serde(flatten)]
    pub body: OrListKeyAssignmentsResponse,
}

/// Typed params for `GET /guardrails/{id}/assignments/members` (`listGuardrailMemberAssignments`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGuardrailMemberAssignmentsParams {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listGuardrailMemberAssignments`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGuardrailMemberAssignmentsResult {
    #[serde(flatten)]
    pub body: OrListMemberAssignmentsResponse,
}

/// Typed params for `GET /guardrails` (`listGuardrails`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGuardrailsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// JSON result for `listGuardrails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGuardrailsResult {
    #[serde(flatten)]
    pub body: OrListGuardrailsResponse,
}

/// Typed params for `GET /images/models/{author}/{slug}/endpoints` (`listImageModelEndpoints`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListImageModelEndpointsParams {
    pub author: String,
    pub slug: String,
}

/// JSON result for `listImageModelEndpoints`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListImageModelEndpointsResult {
    #[serde(flatten)]
    pub body: OrImageModelEndpointsResponse,
}

/// Typed params for `GET /images/models` (`listImageModels`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListImageModelsParams {}

/// JSON result for `listImageModels`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListImageModelsResult {
    #[serde(flatten)]
    pub body: OrImageModelsListResponse,
}

/// Typed params for `GET /guardrails/assignments/keys` (`listKeyAssignments`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListKeyAssignmentsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listKeyAssignments`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListKeyAssignmentsResult {
    #[serde(flatten)]
    pub body: OrListKeyAssignmentsResponse,
}

/// Typed params for `GET /guardrails/assignments/members` (`listMemberAssignments`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListMemberAssignmentsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listMemberAssignments`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListMemberAssignmentsResult {
    #[serde(flatten)]
    pub body: OrListMemberAssignmentsResponse,
}

/// Typed params for `GET /models/count` (`listModelsCount`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListModelsCountParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<String>,
}

/// JSON result for `listModelsCount`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListModelsCountResult {
    #[serde(flatten)]
    pub body: OrModelsCountResponse,
}

/// Typed params for `GET /models/user` (`listModelsUser`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListModelsUserParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listModelsUser`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListModelsUserResult {
    #[serde(flatten)]
    pub body: OrModelsListResponse,
}

/// Typed params for `GET /observability/destinations` (`listObservabilityDestinations`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListObservabilityDestinationsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// JSON result for `listObservabilityDestinations`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListObservabilityDestinationsResult {
    #[serde(flatten)]
    pub body: OrListObservabilityDestinationsResponse,
}

/// Typed params for `GET /organization/members` (`listOrganizationMembers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListOrganizationMembersParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listOrganizationMembers`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListOrganizationMembersResult {
    #[serde(flatten)]
    pub body: ListOrganizationMembersResultBody,
}

/// Generated object `ListOrganizationMembersResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListOrganizationMembersResultBody {
    pub data: Vec<ListOrganizationMembersResultBodyDataItem>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ListOrganizationMembersResultBodyDataItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListOrganizationMembersResultBodyDataItem {
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    pub role: ListOrganizationMembersResultBodyDataItemRoleEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ListOrganizationMembersResultBodyDataItemRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListOrganizationMembersResultBodyDataItemRoleEnum {
    #[serde(rename = "org:admin")]
    OrgAdmin,
    #[serde(rename = "org:member")]
    OrgMember,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ListOrganizationMembersResultBodyDataItemRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Typed params for `GET /keys` (`list`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// Typed params for `GET /presets/{slug}/versions` (`listPresetVersions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListPresetVersionsParams {
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listPresetVersions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListPresetVersionsResult {
    #[serde(flatten)]
    pub body: OrListPresetVersionsResponse,
}

/// Typed params for `GET /presets` (`listPresets`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListPresetsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listPresets`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListPresetsResult {
    #[serde(flatten)]
    pub body: OrListPresetsResponse,
}

/// Typed params for `GET /providers` (`listProviders`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProvidersParams {}

/// JSON result for `listProviders`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProvidersResult {
    #[serde(flatten)]
    pub body: ListProvidersResultBody,
}

/// Generated object `ListProvidersResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProvidersResultBody {
    pub data: Vec<ListProvidersResultBodyDataItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ListProvidersResultBodyDataItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProvidersResultBodyDataItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datacenters: Option<Vec<ListProvidersResultBodyDataItemDatacentersItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headquarters: Option<ListProvidersResultBodyDataItemHeadquartersEnum>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_policy_url: Option<String>,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_page_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_of_service_url: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ListProvidersResultBodyDataItemDatacentersItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListProvidersResultBodyDataItemDatacentersItemEnum {
    #[serde(rename = "AD")]
    AD,
    #[serde(rename = "AE")]
    AE,
    #[serde(rename = "AF")]
    AF,
    #[serde(rename = "AG")]
    AG,
    #[serde(rename = "AI")]
    AI,
    #[serde(rename = "AL")]
    AL,
    #[serde(rename = "AM")]
    AM,
    #[serde(rename = "AO")]
    AO,
    #[serde(rename = "AQ")]
    AQ,
    #[serde(rename = "AR")]
    AR,
    #[serde(rename = "AS")]
    AS,
    #[serde(rename = "AT")]
    AT,
    #[serde(rename = "AU")]
    AU,
    #[serde(rename = "AW")]
    AW,
    #[serde(rename = "AX")]
    AX,
    #[serde(rename = "AZ")]
    AZ,
    #[serde(rename = "BA")]
    BA,
    #[serde(rename = "BB")]
    BB,
    #[serde(rename = "BD")]
    BD,
    #[serde(rename = "BE")]
    BE,
    #[serde(rename = "BF")]
    BF,
    #[serde(rename = "BG")]
    BG,
    #[serde(rename = "BH")]
    BH,
    #[serde(rename = "BI")]
    BI,
    #[serde(rename = "BJ")]
    BJ,
    #[serde(rename = "BL")]
    BL,
    #[serde(rename = "BM")]
    BM,
    #[serde(rename = "BN")]
    BN,
    #[serde(rename = "BO")]
    BO,
    #[serde(rename = "BQ")]
    BQ,
    #[serde(rename = "BR")]
    BR,
    #[serde(rename = "BS")]
    BS,
    #[serde(rename = "BT")]
    BT,
    #[serde(rename = "BV")]
    BV,
    #[serde(rename = "BW")]
    BW,
    #[serde(rename = "BY")]
    BY,
    #[serde(rename = "BZ")]
    BZ,
    #[serde(rename = "CA")]
    CA,
    #[serde(rename = "CC")]
    CC,
    #[serde(rename = "CD")]
    CD,
    #[serde(rename = "CF")]
    CF,
    #[serde(rename = "CG")]
    CG,
    #[serde(rename = "CH")]
    CH,
    #[serde(rename = "CI")]
    CI,
    #[serde(rename = "CK")]
    CK,
    #[serde(rename = "CL")]
    CL,
    #[serde(rename = "CM")]
    CM,
    #[serde(rename = "CN")]
    CN,
    #[serde(rename = "CO")]
    CO,
    #[serde(rename = "CR")]
    CR,
    #[serde(rename = "CU")]
    CU,
    #[serde(rename = "CV")]
    CV,
    #[serde(rename = "CW")]
    CW,
    #[serde(rename = "CX")]
    CX,
    #[serde(rename = "CY")]
    CY,
    #[serde(rename = "CZ")]
    CZ,
    #[serde(rename = "DE")]
    DE,
    #[serde(rename = "DJ")]
    DJ,
    #[serde(rename = "DK")]
    DK,
    #[serde(rename = "DM")]
    DM,
    #[serde(rename = "DO")]
    DO,
    #[serde(rename = "DZ")]
    DZ,
    #[serde(rename = "EC")]
    EC,
    #[serde(rename = "EE")]
    EE,
    #[serde(rename = "EG")]
    EG,
    #[serde(rename = "EH")]
    EH,
    #[serde(rename = "ER")]
    ER,
    #[serde(rename = "ES")]
    ES,
    #[serde(rename = "ET")]
    ET,
    #[serde(rename = "FI")]
    FI,
    #[serde(rename = "FJ")]
    FJ,
    #[serde(rename = "FK")]
    FK,
    #[serde(rename = "FM")]
    FM,
    #[serde(rename = "FO")]
    FO,
    #[serde(rename = "FR")]
    FR,
    #[serde(rename = "GA")]
    GA,
    #[serde(rename = "GB")]
    GB,
    #[serde(rename = "GD")]
    GD,
    #[serde(rename = "GE")]
    GE,
    #[serde(rename = "GF")]
    GF,
    #[serde(rename = "GG")]
    GG,
    #[serde(rename = "GH")]
    GH,
    #[serde(rename = "GI")]
    GI,
    #[serde(rename = "GL")]
    GL,
    #[serde(rename = "GM")]
    GM,
    #[serde(rename = "GN")]
    GN,
    #[serde(rename = "GP")]
    GP,
    #[serde(rename = "GQ")]
    GQ,
    #[serde(rename = "GR")]
    GR,
    #[serde(rename = "GS")]
    GS,
    #[serde(rename = "GT")]
    GT,
    #[serde(rename = "GU")]
    GU,
    #[serde(rename = "GW")]
    GW,
    #[serde(rename = "GY")]
    GY,
    #[serde(rename = "HK")]
    HK,
    #[serde(rename = "HM")]
    HM,
    #[serde(rename = "HN")]
    HN,
    #[serde(rename = "HR")]
    HR,
    #[serde(rename = "HT")]
    HT,
    #[serde(rename = "HU")]
    HU,
    #[serde(rename = "ID")]
    ID,
    #[serde(rename = "IE")]
    IE,
    #[serde(rename = "IL")]
    IL,
    #[serde(rename = "IM")]
    IM,
    #[serde(rename = "IN")]
    IN,
    #[serde(rename = "IO")]
    IO,
    #[serde(rename = "IQ")]
    IQ,
    #[serde(rename = "IR")]
    IR,
    #[serde(rename = "IS")]
    IS,
    #[serde(rename = "IT")]
    IT,
    #[serde(rename = "JE")]
    JE,
    #[serde(rename = "JM")]
    JM,
    #[serde(rename = "JO")]
    JO,
    #[serde(rename = "JP")]
    JP,
    #[serde(rename = "KE")]
    KE,
    #[serde(rename = "KG")]
    KG,
    #[serde(rename = "KH")]
    KH,
    #[serde(rename = "KI")]
    KI,
    #[serde(rename = "KM")]
    KM,
    #[serde(rename = "KN")]
    KN,
    #[serde(rename = "KP")]
    KP,
    #[serde(rename = "KR")]
    KR,
    #[serde(rename = "KW")]
    KW,
    #[serde(rename = "KY")]
    KY,
    #[serde(rename = "KZ")]
    KZ,
    #[serde(rename = "LA")]
    LA,
    #[serde(rename = "LB")]
    LB,
    #[serde(rename = "LC")]
    LC,
    #[serde(rename = "LI")]
    LI,
    #[serde(rename = "LK")]
    LK,
    #[serde(rename = "LR")]
    LR,
    #[serde(rename = "LS")]
    LS,
    #[serde(rename = "LT")]
    LT,
    #[serde(rename = "LU")]
    LU,
    #[serde(rename = "LV")]
    LV,
    #[serde(rename = "LY")]
    LY,
    #[serde(rename = "MA")]
    MA,
    #[serde(rename = "MC")]
    MC,
    #[serde(rename = "MD")]
    MD,
    #[serde(rename = "ME")]
    ME,
    #[serde(rename = "MF")]
    MF,
    #[serde(rename = "MG")]
    MG,
    #[serde(rename = "MH")]
    MH,
    #[serde(rename = "MK")]
    MK,
    #[serde(rename = "ML")]
    ML,
    #[serde(rename = "MM")]
    MM,
    #[serde(rename = "MN")]
    MN,
    #[serde(rename = "MO")]
    MO,
    #[serde(rename = "MP")]
    MP,
    #[serde(rename = "MQ")]
    MQ,
    #[serde(rename = "MR")]
    MR,
    #[serde(rename = "MS")]
    MS,
    #[serde(rename = "MT")]
    MT,
    #[serde(rename = "MU")]
    MU,
    #[serde(rename = "MV")]
    MV,
    #[serde(rename = "MW")]
    MW,
    #[serde(rename = "MX")]
    MX,
    #[serde(rename = "MY")]
    MY,
    #[serde(rename = "MZ")]
    MZ,
    #[serde(rename = "NA")]
    NA,
    #[serde(rename = "NC")]
    NC,
    #[serde(rename = "NE")]
    NE,
    #[serde(rename = "NF")]
    NF,
    #[serde(rename = "NG")]
    NG,
    #[serde(rename = "NI")]
    NI,
    #[serde(rename = "NL")]
    NL,
    #[serde(rename = "NO")]
    NO,
    #[serde(rename = "NP")]
    NP,
    #[serde(rename = "NR")]
    NR,
    #[serde(rename = "NU")]
    NU,
    #[serde(rename = "NZ")]
    NZ,
    #[serde(rename = "OM")]
    OM,
    #[serde(rename = "PA")]
    PA,
    #[serde(rename = "PE")]
    PE,
    #[serde(rename = "PF")]
    PF,
    #[serde(rename = "PG")]
    PG,
    #[serde(rename = "PH")]
    PH,
    #[serde(rename = "PK")]
    PK,
    #[serde(rename = "PL")]
    PL,
    #[serde(rename = "PM")]
    PM,
    #[serde(rename = "PN")]
    PN,
    #[serde(rename = "PR")]
    PR,
    #[serde(rename = "PS")]
    PS,
    #[serde(rename = "PT")]
    PT,
    #[serde(rename = "PW")]
    PW,
    #[serde(rename = "PY")]
    PY,
    #[serde(rename = "QA")]
    QA,
    #[serde(rename = "RE")]
    RE,
    #[serde(rename = "RO")]
    RO,
    #[serde(rename = "RS")]
    RS,
    #[serde(rename = "RU")]
    RU,
    #[serde(rename = "RW")]
    RW,
    #[serde(rename = "SA")]
    SA,
    #[serde(rename = "SB")]
    SB,
    #[serde(rename = "SC")]
    SC,
    #[serde(rename = "SD")]
    SD,
    #[serde(rename = "SE")]
    SE,
    #[serde(rename = "SG")]
    SG,
    #[serde(rename = "SH")]
    SH,
    #[serde(rename = "SI")]
    SI,
    #[serde(rename = "SJ")]
    SJ,
    #[serde(rename = "SK")]
    SK,
    #[serde(rename = "SL")]
    SL,
    #[serde(rename = "SM")]
    SM,
    #[serde(rename = "SN")]
    SN,
    #[serde(rename = "SO")]
    SO,
    #[serde(rename = "SR")]
    SR,
    #[serde(rename = "SS")]
    SS,
    #[serde(rename = "ST")]
    ST,
    #[serde(rename = "SV")]
    SV,
    #[serde(rename = "SX")]
    SX,
    #[serde(rename = "SY")]
    SY,
    #[serde(rename = "SZ")]
    SZ,
    #[serde(rename = "TC")]
    TC,
    #[serde(rename = "TD")]
    TD,
    #[serde(rename = "TF")]
    TF,
    #[serde(rename = "TG")]
    TG,
    #[serde(rename = "TH")]
    TH,
    #[serde(rename = "TJ")]
    TJ,
    #[serde(rename = "TK")]
    TK,
    #[serde(rename = "TL")]
    TL,
    #[serde(rename = "TM")]
    TM,
    #[serde(rename = "TN")]
    TN,
    #[serde(rename = "TO")]
    TO,
    #[serde(rename = "TR")]
    TR,
    #[serde(rename = "TT")]
    TT,
    #[serde(rename = "TV")]
    TV,
    #[serde(rename = "TW")]
    TW,
    #[serde(rename = "TZ")]
    TZ,
    #[serde(rename = "UA")]
    UA,
    #[serde(rename = "UG")]
    UG,
    #[serde(rename = "UM")]
    UM,
    #[serde(rename = "US")]
    US,
    #[serde(rename = "UY")]
    UY,
    #[serde(rename = "UZ")]
    UZ,
    #[serde(rename = "VA")]
    VA,
    #[serde(rename = "VC")]
    VC,
    #[serde(rename = "VE")]
    VE,
    #[serde(rename = "VG")]
    VG,
    #[serde(rename = "VI")]
    VI,
    #[serde(rename = "VN")]
    VN,
    #[serde(rename = "VU")]
    VU,
    #[serde(rename = "WF")]
    WF,
    #[serde(rename = "WS")]
    WS,
    #[serde(rename = "YE")]
    YE,
    #[serde(rename = "YT")]
    YT,
    #[serde(rename = "ZA")]
    ZA,
    #[serde(rename = "ZM")]
    ZM,
    #[serde(rename = "ZW")]
    ZW,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ListProvidersResultBodyDataItemDatacentersItemEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `ListProvidersResultBodyDataItemHeadquartersEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListProvidersResultBodyDataItemHeadquartersEnum {
    #[serde(rename = "AD")]
    AD,
    #[serde(rename = "AE")]
    AE,
    #[serde(rename = "AF")]
    AF,
    #[serde(rename = "AG")]
    AG,
    #[serde(rename = "AI")]
    AI,
    #[serde(rename = "AL")]
    AL,
    #[serde(rename = "AM")]
    AM,
    #[serde(rename = "AO")]
    AO,
    #[serde(rename = "AQ")]
    AQ,
    #[serde(rename = "AR")]
    AR,
    #[serde(rename = "AS")]
    AS,
    #[serde(rename = "AT")]
    AT,
    #[serde(rename = "AU")]
    AU,
    #[serde(rename = "AW")]
    AW,
    #[serde(rename = "AX")]
    AX,
    #[serde(rename = "AZ")]
    AZ,
    #[serde(rename = "BA")]
    BA,
    #[serde(rename = "BB")]
    BB,
    #[serde(rename = "BD")]
    BD,
    #[serde(rename = "BE")]
    BE,
    #[serde(rename = "BF")]
    BF,
    #[serde(rename = "BG")]
    BG,
    #[serde(rename = "BH")]
    BH,
    #[serde(rename = "BI")]
    BI,
    #[serde(rename = "BJ")]
    BJ,
    #[serde(rename = "BL")]
    BL,
    #[serde(rename = "BM")]
    BM,
    #[serde(rename = "BN")]
    BN,
    #[serde(rename = "BO")]
    BO,
    #[serde(rename = "BQ")]
    BQ,
    #[serde(rename = "BR")]
    BR,
    #[serde(rename = "BS")]
    BS,
    #[serde(rename = "BT")]
    BT,
    #[serde(rename = "BV")]
    BV,
    #[serde(rename = "BW")]
    BW,
    #[serde(rename = "BY")]
    BY,
    #[serde(rename = "BZ")]
    BZ,
    #[serde(rename = "CA")]
    CA,
    #[serde(rename = "CC")]
    CC,
    #[serde(rename = "CD")]
    CD,
    #[serde(rename = "CF")]
    CF,
    #[serde(rename = "CG")]
    CG,
    #[serde(rename = "CH")]
    CH,
    #[serde(rename = "CI")]
    CI,
    #[serde(rename = "CK")]
    CK,
    #[serde(rename = "CL")]
    CL,
    #[serde(rename = "CM")]
    CM,
    #[serde(rename = "CN")]
    CN,
    #[serde(rename = "CO")]
    CO,
    #[serde(rename = "CR")]
    CR,
    #[serde(rename = "CU")]
    CU,
    #[serde(rename = "CV")]
    CV,
    #[serde(rename = "CW")]
    CW,
    #[serde(rename = "CX")]
    CX,
    #[serde(rename = "CY")]
    CY,
    #[serde(rename = "CZ")]
    CZ,
    #[serde(rename = "DE")]
    DE,
    #[serde(rename = "DJ")]
    DJ,
    #[serde(rename = "DK")]
    DK,
    #[serde(rename = "DM")]
    DM,
    #[serde(rename = "DO")]
    DO,
    #[serde(rename = "DZ")]
    DZ,
    #[serde(rename = "EC")]
    EC,
    #[serde(rename = "EE")]
    EE,
    #[serde(rename = "EG")]
    EG,
    #[serde(rename = "EH")]
    EH,
    #[serde(rename = "ER")]
    ER,
    #[serde(rename = "ES")]
    ES,
    #[serde(rename = "ET")]
    ET,
    #[serde(rename = "FI")]
    FI,
    #[serde(rename = "FJ")]
    FJ,
    #[serde(rename = "FK")]
    FK,
    #[serde(rename = "FM")]
    FM,
    #[serde(rename = "FO")]
    FO,
    #[serde(rename = "FR")]
    FR,
    #[serde(rename = "GA")]
    GA,
    #[serde(rename = "GB")]
    GB,
    #[serde(rename = "GD")]
    GD,
    #[serde(rename = "GE")]
    GE,
    #[serde(rename = "GF")]
    GF,
    #[serde(rename = "GG")]
    GG,
    #[serde(rename = "GH")]
    GH,
    #[serde(rename = "GI")]
    GI,
    #[serde(rename = "GL")]
    GL,
    #[serde(rename = "GM")]
    GM,
    #[serde(rename = "GN")]
    GN,
    #[serde(rename = "GP")]
    GP,
    #[serde(rename = "GQ")]
    GQ,
    #[serde(rename = "GR")]
    GR,
    #[serde(rename = "GS")]
    GS,
    #[serde(rename = "GT")]
    GT,
    #[serde(rename = "GU")]
    GU,
    #[serde(rename = "GW")]
    GW,
    #[serde(rename = "GY")]
    GY,
    #[serde(rename = "HK")]
    HK,
    #[serde(rename = "HM")]
    HM,
    #[serde(rename = "HN")]
    HN,
    #[serde(rename = "HR")]
    HR,
    #[serde(rename = "HT")]
    HT,
    #[serde(rename = "HU")]
    HU,
    #[serde(rename = "ID")]
    ID,
    #[serde(rename = "IE")]
    IE,
    #[serde(rename = "IL")]
    IL,
    #[serde(rename = "IM")]
    IM,
    #[serde(rename = "IN")]
    IN,
    #[serde(rename = "IO")]
    IO,
    #[serde(rename = "IQ")]
    IQ,
    #[serde(rename = "IR")]
    IR,
    #[serde(rename = "IS")]
    IS,
    #[serde(rename = "IT")]
    IT,
    #[serde(rename = "JE")]
    JE,
    #[serde(rename = "JM")]
    JM,
    #[serde(rename = "JO")]
    JO,
    #[serde(rename = "JP")]
    JP,
    #[serde(rename = "KE")]
    KE,
    #[serde(rename = "KG")]
    KG,
    #[serde(rename = "KH")]
    KH,
    #[serde(rename = "KI")]
    KI,
    #[serde(rename = "KM")]
    KM,
    #[serde(rename = "KN")]
    KN,
    #[serde(rename = "KP")]
    KP,
    #[serde(rename = "KR")]
    KR,
    #[serde(rename = "KW")]
    KW,
    #[serde(rename = "KY")]
    KY,
    #[serde(rename = "KZ")]
    KZ,
    #[serde(rename = "LA")]
    LA,
    #[serde(rename = "LB")]
    LB,
    #[serde(rename = "LC")]
    LC,
    #[serde(rename = "LI")]
    LI,
    #[serde(rename = "LK")]
    LK,
    #[serde(rename = "LR")]
    LR,
    #[serde(rename = "LS")]
    LS,
    #[serde(rename = "LT")]
    LT,
    #[serde(rename = "LU")]
    LU,
    #[serde(rename = "LV")]
    LV,
    #[serde(rename = "LY")]
    LY,
    #[serde(rename = "MA")]
    MA,
    #[serde(rename = "MC")]
    MC,
    #[serde(rename = "MD")]
    MD,
    #[serde(rename = "ME")]
    ME,
    #[serde(rename = "MF")]
    MF,
    #[serde(rename = "MG")]
    MG,
    #[serde(rename = "MH")]
    MH,
    #[serde(rename = "MK")]
    MK,
    #[serde(rename = "ML")]
    ML,
    #[serde(rename = "MM")]
    MM,
    #[serde(rename = "MN")]
    MN,
    #[serde(rename = "MO")]
    MO,
    #[serde(rename = "MP")]
    MP,
    #[serde(rename = "MQ")]
    MQ,
    #[serde(rename = "MR")]
    MR,
    #[serde(rename = "MS")]
    MS,
    #[serde(rename = "MT")]
    MT,
    #[serde(rename = "MU")]
    MU,
    #[serde(rename = "MV")]
    MV,
    #[serde(rename = "MW")]
    MW,
    #[serde(rename = "MX")]
    MX,
    #[serde(rename = "MY")]
    MY,
    #[serde(rename = "MZ")]
    MZ,
    #[serde(rename = "NA")]
    NA,
    #[serde(rename = "NC")]
    NC,
    #[serde(rename = "NE")]
    NE,
    #[serde(rename = "NF")]
    NF,
    #[serde(rename = "NG")]
    NG,
    #[serde(rename = "NI")]
    NI,
    #[serde(rename = "NL")]
    NL,
    #[serde(rename = "NO")]
    NO,
    #[serde(rename = "NP")]
    NP,
    #[serde(rename = "NR")]
    NR,
    #[serde(rename = "NU")]
    NU,
    #[serde(rename = "NZ")]
    NZ,
    #[serde(rename = "OM")]
    OM,
    #[serde(rename = "PA")]
    PA,
    #[serde(rename = "PE")]
    PE,
    #[serde(rename = "PF")]
    PF,
    #[serde(rename = "PG")]
    PG,
    #[serde(rename = "PH")]
    PH,
    #[serde(rename = "PK")]
    PK,
    #[serde(rename = "PL")]
    PL,
    #[serde(rename = "PM")]
    PM,
    #[serde(rename = "PN")]
    PN,
    #[serde(rename = "PR")]
    PR,
    #[serde(rename = "PS")]
    PS,
    #[serde(rename = "PT")]
    PT,
    #[serde(rename = "PW")]
    PW,
    #[serde(rename = "PY")]
    PY,
    #[serde(rename = "QA")]
    QA,
    #[serde(rename = "RE")]
    RE,
    #[serde(rename = "RO")]
    RO,
    #[serde(rename = "RS")]
    RS,
    #[serde(rename = "RU")]
    RU,
    #[serde(rename = "RW")]
    RW,
    #[serde(rename = "SA")]
    SA,
    #[serde(rename = "SB")]
    SB,
    #[serde(rename = "SC")]
    SC,
    #[serde(rename = "SD")]
    SD,
    #[serde(rename = "SE")]
    SE,
    #[serde(rename = "SG")]
    SG,
    #[serde(rename = "SH")]
    SH,
    #[serde(rename = "SI")]
    SI,
    #[serde(rename = "SJ")]
    SJ,
    #[serde(rename = "SK")]
    SK,
    #[serde(rename = "SL")]
    SL,
    #[serde(rename = "SM")]
    SM,
    #[serde(rename = "SN")]
    SN,
    #[serde(rename = "SO")]
    SO,
    #[serde(rename = "SR")]
    SR,
    #[serde(rename = "SS")]
    SS,
    #[serde(rename = "ST")]
    ST,
    #[serde(rename = "SV")]
    SV,
    #[serde(rename = "SX")]
    SX,
    #[serde(rename = "SY")]
    SY,
    #[serde(rename = "SZ")]
    SZ,
    #[serde(rename = "TC")]
    TC,
    #[serde(rename = "TD")]
    TD,
    #[serde(rename = "TF")]
    TF,
    #[serde(rename = "TG")]
    TG,
    #[serde(rename = "TH")]
    TH,
    #[serde(rename = "TJ")]
    TJ,
    #[serde(rename = "TK")]
    TK,
    #[serde(rename = "TL")]
    TL,
    #[serde(rename = "TM")]
    TM,
    #[serde(rename = "TN")]
    TN,
    #[serde(rename = "TO")]
    TO,
    #[serde(rename = "TR")]
    TR,
    #[serde(rename = "TT")]
    TT,
    #[serde(rename = "TV")]
    TV,
    #[serde(rename = "TW")]
    TW,
    #[serde(rename = "TZ")]
    TZ,
    #[serde(rename = "UA")]
    UA,
    #[serde(rename = "UG")]
    UG,
    #[serde(rename = "UM")]
    UM,
    #[serde(rename = "US")]
    US,
    #[serde(rename = "UY")]
    UY,
    #[serde(rename = "UZ")]
    UZ,
    #[serde(rename = "VA")]
    VA,
    #[serde(rename = "VC")]
    VC,
    #[serde(rename = "VE")]
    VE,
    #[serde(rename = "VG")]
    VG,
    #[serde(rename = "VI")]
    VI,
    #[serde(rename = "VN")]
    VN,
    #[serde(rename = "VU")]
    VU,
    #[serde(rename = "WF")]
    WF,
    #[serde(rename = "WS")]
    WS,
    #[serde(rename = "YE")]
    YE,
    #[serde(rename = "YT")]
    YT,
    #[serde(rename = "ZA")]
    ZA,
    #[serde(rename = "ZM")]
    ZM,
    #[serde(rename = "ZW")]
    ZW,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for ListProvidersResultBodyDataItemHeadquartersEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// JSON result for `list`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListResult {
    #[serde(flatten)]
    pub body: ListResultBody,
}

/// Generated object `ListResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListResultBody {
    pub data: Vec<ListResultBodyDataItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ListResultBodyDataItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListResultBodyDataItem {
    pub byok_usage: f64,
    pub byok_usage_daily: f64,
    pub byok_usage_monthly: f64,
    pub byok_usage_weekly: f64,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_user_id: Option<String>,
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub hash: String,
    pub include_byok_in_limit: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_remaining: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reset: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub usage: f64,
    pub usage_daily: f64,
    pub usage_monthly: f64,
    pub usage_weekly: f64,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `GET /videos/{jobId}/content` (`listVideosContent`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListVideosContentParams {
    pub job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
}

/// Binary result for `listVideosContent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListVideosContentResult {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

/// Typed params for `GET /videos/models` (`listVideosModels`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListVideosModelsParams {}

/// JSON result for `listVideosModels`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListVideosModelsResult {
    #[serde(flatten)]
    pub body: OrVideoModelsListResponse,
}

/// Typed params for `GET /workspaces/{id}/budgets` (`listWorkspaceBudgets`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListWorkspaceBudgetsParams {
    pub id: String,
}

/// JSON result for `listWorkspaceBudgets`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListWorkspaceBudgetsResult {
    #[serde(flatten)]
    pub body: OrListWorkspaceBudgetsResponse,
}

/// Typed params for `GET /workspaces/{id}/members` (`listWorkspaceMembers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListWorkspaceMembersParams {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listWorkspaceMembers`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListWorkspaceMembersResult {
    #[serde(flatten)]
    pub body: OrListWorkspaceMembersResponse,
}

/// Typed params for `GET /workspaces` (`listWorkspaces`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListWorkspacesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// JSON result for `listWorkspaces`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListWorkspacesResult {
    #[serde(flatten)]
    pub body: OrListWorkspacesResponse,
}

/// Generated string enum `OpenAIResponsesToolChoiceV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAIResponsesToolChoiceV0Enum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OpenAIResponsesToolChoiceV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OpenAIResponsesToolChoiceV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAIResponsesToolChoiceV1Enum {
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OpenAIResponsesToolChoiceV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OpenAIResponsesToolChoiceV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAIResponsesToolChoiceV2Enum {
    #[serde(rename = "required")]
    Required,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OpenAIResponsesToolChoiceV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OpenAIResponsesToolChoiceV3`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenAIResponsesToolChoiceV3 {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: OpenAIResponsesToolChoiceV3TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OpenAIResponsesToolChoiceV3TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAIResponsesToolChoiceV3TypeEnum {
    #[serde(rename = "function")]
    Function,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OpenAIResponsesToolChoiceV3TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OpenAIResponsesToolChoiceV4`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenAIResponsesToolChoiceV4 {
    #[serde(rename = "type")]
    pub type_: OpenAIResponsesToolChoiceV4TypeUnion,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OpenAIResponsesToolChoiceV4TypeUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpenAIResponsesToolChoiceV4TypeUnion {
    Variant0(OpenAIResponsesToolChoiceV4TypeV0Enum),
    Variant1(OpenAIResponsesToolChoiceV4TypeV1Enum),
    Unknown(serde_json::Value),
}
impl Default for OpenAIResponsesToolChoiceV4TypeUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OpenAIResponsesToolChoiceV4TypeV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAIResponsesToolChoiceV4TypeV0Enum {
    #[serde(rename = "web_search_preview_2025_03_11")]
    WebSearchPreview20250311,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OpenAIResponsesToolChoiceV4TypeV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OpenAIResponsesToolChoiceV4TypeV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAIResponsesToolChoiceV4TypeV1Enum {
    #[serde(rename = "web_search_preview")]
    WebSearchPreview,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OpenAIResponsesToolChoiceV4TypeV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OpenAIResponsesToolChoiceV6`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenAIResponsesToolChoiceV6 {
    #[serde(rename = "type")]
    pub type_: OpenAIResponsesToolChoiceV6TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OpenAIResponsesToolChoiceV6TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAIResponsesToolChoiceV6TypeEnum {
    #[serde(rename = "apply_patch")]
    ApplyPatch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OpenAIResponsesToolChoiceV6TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OpenAIResponsesToolChoiceV7`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenAIResponsesToolChoiceV7 {
    #[serde(rename = "type")]
    pub type_: OpenAIResponsesToolChoiceV7TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OpenAIResponsesToolChoiceV7TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenAIResponsesToolChoiceV7TypeEnum {
    #[serde(rename = "shell")]
    Shell,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OpenAIResponsesToolChoiceV7TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAABenchmarkEntry`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAABenchmarkEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agentic_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coding_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intelligence_index: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrActivityItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrActivityItem {
    pub byok_usage_inference: f64,
    pub completion_tokens: i64,
    pub date: String,
    pub endpoint_id: String,
    pub model: String,
    pub model_permaslug: String,
    pub prompt_tokens: i64,
    pub provider_name: String,
    pub reasoning_tokens: i64,
    pub requests: i64,
    pub usage: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrActivityResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrActivityResponse {
    pub data: Vec<OrActivityItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAdditionalToolsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAdditionalToolsItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub role: OrAdditionalToolsItemRoleEnum,
    pub tools: Vec<OrAdditionalToolsItemToolsItemUnion>,
    #[serde(rename = "type")]
    pub type_: OrAdditionalToolsItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAdditionalToolsItemRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAdditionalToolsItemRoleEnum {
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "critic")]
    Critic,
    #[serde(rename = "discriminator")]
    Discriminator,
    #[serde(rename = "developer")]
    Developer,
    #[serde(rename = "tool")]
    Tool,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAdditionalToolsItemRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrAdditionalToolsItemToolsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAdditionalToolsItemToolsItemUnion {
    Variant0(OrAdditionalToolsItemToolsItemV0),
    Variant1(OrPreviewWebSearchServerTool),
    Variant2(OrPreview20250311WebSearchServerTool),
    Variant3(OrLegacyWebSearchServerTool),
    Variant4(OrWebSearchServerTool),
    Variant5(OrFileSearchServerTool),
    Variant6(OrComputerUseServerTool),
    Variant7(OrCodeInterpreterServerTool),
    Variant8(OrMcpServerTool),
    Variant9(OrImageGenerationServerTool),
    Variant10(OrCodexLocalShellTool),
    Variant11(OrShellServerTool),
    Variant12(OrApplyPatchServerTool),
    Variant13(OrCustomTool),
    Variant14(OrNamespaceTool),
    Variant15(OrAdvisorServerToolOpenRouter),
    Variant16(OrSubagentServerToolOpenRouter),
    Variant17(OrDatetimeServerTool),
    Variant18(OrFilesServerTool),
    Variant19(OrFusionServerToolOpenRouter),
    Variant20(OrImageGenerationServerToolOpenRouter),
    Variant21(OrSearchModelsServerToolOpenRouter),
    Variant22(OrWebFetchServerTool),
    Variant23(OrWebSearchServerToolOpenRouter),
    Variant24(OrApplyPatchServerToolOpenRouter),
    Variant25(OrBashServerTool),
    Variant26(OrShellServerToolOpenRouter),
    Variant27(OrAdditionalToolsItemToolsItemV27),
    Unknown(serde_json::Value),
}
impl Default for OrAdditionalToolsItemToolsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAdditionalToolsItemToolsItemV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAdditionalToolsItemToolsItemV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrAdditionalToolsItemToolsItemV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAdditionalToolsItemToolsItemV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAdditionalToolsItemToolsItemV0TypeEnum {
    #[serde(rename = "function")]
    Function,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAdditionalToolsItemToolsItemV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAdditionalToolsItemToolsItemV27`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAdditionalToolsItemToolsItemV27 {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAdditionalToolsItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAdditionalToolsItemTypeEnum {
    #[serde(rename = "additional_tools")]
    AdditionalTools,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAdditionalToolsItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAdvisorNestedTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAdvisorNestedTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAdvisorReasoning`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAdvisorReasoning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<OrAdvisorReasoningEffortEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAdvisorReasoningEffortEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAdvisorReasoningEffortEnum {
    #[serde(rename = "max")]
    Max,
    #[serde(rename = "xhigh")]
    Xhigh,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAdvisorReasoningEffortEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAdvisorServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAdvisorServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward_transcript: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OrAdvisorReasoning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OrAdvisorNestedTool>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAdvisorServerToolOpenRouter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAdvisorServerToolOpenRouter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrAdvisorServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrAdvisorServerToolOpenRouterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAdvisorServerToolOpenRouterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAdvisorServerToolOpenRouterTypeEnum {
    #[serde(rename = "openrouter:advisor")]
    OpenrouterAdvisor,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAdvisorServerToolOpenRouterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAgentMessageItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAgentMessageItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<OrAgentMessageItemAgent>,
    pub author: String,
    pub content: Vec<OrAgentMessageItemContentItemUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub recipient: String,
    #[serde(rename = "type")]
    pub type_: OrAgentMessageItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAgentMessageItemAgent`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAgentMessageItemAgent {
    pub agent_name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrAgentMessageItemContentItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAgentMessageItemContentItemUnion {
    Variant0(OrInputText),
    Variant1(OrAgentMessageItemContentItemV1),
    Variant2(OrAgentMessageItemContentItemV2),
    Unknown(serde_json::Value),
}
impl Default for OrAgentMessageItemContentItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAgentMessageItemContentItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAgentMessageItemContentItemV1 {
    pub detail: OrAgentMessageItemContentItemV1DetailEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAgentMessageItemContentItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAgentMessageItemContentItemV1DetailEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAgentMessageItemContentItemV1DetailEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "original")]
    Original,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAgentMessageItemContentItemV1DetailEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAgentMessageItemContentItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAgentMessageItemContentItemV1TypeEnum {
    #[serde(rename = "input_image")]
    InputImage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAgentMessageItemContentItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAgentMessageItemContentItemV2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAgentMessageItemContentItemV2 {
    pub encrypted_content: String,
    #[serde(rename = "type")]
    pub type_: OrAgentMessageItemContentItemV2TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAgentMessageItemContentItemV2TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAgentMessageItemContentItemV2TypeEnum {
    #[serde(rename = "encrypted_content")]
    EncryptedContent,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAgentMessageItemContentItemV2TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAgentMessageItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAgentMessageItemTypeEnum {
    #[serde(rename = "agent_message")]
    AgentMessage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAgentMessageItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicAdvisorMessageUsageIteration`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicAdvisorMessageUsageIteration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<OrAnthropicIterationCacheCreation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i64>,
    pub model: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicAdvisorMessageUsageIterationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicAdvisorMessageUsageIterationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicAdvisorMessageUsageIterationTypeEnum {
    #[serde(rename = "advisor_message")]
    AdvisorMessage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicAdvisorMessageUsageIterationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicAdvisorToolResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicAdvisorToolResult {
    pub content: std::collections::BTreeMap<String, serde_json::Value>,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicAdvisorToolResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicAdvisorToolResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicAdvisorToolResultTypeEnum {
    #[serde(rename = "advisor_tool_result")]
    AdvisorToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicAdvisorToolResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicAllowedCallers`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicAllowedCallers {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicBase64ImageSource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicBase64ImageSource {
    pub data: String,
    pub media_type: OrAnthropicImageMimeType,
    #[serde(rename = "type")]
    pub type_: OrAnthropicBase64ImageSourceTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicBase64ImageSourceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicBase64ImageSourceTypeEnum {
    #[serde(rename = "base64")]
    Base64,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicBase64ImageSourceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicBase64PdfSource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicBase64PdfSource {
    pub data: String,
    pub media_type: OrAnthropicBase64PdfSourceMediaTypeEnum,
    #[serde(rename = "type")]
    pub type_: OrAnthropicBase64PdfSourceTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicBase64PdfSourceMediaTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicBase64PdfSourceMediaTypeEnum {
    #[serde(rename = "application/pdf")]
    ApplicationPdf,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicBase64PdfSourceMediaTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicBase64PdfSourceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicBase64PdfSourceTypeEnum {
    #[serde(rename = "base64")]
    Base64,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicBase64PdfSourceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrAnthropicBashCodeExecutionContent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicBashCodeExecutionContent {
    Variant0(OrAnthropicBashCodeExecutionToolResultError),
    Variant1(OrAnthropicBashCodeExecutionResult),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicBashCodeExecutionContent {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicBashCodeExecutionOutput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicBashCodeExecutionOutput {
    pub file_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicBashCodeExecutionOutputTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicBashCodeExecutionOutputTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicBashCodeExecutionOutputTypeEnum {
    #[serde(rename = "bash_code_execution_output")]
    BashCodeExecutionOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicBashCodeExecutionOutputTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicBashCodeExecutionResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicBashCodeExecutionResult {
    pub content: Vec<OrAnthropicBashCodeExecutionOutput>,
    pub return_code: i64,
    pub stderr: String,
    pub stdout: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicBashCodeExecutionResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicBashCodeExecutionResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicBashCodeExecutionResultTypeEnum {
    #[serde(rename = "bash_code_execution_result")]
    BashCodeExecutionResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicBashCodeExecutionResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicBashCodeExecutionToolResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicBashCodeExecutionToolResult {
    pub content: OrAnthropicBashCodeExecutionContent,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicBashCodeExecutionToolResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicBashCodeExecutionToolResultError`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicBashCodeExecutionToolResultError {
    pub error_code: OrAnthropicBashCodeExecutionToolResultErrorErrorCodeEnum,
    #[serde(rename = "type")]
    pub type_: OrAnthropicBashCodeExecutionToolResultErrorTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicBashCodeExecutionToolResultErrorErrorCodeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicBashCodeExecutionToolResultErrorErrorCodeEnum {
    #[serde(rename = "invalid_tool_input")]
    InvalidToolInput,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "too_many_requests")]
    TooManyRequests,
    #[serde(rename = "execution_time_exceeded")]
    ExecutionTimeExceeded,
    #[serde(rename = "output_file_too_large")]
    OutputFileTooLarge,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicBashCodeExecutionToolResultErrorErrorCodeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicBashCodeExecutionToolResultErrorTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicBashCodeExecutionToolResultErrorTypeEnum {
    #[serde(rename = "bash_code_execution_tool_result_error")]
    BashCodeExecutionToolResultError,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicBashCodeExecutionToolResultErrorTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicBashCodeExecutionToolResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicBashCodeExecutionToolResultTypeEnum {
    #[serde(rename = "bash_code_execution_tool_result")]
    BashCodeExecutionToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicBashCodeExecutionToolResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCacheControlDirective`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCacheControlDirective {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<OrAnthropicCacheControlTtl>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCacheControlDirectiveTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCacheControlDirectiveTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCacheControlDirectiveTypeEnum {
    #[serde(rename = "ephemeral")]
    Ephemeral,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCacheControlDirectiveTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicCacheControlTtl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCacheControlTtl {
    #[serde(rename = "5m")]
    T5m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCacheControlTtl {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCacheCreation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCacheCreation {
    pub ephemeral_1h_input_tokens: i64,
    pub ephemeral_5m_input_tokens: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrAnthropicCaller`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicCaller {
    Variant0(OrAnthropicDirectCaller),
    Variant1(OrAnthropicCodeExecution20250825Caller),
    Variant2(OrAnthropicCodeExecution20260120Caller),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicCaller {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicCitationCharLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationCharLocation {
    pub cited_text: String,
    pub document_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub end_char_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    pub start_char_index: i64,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationCharLocationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicCitationCharLocationParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationCharLocationParam {
    pub cited_text: String,
    pub document_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub end_char_index: i64,
    pub start_char_index: i64,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationCharLocationParamTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCitationCharLocationParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationCharLocationParamTypeEnum {
    #[serde(rename = "char_location")]
    CharLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationCharLocationParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicCitationCharLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationCharLocationTypeEnum {
    #[serde(rename = "char_location")]
    CharLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationCharLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCitationContentBlockLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationContentBlockLocation {
    pub cited_text: String,
    pub document_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub end_block_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    pub start_block_index: i64,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationContentBlockLocationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicCitationContentBlockLocationParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationContentBlockLocationParam {
    pub cited_text: String,
    pub document_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub end_block_index: i64,
    pub start_block_index: i64,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationContentBlockLocationParamTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCitationContentBlockLocationParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationContentBlockLocationParamTypeEnum {
    #[serde(rename = "content_block_location")]
    ContentBlockLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationContentBlockLocationParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicCitationContentBlockLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationContentBlockLocationTypeEnum {
    #[serde(rename = "content_block_location")]
    ContentBlockLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationContentBlockLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCitationPageLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationPageLocation {
    pub cited_text: String,
    pub document_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub end_page_number: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    pub start_page_number: i64,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationPageLocationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicCitationPageLocationParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationPageLocationParam {
    pub cited_text: String,
    pub document_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub end_page_number: i64,
    pub start_page_number: i64,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationPageLocationParamTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCitationPageLocationParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationPageLocationParamTypeEnum {
    #[serde(rename = "page_location")]
    PageLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationPageLocationParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicCitationPageLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationPageLocationTypeEnum {
    #[serde(rename = "page_location")]
    PageLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationPageLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCitationSearchResultLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationSearchResultLocation {
    pub cited_text: String,
    pub end_block_index: i64,
    pub search_result_index: i64,
    pub source: String,
    pub start_block_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationSearchResultLocationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicCitationSearchResultLocationParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationSearchResultLocationParam {
    pub cited_text: String,
    pub end_block_index: i64,
    pub search_result_index: i64,
    pub source: String,
    pub start_block_index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationSearchResultLocationParamTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCitationSearchResultLocationParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationSearchResultLocationParamTypeEnum {
    #[serde(rename = "search_result_location")]
    SearchResultLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationSearchResultLocationParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicCitationSearchResultLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationSearchResultLocationTypeEnum {
    #[serde(rename = "search_result_location")]
    SearchResultLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationSearchResultLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCitationWebSearchResultLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationWebSearchResultLocation {
    pub cited_text: String,
    pub encrypted_index: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationWebSearchResultLocationTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicCitationWebSearchResultLocationParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationWebSearchResultLocationParam {
    pub cited_text: String,
    pub encrypted_index: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCitationWebSearchResultLocationParamTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCitationWebSearchResultLocationParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationWebSearchResultLocationParamTypeEnum {
    #[serde(rename = "web_search_result_location")]
    WebSearchResultLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationWebSearchResultLocationParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicCitationWebSearchResultLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCitationWebSearchResultLocationTypeEnum {
    #[serde(rename = "web_search_result_location")]
    WebSearchResultLocation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCitationWebSearchResultLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCitationsConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCitationsConfig {
    pub enabled: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicCodeExecution20250825Caller`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCodeExecution20250825Caller {
    pub tool_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCodeExecution20250825CallerTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCodeExecution20250825CallerTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCodeExecution20250825CallerTypeEnum {
    #[serde(rename = "code_execution_20250825")]
    CodeExecution20250825,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCodeExecution20250825CallerTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCodeExecution20260120Caller`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCodeExecution20260120Caller {
    pub tool_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCodeExecution20260120CallerTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCodeExecution20260120CallerTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCodeExecution20260120CallerTypeEnum {
    #[serde(rename = "code_execution_20260120")]
    CodeExecution20260120,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCodeExecution20260120CallerTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrAnthropicCodeExecutionContent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicCodeExecutionContent {
    Variant0(OrAnthropicCodeExecutionToolResultError),
    Variant1(OrAnthropicCodeExecutionResult),
    Variant2(OrAnthropicEncryptedCodeExecutionResult),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicCodeExecutionContent {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicCodeExecutionOutput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCodeExecutionOutput {
    pub file_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCodeExecutionOutputTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCodeExecutionOutputTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCodeExecutionOutputTypeEnum {
    #[serde(rename = "code_execution_output")]
    CodeExecutionOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCodeExecutionOutputTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCodeExecutionResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCodeExecutionResult {
    pub content: Vec<OrAnthropicCodeExecutionOutput>,
    pub return_code: i64,
    pub stderr: String,
    pub stdout: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCodeExecutionResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCodeExecutionResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCodeExecutionResultTypeEnum {
    #[serde(rename = "code_execution_result")]
    CodeExecutionResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCodeExecutionResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCodeExecutionToolResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCodeExecutionToolResult {
    pub content: OrAnthropicCodeExecutionContent,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCodeExecutionToolResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicCodeExecutionToolResultError`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCodeExecutionToolResultError {
    pub error_code: OrAnthropicServerToolErrorCode,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCodeExecutionToolResultErrorTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCodeExecutionToolResultErrorTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCodeExecutionToolResultErrorTypeEnum {
    #[serde(rename = "code_execution_tool_result_error")]
    CodeExecutionToolResultError,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCodeExecutionToolResultErrorTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicCodeExecutionToolResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCodeExecutionToolResultTypeEnum {
    #[serde(rename = "code_execution_tool_result")]
    CodeExecutionToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCodeExecutionToolResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCompactionBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCompactionBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCompactionBlockTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCompactionBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCompactionBlockTypeEnum {
    #[serde(rename = "compaction")]
    Compaction,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCompactionBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicCompactionUsageIteration`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicCompactionUsageIteration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<OrAnthropicIterationCacheCreation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i64>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicCompactionUsageIterationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicCompactionUsageIterationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicCompactionUsageIterationTypeEnum {
    #[serde(rename = "compaction")]
    Compaction,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicCompactionUsageIterationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicContainer`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicContainer {
    pub expires_at: String,
    pub id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicContainerUpload`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicContainerUpload {
    pub file_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicContainerUploadTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicContainerUploadTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicContainerUploadTypeEnum {
    #[serde(rename = "container_upload")]
    ContainerUpload,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicContainerUploadTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicDirectCaller`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicDirectCaller {
    #[serde(rename = "type")]
    pub type_: OrAnthropicDirectCallerTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicDirectCallerTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicDirectCallerTypeEnum {
    #[serde(rename = "direct")]
    Direct,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicDirectCallerTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicDocumentBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicDocumentBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<OrAnthropicCitationsConfig>,
    pub source: OrAnthropicDocumentBlockSourceUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicDocumentBlockTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicDocumentBlockParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicDocumentBlockParam {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<OrAnthropicDocumentBlockParamCitations>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub source: OrAnthropicDocumentBlockParamSourceUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicDocumentBlockParamTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicDocumentBlockParamCitations`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicDocumentBlockParamCitations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrAnthropicDocumentBlockParamSourceUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicDocumentBlockParamSourceUnion {
    Variant0(OrAnthropicBase64PdfSource),
    Variant1(OrAnthropicPlainTextSource),
    Variant2(OrAnthropicDocumentBlockParamSourceV2),
    Variant3(OrAnthropicUrlPdfSource),
    Variant4(OrAnthropicFileDocumentSource),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicDocumentBlockParamSourceUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicDocumentBlockParamSourceV2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicDocumentBlockParamSourceV2 {
    pub content: OrAnthropicDocumentBlockParamSourceV2ContentUnion,
    #[serde(rename = "type")]
    pub type_: OrAnthropicDocumentBlockParamSourceV2TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrAnthropicDocumentBlockParamSourceV2ContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicDocumentBlockParamSourceV2ContentUnion {
    Variant0(String),
    Variant1(Vec<OrAnthropicDocumentBlockParamSourceV2ContentV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicDocumentBlockParamSourceV2ContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrAnthropicDocumentBlockParamSourceV2ContentV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicDocumentBlockParamSourceV2ContentV1ItemUnion {
    Variant0(OrAnthropicTextBlockParam),
    Variant1(OrAnthropicImageBlockParam),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicDocumentBlockParamSourceV2ContentV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrAnthropicDocumentBlockParamSourceV2TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicDocumentBlockParamSourceV2TypeEnum {
    #[serde(rename = "content")]
    Content,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicDocumentBlockParamSourceV2TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicDocumentBlockParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicDocumentBlockParamTypeEnum {
    #[serde(rename = "document")]
    Document,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicDocumentBlockParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrAnthropicDocumentBlockSourceUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicDocumentBlockSourceUnion {
    Variant0(OrAnthropicBase64PdfSource),
    Variant1(OrAnthropicPlainTextSource),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicDocumentBlockSourceUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrAnthropicDocumentBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicDocumentBlockTypeEnum {
    #[serde(rename = "document")]
    Document,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicDocumentBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicEncryptedCodeExecutionResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicEncryptedCodeExecutionResult {
    pub content: Vec<OrAnthropicCodeExecutionOutput>,
    pub encrypted_stdout: String,
    pub return_code: i64,
    pub stderr: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicEncryptedCodeExecutionResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicEncryptedCodeExecutionResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicEncryptedCodeExecutionResultTypeEnum {
    #[serde(rename = "encrypted_code_execution_result")]
    EncryptedCodeExecutionResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicEncryptedCodeExecutionResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicFileDocumentSource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicFileDocumentSource {
    pub file_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicFileDocumentSourceTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicFileDocumentSourceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicFileDocumentSourceTypeEnum {
    #[serde(rename = "file")]
    File,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicFileDocumentSourceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicImageBlockParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicImageBlockParam {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    pub source: OrAnthropicImageBlockParamSourceUnion,
    #[serde(rename = "type")]
    pub type_: OrAnthropicImageBlockParamTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrAnthropicImageBlockParamSourceUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicImageBlockParamSourceUnion {
    Variant0(OrAnthropicBase64ImageSource),
    Variant1(OrAnthropicUrlImageSource),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicImageBlockParamSourceUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrAnthropicImageBlockParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicImageBlockParamTypeEnum {
    #[serde(rename = "image")]
    Image,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicImageBlockParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicImageMimeType`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicImageMimeType {
    #[serde(rename = "image/jpeg")]
    ImageJpeg,
    #[serde(rename = "image/png")]
    ImagePng,
    #[serde(rename = "image/gif")]
    ImageGif,
    #[serde(rename = "image/webp")]
    ImageWebp,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicImageMimeType {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicInputTokensClearAtLeast`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicInputTokensClearAtLeast {
    #[serde(rename = "type")]
    pub type_: OrAnthropicInputTokensClearAtLeastTypeEnum,
    pub value: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicInputTokensClearAtLeastTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicInputTokensClearAtLeastTypeEnum {
    #[serde(rename = "input_tokens")]
    InputTokens,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicInputTokensClearAtLeastTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicInputTokensTrigger`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicInputTokensTrigger {
    #[serde(rename = "type")]
    pub type_: OrAnthropicInputTokensTriggerTypeEnum,
    pub value: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicInputTokensTriggerTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicInputTokensTriggerTypeEnum {
    #[serde(rename = "input_tokens")]
    InputTokens,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicInputTokensTriggerTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicIterationCacheCreation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicIterationCacheCreation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral_1h_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral_5m_input_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicMessageUsageIteration`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicMessageUsageIteration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<OrAnthropicIterationCacheCreation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicMessageUsageIterationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicMessageUsageIterationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicMessageUsageIterationTypeEnum {
    #[serde(rename = "message")]
    Message,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicMessageUsageIterationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicOutputTokensDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicOutputTokensDetails {
    pub thinking_tokens: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicPlainTextSource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicPlainTextSource {
    pub data: String,
    pub media_type: OrAnthropicPlainTextSourceMediaTypeEnum,
    #[serde(rename = "type")]
    pub type_: OrAnthropicPlainTextSourceTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicPlainTextSourceMediaTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicPlainTextSourceMediaTypeEnum {
    #[serde(rename = "text/plain")]
    TextPlain,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicPlainTextSourceMediaTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicPlainTextSourceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicPlainTextSourceTypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicPlainTextSourceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicRedactedThinkingBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicRedactedThinkingBlock {
    pub data: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicRedactedThinkingBlockTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicRedactedThinkingBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicRedactedThinkingBlockTypeEnum {
    #[serde(rename = "redacted_thinking")]
    RedactedThinking,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicRedactedThinkingBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicRefusalStopDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicRefusalStopDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<OrAnthropicRefusalStopDetailsCategoryEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicRefusalStopDetailsTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicRefusalStopDetailsCategoryEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicRefusalStopDetailsCategoryEnum {
    #[serde(rename = "cyber")]
    Cyber,
    #[serde(rename = "bio")]
    Bio,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicRefusalStopDetailsCategoryEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicRefusalStopDetailsTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicRefusalStopDetailsTypeEnum {
    #[serde(rename = "refusal")]
    Refusal,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicRefusalStopDetailsTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicSearchResultBlockParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicSearchResultBlockParam {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<OrAnthropicSearchResultBlockParamCitations>,
    pub content: Vec<OrAnthropicTextBlockParam>,
    pub source: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicSearchResultBlockParamTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicSearchResultBlockParamCitations`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicSearchResultBlockParamCitations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicSearchResultBlockParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicSearchResultBlockParamTypeEnum {
    #[serde(rename = "search_result")]
    SearchResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicSearchResultBlockParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicServerToolErrorCode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicServerToolErrorCode {
    #[serde(rename = "invalid_tool_input")]
    InvalidToolInput,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "too_many_requests")]
    TooManyRequests,
    #[serde(rename = "execution_time_exceeded")]
    ExecutionTimeExceeded,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicServerToolErrorCode {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicServerToolUsage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicServerToolUsage {
    pub web_fetch_requests: i64,
    pub web_search_requests: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicSpeed`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicSpeed {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicTextBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicTextBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<OrAnthropicTextCitation>>,
    pub text: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicTextBlockTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicTextBlockParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicTextBlockParam {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<OrAnthropicTextBlockParamCitationsItemUnion>>,
    pub text: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicTextBlockParamTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrAnthropicTextBlockParamCitationsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicTextBlockParamCitationsItemUnion {
    Variant0(OrAnthropicCitationCharLocationParam),
    Variant1(OrAnthropicCitationPageLocationParam),
    Variant2(OrAnthropicCitationContentBlockLocationParam),
    Variant3(OrAnthropicCitationWebSearchResultLocationParam),
    Variant4(OrAnthropicCitationSearchResultLocationParam),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicTextBlockParamCitationsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrAnthropicTextBlockParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextBlockParamTypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextBlockParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicTextBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextBlockTypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrAnthropicTextCitation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicTextCitation {
    Variant0(OrAnthropicCitationCharLocation),
    Variant1(OrAnthropicCitationPageLocation),
    Variant2(OrAnthropicCitationContentBlockLocation),
    Variant3(OrAnthropicCitationWebSearchResultLocation),
    Variant4(OrAnthropicCitationSearchResultLocation),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicTextCitation {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrAnthropicTextEditorCodeExecutionContent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicTextEditorCodeExecutionContent {
    Variant0(OrAnthropicTextEditorCodeExecutionToolResultError),
    Variant1(OrAnthropicTextEditorCodeExecutionViewResult),
    Variant2(OrAnthropicTextEditorCodeExecutionCreateResult),
    Variant3(OrAnthropicTextEditorCodeExecutionStrReplaceResult),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicTextEditorCodeExecutionContent {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicTextEditorCodeExecutionCreateResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicTextEditorCodeExecutionCreateResult {
    pub is_file_update: bool,
    #[serde(rename = "type")]
    pub type_: OrAnthropicTextEditorCodeExecutionCreateResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicTextEditorCodeExecutionCreateResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextEditorCodeExecutionCreateResultTypeEnum {
    #[serde(rename = "text_editor_code_execution_create_result")]
    TextEditorCodeExecutionCreateResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextEditorCodeExecutionCreateResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicTextEditorCodeExecutionStrReplaceResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicTextEditorCodeExecutionStrReplaceResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_lines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_start: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_lines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_start: Option<i64>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicTextEditorCodeExecutionStrReplaceResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicTextEditorCodeExecutionStrReplaceResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextEditorCodeExecutionStrReplaceResultTypeEnum {
    #[serde(rename = "text_editor_code_execution_str_replace_result")]
    TextEditorCodeExecutionStrReplaceResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextEditorCodeExecutionStrReplaceResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicTextEditorCodeExecutionToolResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicTextEditorCodeExecutionToolResult {
    pub content: OrAnthropicTextEditorCodeExecutionContent,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicTextEditorCodeExecutionToolResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicTextEditorCodeExecutionToolResultError`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicTextEditorCodeExecutionToolResultError {
    pub error_code: OrAnthropicTextEditorCodeExecutionToolResultErrorErrorCodeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicTextEditorCodeExecutionToolResultErrorTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicTextEditorCodeExecutionToolResultErrorErrorCodeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextEditorCodeExecutionToolResultErrorErrorCodeEnum {
    #[serde(rename = "invalid_tool_input")]
    InvalidToolInput,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "too_many_requests")]
    TooManyRequests,
    #[serde(rename = "execution_time_exceeded")]
    ExecutionTimeExceeded,
    #[serde(rename = "file_not_found")]
    FileNotFound,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextEditorCodeExecutionToolResultErrorErrorCodeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicTextEditorCodeExecutionToolResultErrorTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextEditorCodeExecutionToolResultErrorTypeEnum {
    #[serde(rename = "text_editor_code_execution_tool_result_error")]
    TextEditorCodeExecutionToolResultError,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextEditorCodeExecutionToolResultErrorTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicTextEditorCodeExecutionToolResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextEditorCodeExecutionToolResultTypeEnum {
    #[serde(rename = "text_editor_code_execution_tool_result")]
    TextEditorCodeExecutionToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextEditorCodeExecutionToolResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicTextEditorCodeExecutionViewResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicTextEditorCodeExecutionViewResult {
    pub content: String,
    pub file_type: OrAnthropicTextEditorCodeExecutionViewResultFileTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_lines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_lines: Option<i64>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicTextEditorCodeExecutionViewResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicTextEditorCodeExecutionViewResultFileTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextEditorCodeExecutionViewResultFileTypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "pdf")]
    Pdf,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextEditorCodeExecutionViewResultFileTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicTextEditorCodeExecutionViewResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicTextEditorCodeExecutionViewResultTypeEnum {
    #[serde(rename = "text_editor_code_execution_view_result")]
    TextEditorCodeExecutionViewResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicTextEditorCodeExecutionViewResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicThinkingBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicThinkingBlock {
    pub signature: String,
    pub thinking: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicThinkingBlockTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicThinkingBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicThinkingBlockTypeEnum {
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicThinkingBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicThinkingDisplay`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicThinkingDisplay {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicThinkingTurns`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicThinkingTurns {
    #[serde(rename = "type")]
    pub type_: OrAnthropicThinkingTurnsTypeEnum,
    pub value: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicThinkingTurnsTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicThinkingTurnsTypeEnum {
    #[serde(rename = "thinking_turns")]
    ThinkingTurns,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicThinkingTurnsTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicToolReference`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolReference {
    pub tool_name: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolReferenceTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicToolReferenceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolReferenceTypeEnum {
    #[serde(rename = "tool_reference")]
    ToolReference,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolReferenceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrAnthropicToolSearchContent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicToolSearchContent {
    Variant0(OrAnthropicToolSearchResultError),
    Variant1(OrAnthropicToolSearchResult),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicToolSearchContent {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicToolSearchResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolSearchResult {
    pub tool_references: Vec<OrAnthropicToolReference>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolSearchResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicToolSearchResultError`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolSearchResultError {
    pub error_code: OrAnthropicServerToolErrorCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolSearchResultErrorTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicToolSearchResultErrorTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolSearchResultErrorTypeEnum {
    #[serde(rename = "tool_search_tool_result_error")]
    ToolSearchToolResultError,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolSearchResultErrorTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicToolSearchResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolSearchResultTypeEnum {
    #[serde(rename = "tool_search_tool_search_result")]
    ToolSearchToolSearchResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolSearchResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicToolSearchToolBm25`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolSearchToolBm25 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<OrAnthropicAllowedCallers>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    pub name: OrAnthropicToolSearchToolBm25NameEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolSearchToolBm25TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicToolSearchToolBm25NameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolSearchToolBm25NameEnum {
    #[serde(rename = "tool_search_tool_bm25")]
    ToolSearchToolBm25,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolSearchToolBm25NameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicToolSearchToolBm25TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolSearchToolBm25TypeEnum {
    #[serde(rename = "tool_search_tool_bm25_20251119")]
    ToolSearchToolBm2520251119,
    #[serde(rename = "tool_search_tool_bm25")]
    ToolSearchToolBm25,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolSearchToolBm25TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicToolSearchToolRegex`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolSearchToolRegex {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<OrAnthropicAllowedCallers>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    pub name: OrAnthropicToolSearchToolRegexNameEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolSearchToolRegexTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicToolSearchToolRegexNameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolSearchToolRegexNameEnum {
    #[serde(rename = "tool_search_tool_regex")]
    ToolSearchToolRegex,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolSearchToolRegexNameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicToolSearchToolRegexTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolSearchToolRegexTypeEnum {
    #[serde(rename = "tool_search_tool_regex_20251119")]
    ToolSearchToolRegex20251119,
    #[serde(rename = "tool_search_tool_regex")]
    ToolSearchToolRegex,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolSearchToolRegexTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicToolSearchToolResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolSearchToolResult {
    pub content: OrAnthropicToolSearchContent,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolSearchToolResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicToolSearchToolResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolSearchToolResultTypeEnum {
    #[serde(rename = "tool_search_tool_result")]
    ToolSearchToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolSearchToolResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicToolUseBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolUseBlock {
    pub caller: OrAnthropicCaller,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolUseBlockTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicToolUseBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolUseBlockTypeEnum {
    #[serde(rename = "tool_use")]
    ToolUse,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolUseBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicToolUsesKeep`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolUsesKeep {
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolUsesKeepTypeEnum,
    pub value: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicToolUsesKeepTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolUsesKeepTypeEnum {
    #[serde(rename = "tool_uses")]
    ToolUses,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolUsesKeepTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicToolUsesTrigger`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicToolUsesTrigger {
    #[serde(rename = "type")]
    pub type_: OrAnthropicToolUsesTriggerTypeEnum,
    pub value: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicToolUsesTriggerTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicToolUsesTriggerTypeEnum {
    #[serde(rename = "tool_uses")]
    ToolUses,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicToolUsesTriggerTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicUnknownUsageIteration`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicUnknownUsageIteration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<OrAnthropicIterationCacheCreation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i64>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicUrlImageSource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicUrlImageSource {
    #[serde(rename = "type")]
    pub type_: OrAnthropicUrlImageSourceTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicUrlImageSourceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicUrlImageSourceTypeEnum {
    #[serde(rename = "url")]
    Url,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicUrlImageSourceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicUrlPdfSource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicUrlPdfSource {
    #[serde(rename = "type")]
    pub type_: OrAnthropicUrlPdfSourceTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicUrlPdfSourceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicUrlPdfSourceTypeEnum {
    #[serde(rename = "url")]
    Url,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicUrlPdfSourceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrAnthropicUsageIteration`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicUsageIteration {
    Variant0(OrAnthropicCompactionUsageIteration),
    Variant1(OrAnthropicMessageUsageIteration),
    Variant2(OrAnthropicAdvisorMessageUsageIteration),
    Variant3(OrAnthropicUnknownUsageIteration),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicUsageIteration {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicWebFetchBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicWebFetchBlock {
    pub content: OrAnthropicDocumentBlock,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved_at: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicWebFetchBlockTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicWebFetchBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebFetchBlockTypeEnum {
    #[serde(rename = "web_fetch_result")]
    WebFetchResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebFetchBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrAnthropicWebFetchContent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicWebFetchContent {
    Variant0(OrAnthropicWebFetchToolResultError),
    Variant1(OrAnthropicWebFetchBlock),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicWebFetchContent {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicWebFetchToolResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicWebFetchToolResult {
    pub caller: OrAnthropicCaller,
    pub content: OrAnthropicWebFetchContent,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicWebFetchToolResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicWebFetchToolResultError`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicWebFetchToolResultError {
    pub error_code: OrAnthropicWebFetchToolResultErrorErrorCodeEnum,
    #[serde(rename = "type")]
    pub type_: OrAnthropicWebFetchToolResultErrorTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicWebFetchToolResultErrorErrorCodeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebFetchToolResultErrorErrorCodeEnum {
    #[serde(rename = "invalid_tool_input")]
    InvalidToolInput,
    #[serde(rename = "url_too_long")]
    UrlTooLong,
    #[serde(rename = "url_not_allowed")]
    UrlNotAllowed,
    #[serde(rename = "url_not_accessible")]
    UrlNotAccessible,
    #[serde(rename = "unsupported_content_type")]
    UnsupportedContentType,
    #[serde(rename = "too_many_requests")]
    TooManyRequests,
    #[serde(rename = "max_uses_exceeded")]
    MaxUsesExceeded,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebFetchToolResultErrorErrorCodeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicWebFetchToolResultErrorTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebFetchToolResultErrorTypeEnum {
    #[serde(rename = "web_fetch_tool_result_error")]
    WebFetchToolResultError,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebFetchToolResultErrorTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicWebFetchToolResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebFetchToolResultTypeEnum {
    #[serde(rename = "web_fetch_tool_result")]
    WebFetchToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebFetchToolResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicWebSearchResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicWebSearchResult {
    pub encrypted_content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_age: Option<String>,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicWebSearchResultTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAnthropicWebSearchResultBlockParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicWebSearchResultBlockParam {
    pub encrypted_content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_age: Option<String>,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicWebSearchResultBlockParamTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicWebSearchResultBlockParamTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebSearchResultBlockParamTypeEnum {
    #[serde(rename = "web_search_result")]
    WebSearchResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebSearchResultBlockParamTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicWebSearchResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebSearchResultTypeEnum {
    #[serde(rename = "web_search_result")]
    WebSearchResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebSearchResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicWebSearchToolResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicWebSearchToolResult {
    pub caller: OrAnthropicCaller,
    pub content: OrAnthropicWebSearchToolResultContentUnion,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrAnthropicWebSearchToolResultTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrAnthropicWebSearchToolResultContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrAnthropicWebSearchToolResultContentUnion {
    Variant0(Vec<OrAnthropicWebSearchResult>),
    Variant1(OrAnthropicWebSearchToolResultError),
    Unknown(serde_json::Value),
}
impl Default for OrAnthropicWebSearchToolResultContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrAnthropicWebSearchToolResultError`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicWebSearchToolResultError {
    pub error_code: OrAnthropicWebSearchToolResultErrorErrorCodeEnum,
    #[serde(rename = "type")]
    pub type_: OrAnthropicWebSearchToolResultErrorTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicWebSearchToolResultErrorErrorCodeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebSearchToolResultErrorErrorCodeEnum {
    #[serde(rename = "invalid_tool_input")]
    InvalidToolInput,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "max_uses_exceeded")]
    MaxUsesExceeded,
    #[serde(rename = "too_many_requests")]
    TooManyRequests,
    #[serde(rename = "query_too_long")]
    QueryTooLong,
    #[serde(rename = "request_too_large")]
    RequestTooLarge,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebSearchToolResultErrorErrorCodeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicWebSearchToolResultErrorTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebSearchToolResultErrorTypeEnum {
    #[serde(rename = "web_search_tool_result_error")]
    WebSearchToolResultError,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebSearchToolResultErrorTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrAnthropicWebSearchToolResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebSearchToolResultTypeEnum {
    #[serde(rename = "web_search_tool_result")]
    WebSearchToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebSearchToolResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAnthropicWebSearchToolUserLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAnthropicWebSearchToolUserLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrAnthropicWebSearchToolUserLocationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAnthropicWebSearchToolUserLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAnthropicWebSearchToolUserLocationTypeEnum {
    #[serde(rename = "approximate")]
    Approximate,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAnthropicWebSearchToolUserLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrApiErrorType`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApiErrorType {
    #[serde(rename = "context_length_exceeded")]
    ContextLengthExceeded,
    #[serde(rename = "max_tokens_exceeded")]
    MaxTokensExceeded,
    #[serde(rename = "token_limit_exceeded")]
    TokenLimitExceeded,
    #[serde(rename = "string_too_long")]
    StringTooLong,
    #[serde(rename = "authentication")]
    Authentication,
    #[serde(rename = "permission_denied")]
    PermissionDenied,
    #[serde(rename = "payment_required")]
    PaymentRequired,
    #[serde(rename = "rate_limit_exceeded")]
    RateLimitExceeded,
    #[serde(rename = "provider_overloaded")]
    ProviderOverloaded,
    #[serde(rename = "provider_unavailable")]
    ProviderUnavailable,
    #[serde(rename = "invalid_request")]
    InvalidRequest,
    #[serde(rename = "invalid_prompt")]
    InvalidPrompt,
    #[serde(rename = "not_found")]
    NotFound,
    #[serde(rename = "precondition_failed")]
    PreconditionFailed,
    #[serde(rename = "payload_too_large")]
    PayloadTooLarge,
    #[serde(rename = "unprocessable")]
    Unprocessable,
    #[serde(rename = "content_policy_violation")]
    ContentPolicyViolation,
    #[serde(rename = "refusal")]
    Refusal,
    #[serde(rename = "invalid_image")]
    InvalidImage,
    #[serde(rename = "image_too_large")]
    ImageTooLarge,
    #[serde(rename = "image_too_small")]
    ImageTooSmall,
    #[serde(rename = "unsupported_image_format")]
    UnsupportedImageFormat,
    #[serde(rename = "image_not_found")]
    ImageNotFound,
    #[serde(rename = "image_download_failed")]
    ImageDownloadFailed,
    #[serde(rename = "server")]
    Server,
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "unmapped")]
    Unmapped,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApiErrorType {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAppRankingsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAppRankingsItem {
    pub app_id: i64,
    pub app_name: String,
    pub rank: i64,
    pub total_requests: i64,
    pub total_tokens: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrAppRankingsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAppRankingsResponse {
    pub data: Vec<OrAppRankingsItem>,
    pub meta: OrRankingsDailyMeta,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrApplyPatchCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrApplyPatchCallItem {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub operation: OrApplyPatchCallOperation,
    pub status: OrApplyPatchCallStatus,
    #[serde(rename = "type")]
    pub type_: OrApplyPatchCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrApplyPatchCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchCallItemTypeEnum {
    #[serde(rename = "apply_patch_call")]
    ApplyPatchCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrApplyPatchCallOperation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrApplyPatchCallOperation {
    Variant0(OrApplyPatchCreateFileOperation),
    Variant1(OrApplyPatchUpdateFileOperation),
    Variant2(OrApplyPatchDeleteFileOperation),
    Unknown(serde_json::Value),
}
impl Default for OrApplyPatchCallOperation {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrApplyPatchCallOutputItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrApplyPatchCallOutputItem {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub status: OrApplyPatchCallOutputItemStatusEnum,
    #[serde(rename = "type")]
    pub type_: OrApplyPatchCallOutputItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrApplyPatchCallOutputItemStatusEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchCallOutputItemStatusEnum {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchCallOutputItemStatusEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrApplyPatchCallOutputItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchCallOutputItemTypeEnum {
    #[serde(rename = "apply_patch_call_output")]
    ApplyPatchCallOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchCallOutputItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrApplyPatchCallStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchCallStatus {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchCallStatus {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrApplyPatchCreateFileOperation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrApplyPatchCreateFileOperation {
    pub diff: String,
    pub path: String,
    #[serde(rename = "type")]
    pub type_: OrApplyPatchCreateFileOperationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrApplyPatchCreateFileOperationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchCreateFileOperationTypeEnum {
    #[serde(rename = "create_file")]
    CreateFile,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchCreateFileOperationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrApplyPatchDeleteFileOperation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrApplyPatchDeleteFileOperation {
    pub path: String,
    #[serde(rename = "type")]
    pub type_: OrApplyPatchDeleteFileOperationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrApplyPatchDeleteFileOperationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchDeleteFileOperationTypeEnum {
    #[serde(rename = "delete_file")]
    DeleteFile,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchDeleteFileOperationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrApplyPatchEngineEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchEngineEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "openrouter")]
    Openrouter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchEngineEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrApplyPatchServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrApplyPatchServerTool {
    #[serde(rename = "type")]
    pub type_: OrApplyPatchServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrApplyPatchServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrApplyPatchServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrApplyPatchEngineEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrApplyPatchServerToolOpenRouter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrApplyPatchServerToolOpenRouter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrApplyPatchServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrApplyPatchServerToolOpenRouterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrApplyPatchServerToolOpenRouterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchServerToolOpenRouterTypeEnum {
    #[serde(rename = "openrouter:apply_patch")]
    OpenrouterApplyPatch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchServerToolOpenRouterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrApplyPatchServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchServerToolTypeEnum {
    #[serde(rename = "apply_patch")]
    ApplyPatch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrApplyPatchUpdateFileOperation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrApplyPatchUpdateFileOperation {
    pub diff: String,
    pub path: String,
    #[serde(rename = "type")]
    pub type_: OrApplyPatchUpdateFileOperationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrApplyPatchUpdateFileOperationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrApplyPatchUpdateFileOperationTypeEnum {
    #[serde(rename = "update_file")]
    UpdateFile,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrApplyPatchUpdateFileOperationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAutoBetaRouterPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAutoBetaRouterPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_quality_tradeoff: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub id: OrAutoBetaRouterPluginIdEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAutoBetaRouterPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAutoBetaRouterPluginIdEnum {
    #[serde(rename = "auto-beta-router")]
    AutoBetaRouter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAutoBetaRouterPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrAutoRouterPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrAutoRouterPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_quality_tradeoff: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub id: OrAutoRouterPluginIdEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_model: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrAutoRouterPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrAutoRouterPluginIdEnum {
    #[serde(rename = "auto-router")]
    AutoRouter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrAutoRouterPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrBYOKKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBYOKKey {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_api_key_hashes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_user_ids: Option<Vec<String>>,
    pub created_at: String,
    pub disabled: bool,
    pub id: String,
    pub is_fallback: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub provider: OrBYOKProviderSlug,
    pub sort_order: i64,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrBYOKProviderSlug`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrBYOKProviderSlug {
    #[serde(rename = "ai21")]
    Ai21,
    #[serde(rename = "aion-labs")]
    AionLabs,
    #[serde(rename = "akashml")]
    Akashml,
    #[serde(rename = "alibaba")]
    Alibaba,
    #[serde(rename = "amazon-bedrock")]
    AmazonBedrock,
    #[serde(rename = "amazon-nova")]
    AmazonNova,
    #[serde(rename = "ambient")]
    Ambient,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "arcee-ai")]
    ArceeAi,
    #[serde(rename = "atlas-cloud")]
    AtlasCloud,
    #[serde(rename = "avian")]
    Avian,
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "baidu")]
    Baidu,
    #[serde(rename = "baseten")]
    Baseten,
    #[serde(rename = "black-forest-labs")]
    BlackForestLabs,
    #[serde(rename = "byteplus")]
    Byteplus,
    #[serde(rename = "cerebras")]
    Cerebras,
    #[serde(rename = "chutes")]
    Chutes,
    #[serde(rename = "cirrascale")]
    Cirrascale,
    #[serde(rename = "clarifai")]
    Clarifai,
    #[serde(rename = "cloudflare")]
    Cloudflare,
    #[serde(rename = "cohere")]
    Cohere,
    #[serde(rename = "coreweave")]
    Coreweave,
    #[serde(rename = "crusoe")]
    Crusoe,
    #[serde(rename = "darkbloom")]
    Darkbloom,
    #[serde(rename = "decart")]
    Decart,
    #[serde(rename = "deepgram")]
    Deepgram,
    #[serde(rename = "deepinfra")]
    Deepinfra,
    #[serde(rename = "deepseek")]
    Deepseek,
    #[serde(rename = "dekallm")]
    Dekallm,
    #[serde(rename = "digitalocean")]
    Digitalocean,
    #[serde(rename = "featherless")]
    Featherless,
    #[serde(rename = "fireworks")]
    Fireworks,
    #[serde(rename = "fish-audio")]
    FishAudio,
    #[serde(rename = "friendli")]
    Friendli,
    #[serde(rename = "gmicloud")]
    Gmicloud,
    #[serde(rename = "google-ai-studio")]
    GoogleAiStudio,
    #[serde(rename = "google-vertex")]
    GoogleVertex,
    #[serde(rename = "groq")]
    Groq,
    #[serde(rename = "heygen")]
    Heygen,
    #[serde(rename = "inception")]
    Inception,
    #[serde(rename = "inceptron")]
    Inceptron,
    #[serde(rename = "inferact-vllm")]
    InferactVllm,
    #[serde(rename = "inference-net")]
    InferenceNet,
    #[serde(rename = "infermatic")]
    Infermatic,
    #[serde(rename = "inflection")]
    Inflection,
    #[serde(rename = "io-net")]
    IoNet,
    #[serde(rename = "ionstream")]
    Ionstream,
    #[serde(rename = "krea")]
    Krea,
    #[serde(rename = "liquid")]
    Liquid,
    #[serde(rename = "mancer")]
    Mancer,
    #[serde(rename = "mara")]
    Mara,
    #[serde(rename = "meta")]
    Meta,
    #[serde(rename = "minimax")]
    Minimax,
    #[serde(rename = "mistral")]
    Mistral,
    #[serde(rename = "modelrun")]
    Modelrun,
    #[serde(rename = "modular")]
    Modular,
    #[serde(rename = "moonshotai")]
    Moonshotai,
    #[serde(rename = "morph")]
    Morph,
    #[serde(rename = "ncompass")]
    Ncompass,
    #[serde(rename = "nebius")]
    Nebius,
    #[serde(rename = "nex-agi")]
    NexAgi,
    #[serde(rename = "nextbit")]
    Nextbit,
    #[serde(rename = "novita")]
    Novita,
    #[serde(rename = "nvidia")]
    Nvidia,
    #[serde(rename = "open-inference")]
    OpenInference,
    #[serde(rename = "openai")]
    Openai,
    #[serde(rename = "parasail")]
    Parasail,
    #[serde(rename = "perceptron")]
    Perceptron,
    #[serde(rename = "perplexity")]
    Perplexity,
    #[serde(rename = "phala")]
    Phala,
    #[serde(rename = "poolside")]
    Poolside,
    #[serde(rename = "quiver")]
    Quiver,
    #[serde(rename = "recraft")]
    Recraft,
    #[serde(rename = "reka")]
    Reka,
    #[serde(rename = "relace")]
    Relace,
    #[serde(rename = "runway")]
    Runway,
    #[serde(rename = "sail-research")]
    SailResearch,
    #[serde(rename = "sakana")]
    Sakana,
    #[serde(rename = "sakana-ai")]
    SakanaAi,
    #[serde(rename = "sambanova")]
    Sambanova,
    #[serde(rename = "seed")]
    Seed,
    #[serde(rename = "siliconflow")]
    Siliconflow,
    #[serde(rename = "sourceful")]
    Sourceful,
    #[serde(rename = "stepfun")]
    Stepfun,
    #[serde(rename = "streamlake")]
    Streamlake,
    #[serde(rename = "switchpoint")]
    Switchpoint,
    #[serde(rename = "tencent")]
    Tencent,
    #[serde(rename = "tenstorrent")]
    Tenstorrent,
    #[serde(rename = "together")]
    Together,
    #[serde(rename = "upstage")]
    Upstage,
    #[serde(rename = "venice")]
    Venice,
    #[serde(rename = "wafer")]
    Wafer,
    #[serde(rename = "wandb")]
    Wandb,
    #[serde(rename = "wandb-legacy")]
    WandbLegacy,
    #[serde(rename = "xai")]
    Xai,
    #[serde(rename = "xiaomi")]
    Xiaomi,
    #[serde(rename = "z-ai")]
    ZAi,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrBYOKProviderSlug {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrBaseInputs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrBaseInputs {
    Variant0(String),
    Variant1(Vec<BaseInputsV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrBaseInputs {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrBaseReasoningConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBaseReasoningConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<OrReasoningContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<OrReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<OrReasoningMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<OrReasoningSummaryVerbosity>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBashServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBashServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrBashServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrBashServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBashServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBashServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrBashServerToolEngine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<OrBashServerToolEnvironment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sleep_after_seconds: Option<OrSandboxSleepAfterSeconds>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrBashServerToolEngine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrBashServerToolEngine {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "openrouter")]
    Openrouter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrBashServerToolEngine {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrBashServerToolEnvironment`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrBashServerToolEnvironment {
    Variant0(OrContainerAutoEnvironment),
    Variant1(OrContainerReferenceEnvironment),
    Unknown(serde_json::Value),
}
impl Default for OrBashServerToolEnvironment {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrBashServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrBashServerToolTypeEnum {
    #[serde(rename = "openrouter:bash")]
    OpenrouterBash,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrBashServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrBooleanCapability`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBooleanCapability {
    #[serde(rename = "type")]
    pub type_: OrBooleanCapabilityTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrBooleanCapabilityTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrBooleanCapabilityTypeEnum {
    #[serde(rename = "boolean")]
    Boolean,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrBooleanCapabilityTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrBulkAddWorkspaceMembersRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkAddWorkspaceMembersRequest {
    pub user_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkAddWorkspaceMembersResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkAddWorkspaceMembersResponse {
    pub added_count: i64,
    pub data: Vec<OrWorkspaceMember>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkAssignKeysRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkAssignKeysRequest {
    pub key_hashes: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkAssignKeysResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkAssignKeysResponse {
    pub assigned_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkAssignMembersRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkAssignMembersRequest {
    pub member_user_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkAssignMembersResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkAssignMembersResponse {
    pub assigned_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkRemoveWorkspaceMembersRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkRemoveWorkspaceMembersRequest {
    pub user_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkRemoveWorkspaceMembersResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkRemoveWorkspaceMembersResponse {
    pub removed_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkUnassignKeysRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkUnassignKeysRequest {
    pub key_hashes: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkUnassignKeysResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkUnassignKeysResponse {
    pub unassigned_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkUnassignMembersRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkUnassignMembersRequest {
    pub member_user_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrBulkUnassignMembersResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrBulkUnassignMembersResponse {
    pub unassigned_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrCapabilityDescriptor`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrCapabilityDescriptor {
    Variant0(OrEnumCapability),
    Variant1(OrRangeCapability),
    Variant2(OrBooleanCapability),
    Unknown(serde_json::Value),
}
impl Default for OrCapabilityDescriptor {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrChatAssistantImages`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatAssistantImages {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatAssistantMessage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatAssistantMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<OrChatAudioOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<OrChatAssistantMessageContentUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<OrChatAssistantImages>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_details: Option<OrChatReasoningDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    pub role: OrChatAssistantMessageRoleEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OrChatToolCall>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrChatAssistantMessageContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatAssistantMessageContentUnion {
    Variant0(String),
    Variant1(Vec<OrChatContentItems>),
    Unknown(serde_json::Value),
}
impl Default for OrChatAssistantMessageContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrChatAssistantMessageRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatAssistantMessageRoleEnum {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatAssistantMessageRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatAudioOutput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatAudioOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatChoice`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatChoice {
    pub finish_reason: OrChatFinishReasonEnum,
    pub index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<OrChatTokenLogprobs>,
    pub message: OrChatAssistantMessage,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatContentAudio`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentAudio {
    pub input_audio: OrChatContentAudioInputAudio,
    #[serde(rename = "type")]
    pub type_: OrChatContentAudioTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatContentAudioInputAudio`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentAudioInputAudio {
    pub data: String,
    pub format: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatContentAudioTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatContentAudioTypeEnum {
    #[serde(rename = "input_audio")]
    InputAudio,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatContentAudioTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatContentCacheControl`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentCacheControl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<OrAnthropicCacheControlTtl>,
    #[serde(rename = "type")]
    pub type_: OrChatContentCacheControlTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatContentCacheControlTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatContentCacheControlTypeEnum {
    #[serde(rename = "ephemeral")]
    Ephemeral,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatContentCacheControlTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatContentFile`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentFile {
    pub file: OrChatContentFileFile,
    #[serde(rename = "type")]
    pub type_: OrChatContentFileTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatContentFileFile`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentFileFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatContentFileTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatContentFileTypeEnum {
    #[serde(rename = "file")]
    File,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatContentFileTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatContentImage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentImage {
    pub image_url: OrChatContentImageImageUrl,
    #[serde(rename = "type")]
    pub type_: OrChatContentImageTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatContentImageImageUrl`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentImageImageUrl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<OrChatContentImageImageUrlDetailEnum>,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatContentImageImageUrlDetailEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatContentImageImageUrlDetailEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "original")]
    Original,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatContentImageImageUrlDetailEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrChatContentImageTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatContentImageTypeEnum {
    #[serde(rename = "image_url")]
    ImageUrl,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatContentImageTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrChatContentItems`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatContentItems {
    Variant0(OrChatContentText),
    Variant1(OrChatContentImage),
    Variant2(OrChatContentAudio),
    Variant3(OrLegacyChatContentVideo),
    Variant4(OrChatContentVideo),
    Variant5(OrChatContentFile),
    Unknown(serde_json::Value),
}
impl Default for OrChatContentItems {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrChatContentText`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentText {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrChatContentCacheControl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_breakpoint: Option<OrPromptCacheBreakpoint>,
    pub text: String,
    #[serde(rename = "type")]
    pub type_: OrChatContentTextTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatContentTextTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatContentTextTypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatContentTextTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatContentVideo`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentVideo {
    #[serde(rename = "type")]
    pub type_: OrChatContentVideoTypeEnum,
    pub video_url: OrChatContentVideoInput,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatContentVideoInput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatContentVideoInput {
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatContentVideoTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatContentVideoTypeEnum {
    #[serde(rename = "video_url")]
    VideoUrl,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatContentVideoTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatDebugOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatDebugOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub echo_upstream_body: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatDeveloperMessage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatDeveloperMessage {
    pub content: OrChatDeveloperMessageContentUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub role: OrChatDeveloperMessageRoleEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrChatDeveloperMessageContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatDeveloperMessageContentUnion {
    Variant0(String),
    Variant1(Vec<OrChatContentText>),
    Unknown(serde_json::Value),
}
impl Default for OrChatDeveloperMessageContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrChatDeveloperMessageRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatDeveloperMessageRoleEnum {
    #[serde(rename = "developer")]
    Developer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatDeveloperMessageRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatFinishReasonEnum`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatFinishReasonEnum {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatFormatGrammarConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatFormatGrammarConfig {
    pub grammar: String,
    #[serde(rename = "type")]
    pub type_: OrChatFormatGrammarConfigTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatFormatGrammarConfigTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatFormatGrammarConfigTypeEnum {
    #[serde(rename = "grammar")]
    Grammar,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatFormatGrammarConfigTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatFormatJsonObjectConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatFormatJsonObjectConfig {
    #[serde(rename = "type")]
    pub type_: OrChatFormatJsonObjectConfigTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatFormatJsonObjectConfigTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatFormatJsonObjectConfigTypeEnum {
    #[serde(rename = "json_object")]
    JsonObject,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatFormatJsonObjectConfigTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatFormatJsonSchemaConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatFormatJsonSchemaConfig {
    pub json_schema: OrChatJsonSchemaConfig,
    #[serde(rename = "type")]
    pub type_: OrChatFormatJsonSchemaConfigTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatFormatJsonSchemaConfigTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatFormatJsonSchemaConfigTypeEnum {
    #[serde(rename = "json_schema")]
    JsonSchema,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatFormatJsonSchemaConfigTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatFormatPythonConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatFormatPythonConfig {
    #[serde(rename = "type")]
    pub type_: OrChatFormatPythonConfigTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatFormatPythonConfigTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatFormatPythonConfigTypeEnum {
    #[serde(rename = "python")]
    Python,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatFormatPythonConfigTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatFormatTextConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatFormatTextConfig {
    #[serde(rename = "type")]
    pub type_: OrChatFormatTextConfigTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatFormatTextConfigTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatFormatTextConfigTypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatFormatTextConfigTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrChatFunctionTool`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatFunctionTool {
    Variant0(ChatFunctionToolV0),
    Variant1(OrAdvisorServerToolOpenRouter),
    Variant2(OrBashServerTool),
    Variant3(OrDatetimeServerTool),
    Variant4(OrFilesServerTool),
    Variant5(OrFusionServerToolOpenRouter),
    Variant6(OrImageGenerationServerToolOpenRouter),
    Variant7(OrChatSearchModelsServerTool),
    Variant8(OrSubagentServerToolOpenRouter),
    Variant9(OrWebFetchServerTool),
    Variant10(OrOpenRouterWebSearchServerTool),
    Variant11(OrChatWebSearchShorthand),
    Unknown(serde_json::Value),
}
impl Default for OrChatFunctionTool {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrChatJsonSchemaConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatJsonSchemaConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrChatMessages`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatMessages {
    Variant0(OrChatSystemMessage),
    Variant1(OrChatUserMessage),
    Variant2(OrChatDeveloperMessage),
    Variant3(OrChatAssistantMessage),
    Variant4(OrChatToolMessage),
    Unknown(serde_json::Value),
}
impl Default for OrChatMessages {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrChatModelNames`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatModelNames {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatNamedToolChoice`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatNamedToolChoice {
    pub function: OrChatNamedToolChoiceFunction,
    #[serde(rename = "type")]
    pub type_: OrChatNamedToolChoiceTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatNamedToolChoiceFunction`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatNamedToolChoiceFunction {
    pub name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatNamedToolChoiceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatNamedToolChoiceTypeEnum {
    #[serde(rename = "function")]
    Function,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatNamedToolChoiceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatReasoningDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatReasoningDetails {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatReasoningSummaryVerbosityEnum`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatReasoningSummaryVerbosityEnum {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<OrChatDebugOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_config: Option<OrImageConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<std::collections::BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    pub messages: Vec<OrChatMessages>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<OrChatRequestModalitiesItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<OrModelName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<OrChatModelNames>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<OrChatRequestPluginsItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction: Option<OrPrediction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_options: Option<OrPromptCacheOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<OrProviderPreferences>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OrChatRequestReasoning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<OrChatRequestReasoningEffortEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<OrChatRequestResponseFormatUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<OrDeprecatedRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<OrChatRequestServiceTierEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<OrChatRequestStopUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_server_tools_when: Option<OrStopServerToolsWhen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<OrChatStreamOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<OrChatToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OrChatFunctionTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_a: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<OrTraceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatRequestModalitiesItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatRequestModalitiesItemEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "audio")]
    Audio,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatRequestModalitiesItemEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrChatRequestPluginsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatRequestPluginsItemUnion {
    Variant0(OrAutoRouterPlugin),
    Variant1(OrAutoBetaRouterPlugin),
    Variant2(OrModerationPlugin),
    Variant3(OrWebSearchPlugin),
    Variant4(OrWebFetchPlugin),
    Variant5(OrFileParserPlugin),
    Variant6(OrResponseHealingPlugin),
    Variant7(OrContextCompressionPlugin),
    Variant8(OrParetoRouterPlugin),
    Variant9(OrFusionPlugin),
    Unknown(serde_json::Value),
}
impl Default for OrChatRequestPluginsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrChatRequestReasoning`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatRequestReasoning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<OrChatRequestReasoningEffortEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<OrChatReasoningSummaryVerbosityEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatRequestReasoningEffortEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatRequestReasoningEffortEnum {
    #[serde(rename = "max")]
    Max,
    #[serde(rename = "xhigh")]
    Xhigh,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatRequestReasoningEffortEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrChatRequestResponseFormatUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatRequestResponseFormatUnion {
    Variant0(OrChatFormatTextConfig),
    Variant1(OrChatFormatJsonObjectConfig),
    Variant2(OrChatFormatJsonSchemaConfig),
    Variant3(OrChatFormatGrammarConfig),
    Variant4(OrChatFormatPythonConfig),
    Unknown(serde_json::Value),
}
impl Default for OrChatRequestResponseFormatUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrChatRequestServiceTierEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatRequestServiceTierEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "flex")]
    Flex,
    #[serde(rename = "priority")]
    Priority,
    #[serde(rename = "scale")]
    Scale,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatRequestServiceTierEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrChatRequestStopUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatRequestStopUnion {
    Variant0(String),
    Variant1(Vec<String>),
    Unknown(serde_json::Value),
}
impl Default for OrChatRequestStopUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrChatResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatResult {
    pub choices: Vec<OrChatChoice>,
    pub created: i64,
    pub id: String,
    pub model: String,
    pub object: OrChatResultObjectEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter_metadata: Option<OrOpenRouterMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<OrChatUsage>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatResultObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatResultObjectEnum {
    #[serde(rename = "chat.completion")]
    ChatCompletion,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatResultObjectEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatSearchModelsServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatSearchModelsServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrSearchModelsServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrChatSearchModelsServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatSearchModelsServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatSearchModelsServerToolTypeEnum {
    #[serde(rename = "openrouter:experimental__search_models")]
    OpenrouterExperimentalSearchModels,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatSearchModelsServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatServerToolChoice`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatServerToolChoice {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatStreamOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatStreamOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatSystemMessage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatSystemMessage {
    pub content: OrChatSystemMessageContentUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub role: OrChatSystemMessageRoleEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrChatSystemMessageContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatSystemMessageContentUnion {
    Variant0(String),
    Variant1(Vec<OrChatContentText>),
    Unknown(serde_json::Value),
}
impl Default for OrChatSystemMessageContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrChatSystemMessageRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatSystemMessageRoleEnum {
    #[serde(rename = "system")]
    System,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatSystemMessageRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatTokenLogprob`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatTokenLogprob {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i64>>,
    pub logprob: f64,
    pub token: String,
    pub top_logprobs: Vec<OrChatTokenLogprobTopLogprobsItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatTokenLogprobTopLogprobsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatTokenLogprobTopLogprobsItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i64>>,
    pub logprob: f64,
    pub token: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatTokenLogprobs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatTokenLogprobs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<OrChatTokenLogprob>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Vec<OrChatTokenLogprob>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatToolCall`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatToolCall {
    pub function: OrChatToolCallFunction,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: OrChatToolCallTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatToolCallFunction`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatToolCallFunction {
    pub arguments: String,
    pub name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatToolCallTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatToolCallTypeEnum {
    #[serde(rename = "function")]
    Function,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatToolCallTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrChatToolChoice`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatToolChoice {
    Variant0(ChatToolChoiceV0Enum),
    Variant1(ChatToolChoiceV1Enum),
    Variant2(ChatToolChoiceV2Enum),
    Variant3(OrChatNamedToolChoice),
    Variant4(OrChatServerToolChoice),
    Unknown(serde_json::Value),
}
impl Default for OrChatToolChoice {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrChatToolMessage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatToolMessage {
    pub content: OrChatToolMessageContentUnion,
    pub role: OrChatToolMessageRoleEnum,
    pub tool_call_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrChatToolMessageContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatToolMessageContentUnion {
    Variant0(String),
    Variant1(Vec<OrChatContentItems>),
    Unknown(serde_json::Value),
}
impl Default for OrChatToolMessageContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrChatToolMessageRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatToolMessageRoleEnum {
    #[serde(rename = "tool")]
    Tool,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatToolMessageRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatUsage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatUsage {
    pub completion_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<OrChatUsageCompletionTokensDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_details: Option<OrCostDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_byok: Option<bool>,
    pub prompt_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<OrChatUsagePromptTokensDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_tool_use_details: Option<OrServerToolUseDetails>,
    pub total_tokens: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatUsageCompletionTokensDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatUsageCompletionTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_prediction_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_prediction_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatUsagePromptTokensDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatUsagePromptTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrChatUserMessage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatUserMessage {
    pub content: OrChatUserMessageContentUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub role: OrChatUserMessageRoleEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrChatUserMessageContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrChatUserMessageContentUnion {
    Variant0(String),
    Variant1(Vec<OrChatContentItems>),
    Unknown(serde_json::Value),
}
impl Default for OrChatUserMessageContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrChatUserMessageRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatUserMessageRoleEnum {
    #[serde(rename = "user")]
    User,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatUserMessageRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrChatWebSearchShorthand`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrChatWebSearchShorthand {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebSearchEngineEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_characters: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrWebSearchConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<OrSearchQualityLevel>,
    #[serde(rename = "type")]
    pub type_: OrChatWebSearchShorthandTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrWebSearchUserLocationServerTool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrChatWebSearchShorthandTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrChatWebSearchShorthandTypeEnum {
    #[serde(rename = "web_search")]
    WebSearch,
    #[serde(rename = "web_search_preview")]
    WebSearchPreview,
    #[serde(rename = "web_search_preview_2025_03_11")]
    WebSearchPreview20250311,
    #[serde(rename = "web_search_2025_08_26")]
    WebSearch20250826,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrChatWebSearchShorthandTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrCodeInterpreterServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCodeInterpreterServerTool {
    pub container: OrCodeInterpreterServerToolContainerUnion,
    #[serde(rename = "type")]
    pub type_: OrCodeInterpreterServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrCodeInterpreterServerToolContainerUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrCodeInterpreterServerToolContainerUnion {
    Variant0(String),
    Variant1(OrCodeInterpreterServerToolContainerV1),
    Unknown(serde_json::Value),
}
impl Default for OrCodeInterpreterServerToolContainerUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrCodeInterpreterServerToolContainerV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCodeInterpreterServerToolContainerV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<OrCodeInterpreterServerToolContainerV1MemoryLimitEnum>,
    #[serde(rename = "type")]
    pub type_: OrCodeInterpreterServerToolContainerV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCodeInterpreterServerToolContainerV1MemoryLimitEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCodeInterpreterServerToolContainerV1MemoryLimitEnum {
    #[serde(rename = "1g")]
    T1g,
    #[serde(rename = "4g")]
    T4g,
    #[serde(rename = "16g")]
    T16g,
    #[serde(rename = "64g")]
    T64g,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCodeInterpreterServerToolContainerV1MemoryLimitEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrCodeInterpreterServerToolContainerV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCodeInterpreterServerToolContainerV1TypeEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCodeInterpreterServerToolContainerV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrCodeInterpreterServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCodeInterpreterServerToolTypeEnum {
    #[serde(rename = "code_interpreter")]
    CodeInterpreter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCodeInterpreterServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrCodexLocalShellTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCodexLocalShellTool {
    #[serde(rename = "type")]
    pub type_: OrCodexLocalShellToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCodexLocalShellToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCodexLocalShellToolTypeEnum {
    #[serde(rename = "local_shell")]
    LocalShell,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCodexLocalShellToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrCompactionItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCompactionItem {
    pub encrypted_content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrCompactionItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCompactionItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCompactionItemTypeEnum {
    #[serde(rename = "compaction")]
    Compaction,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCompactionItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrCompoundFilter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCompoundFilter {
    pub filters: Vec<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "type")]
    pub type_: OrCompoundFilterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCompoundFilterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCompoundFilterTypeEnum {
    #[serde(rename = "and")]
    And,
    #[serde(rename = "or")]
    Or,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCompoundFilterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrComputerUseServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrComputerUseServerTool {
    pub display_height: i64,
    pub display_width: i64,
    pub environment: OrComputerUseServerToolEnvironmentEnum,
    #[serde(rename = "type")]
    pub type_: OrComputerUseServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrComputerUseServerToolEnvironmentEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrComputerUseServerToolEnvironmentEnum {
    #[serde(rename = "windows")]
    Windows,
    #[serde(rename = "mac")]
    Mac,
    #[serde(rename = "linux")]
    Linux,
    #[serde(rename = "ubuntu")]
    Ubuntu,
    #[serde(rename = "browser")]
    Browser,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrComputerUseServerToolEnvironmentEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrComputerUseServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrComputerUseServerToolTypeEnum {
    #[serde(rename = "computer_use_preview")]
    ComputerUsePreview,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrComputerUseServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContainerAutoEnvironment`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContainerAutoEnvironment {
    #[serde(rename = "type")]
    pub type_: OrContainerAutoEnvironmentTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContainerAutoEnvironmentTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContainerAutoEnvironmentTypeEnum {
    #[serde(rename = "container_auto")]
    ContainerAuto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContainerAutoEnvironmentTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContainerReferenceEnvironment`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContainerReferenceEnvironment {
    pub container_id: String,
    #[serde(rename = "type")]
    pub type_: OrContainerReferenceEnvironmentTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContainerReferenceEnvironmentTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContainerReferenceEnvironmentTypeEnum {
    #[serde(rename = "container_reference")]
    ContainerReference,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContainerReferenceEnvironmentTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrContentFilterAction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentFilterAction {
    #[serde(rename = "redact")]
    Redact,
    #[serde(rename = "block")]
    Block,
    #[serde(rename = "flag")]
    Flag,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentFilterAction {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrContentFilterBuiltinAction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentFilterBuiltinAction {
    #[serde(rename = "redact")]
    Redact,
    #[serde(rename = "block")]
    Block,
    #[serde(rename = "flag")]
    Flag,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentFilterBuiltinAction {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContentFilterBuiltinEntry`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentFilterBuiltinEntry {
    pub action: OrContentFilterBuiltinAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_scope: Option<OrPromptInjectionScanScope>,
    pub slug: OrContentFilterBuiltinSlug,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrContentFilterBuiltinEntryInput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentFilterBuiltinEntryInput {
    pub action: OrContentFilterBuiltinAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_scope: Option<OrPromptInjectionScanScope>,
    pub slug: OrContentFilterBuiltinSlug,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContentFilterBuiltinSlug`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentFilterBuiltinSlug {
    #[serde(rename = "email")]
    Email,
    #[serde(rename = "phone")]
    Phone,
    #[serde(rename = "ssn")]
    Ssn,
    #[serde(rename = "credit-card")]
    CreditCard,
    #[serde(rename = "ip-address")]
    IpAddress,
    #[serde(rename = "person-name")]
    PersonName,
    #[serde(rename = "address")]
    Address,
    #[serde(rename = "regex-prompt-injection")]
    RegexPromptInjection,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentFilterBuiltinSlug {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContentFilterEntry`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentFilterEntry {
    pub action: OrContentFilterAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub pattern: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrContentPartAudio`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartAudio {
    pub audio_url: OrContentPartAudioAudioUrl,
    #[serde(rename = "type")]
    pub type_: OrContentPartAudioTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrContentPartAudioAudioUrl`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartAudioAudioUrl {
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContentPartAudioTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentPartAudioTypeEnum {
    #[serde(rename = "audio_url")]
    AudioUrl,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentPartAudioTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContentPartImage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartImage {
    pub image_url: OrContentPartImageImageUrl,
    #[serde(rename = "type")]
    pub type_: OrContentPartImageTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrContentPartImageImageUrl`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartImageImageUrl {
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContentPartImageTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentPartImageTypeEnum {
    #[serde(rename = "image_url")]
    ImageUrl,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentPartImageTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContentPartInputAudio`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartInputAudio {
    pub input_audio: OrMultimodalMedia,
    #[serde(rename = "type")]
    pub type_: OrContentPartInputAudioTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContentPartInputAudioTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentPartInputAudioTypeEnum {
    #[serde(rename = "input_audio")]
    InputAudio,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentPartInputAudioTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContentPartInputFile`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartInputFile {
    pub input_file: OrMultimodalMedia,
    #[serde(rename = "type")]
    pub type_: OrContentPartInputFileTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContentPartInputFileTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentPartInputFileTypeEnum {
    #[serde(rename = "input_file")]
    InputFile,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentPartInputFileTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContentPartInputVideo`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartInputVideo {
    pub input_video: OrMultimodalMedia,
    #[serde(rename = "type")]
    pub type_: OrContentPartInputVideoTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContentPartInputVideoTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentPartInputVideoTypeEnum {
    #[serde(rename = "input_video")]
    InputVideo,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentPartInputVideoTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContentPartVideo`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartVideo {
    #[serde(rename = "type")]
    pub type_: OrContentPartVideoTypeEnum,
    pub video_url: OrContentPartVideoVideoUrl,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContentPartVideoTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContentPartVideoTypeEnum {
    #[serde(rename = "video_url")]
    VideoUrl,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContentPartVideoTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContentPartVideoVideoUrl`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContentPartVideoVideoUrl {
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrContextCompactionItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContextCompactionItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrContextCompactionItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContextCompactionItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContextCompactionItemTypeEnum {
    #[serde(rename = "context_compaction")]
    ContextCompaction,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContextCompactionItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrContextCompressionEngine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContextCompressionEngine {
    #[serde(rename = "middle-out")]
    MiddleOut,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContextCompressionEngine {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrContextCompressionPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrContextCompressionPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrContextCompressionEngine>,
    pub id: OrContextCompressionPluginIdEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrContextCompressionPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrContextCompressionPluginIdEnum {
    #[serde(rename = "context-compression")]
    ContextCompression,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrContextCompressionPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrCostDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCostDetails {
    pub upstream_inference_completions_cost: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_inference_cost: Option<f64>,
    pub upstream_inference_prompt_cost: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateBYOKKeyRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateBYOKKeyRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_fallback: Option<bool>,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub provider: OrBYOKProviderSlug,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateBYOKKeyResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateBYOKKeyResponse {
    pub data: OrCreateBYOKKeyResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateBYOKKeyResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateBYOKKeyResponseData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_api_key_hashes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_user_ids: Option<Vec<String>>,
    pub created_at: String,
    pub disabled: bool,
    pub id: String,
    pub is_fallback: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub provider: OrBYOKProviderSlug,
    pub sort_order: i64,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateGuardrailRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateGuardrailRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_providers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filter_builtins: Option<Vec<OrContentFilterBuiltinEntryInput>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filters: Option<Vec<OrContentFilterEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_anthropic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_google: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_openai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_other: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_xai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_providers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_byok_in_budgets: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_usd: Option<f64>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_interval: Option<OrGuardrailInterval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateGuardrailResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateGuardrailResponse {
    pub data: OrCreateGuardrailResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateGuardrailResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateGuardrailResponseData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_providers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filter_builtins: Option<Vec<OrContentFilterBuiltinEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filters: Option<Vec<OrContentFilterEntry>>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_anthropic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_google: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_openai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_other: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_xai: Option<bool>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_providers: Option<Vec<String>>,
    pub include_byok_in_budgets: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_usd: Option<f64>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_interval: Option<OrGuardrailInterval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateObservabilityDestinationRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateObservabilityDestinationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_rules: Option<OrObservabilityFilterRulesConfigNullable>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_rate: Option<f64>,
    #[serde(rename = "type")]
    pub type_: OrCreateObservabilityDestinationRequestTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCreateObservabilityDestinationRequestTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCreateObservabilityDestinationRequestTypeEnum {
    #[serde(rename = "arize")]
    Arize,
    #[serde(rename = "braintrust")]
    Braintrust,
    #[serde(rename = "clickhouse")]
    Clickhouse,
    #[serde(rename = "datadog")]
    Datadog,
    #[serde(rename = "grafana")]
    Grafana,
    #[serde(rename = "langfuse")]
    Langfuse,
    #[serde(rename = "langsmith")]
    Langsmith,
    #[serde(rename = "newrelic")]
    Newrelic,
    #[serde(rename = "opik")]
    Opik,
    #[serde(rename = "otel-collector")]
    OtelCollector,
    #[serde(rename = "posthog")]
    Posthog,
    #[serde(rename = "ramp")]
    Ramp,
    #[serde(rename = "s3")]
    S3,
    #[serde(rename = "sentry")]
    Sentry,
    #[serde(rename = "snowflake")]
    Snowflake,
    #[serde(rename = "weave")]
    Weave,
    #[serde(rename = "webhook")]
    Webhook,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCreateObservabilityDestinationRequestTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrCreateObservabilityDestinationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateObservabilityDestinationResponse {
    pub data: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreatePresetFromInferenceResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreatePresetFromInferenceResponse {
    pub data: OrPresetWithDesignatedVersion,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateWorkspaceRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateWorkspaceRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_image_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider_sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_text_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_logging_api_key_ids: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_logging_sampling_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_data_discount_logging_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_observability_broadcast_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_observability_io_logging_enabled: Option<bool>,
    pub name: String,
    pub slug: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateWorkspaceResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateWorkspaceResponse {
    pub data: OrCreateWorkspaceResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCreateWorkspaceResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCreateWorkspaceResponseData {
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    pub default_guardrail_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_image_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider_sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_text_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_logging_api_key_ids: Option<Vec<i64>>,
    pub io_logging_sampling_rate: f64,
    pub is_data_discount_logging_enabled: bool,
    pub is_observability_broadcast_enabled: bool,
    pub is_observability_io_logging_enabled: bool,
    pub name: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCustomTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCustomTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OrCustomToolFormatUnion>,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: OrCustomToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrCustomToolCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCustomToolCallItem {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub input: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrCustomToolCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCustomToolCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCustomToolCallItemTypeEnum {
    #[serde(rename = "custom_tool_call")]
    CustomToolCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCustomToolCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrCustomToolCallOutputItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCustomToolCallOutputItem {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub output: OrCustomToolCallOutputItemOutputUnion,
    #[serde(rename = "type")]
    pub type_: OrCustomToolCallOutputItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrCustomToolCallOutputItemOutputUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrCustomToolCallOutputItemOutputUnion {
    Variant0(String),
    Variant1(Vec<OrCustomToolCallOutputItemOutputV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrCustomToolCallOutputItemOutputUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrCustomToolCallOutputItemOutputV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrCustomToolCallOutputItemOutputV1ItemUnion {
    Variant0(OrInputText),
    Variant1(OrCustomToolCallOutputItemOutputV1ItemV1),
    Variant2(OrInputFile),
    Unknown(serde_json::Value),
}
impl Default for OrCustomToolCallOutputItemOutputV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrCustomToolCallOutputItemOutputV1ItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCustomToolCallOutputItemOutputV1ItemV1 {
    pub detail: OrCustomToolCallOutputItemOutputV1ItemV1DetailEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrCustomToolCallOutputItemOutputV1ItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCustomToolCallOutputItemOutputV1ItemV1DetailEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCustomToolCallOutputItemOutputV1ItemV1DetailEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "original")]
    Original,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCustomToolCallOutputItemOutputV1ItemV1DetailEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrCustomToolCallOutputItemOutputV1ItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCustomToolCallOutputItemOutputV1ItemV1TypeEnum {
    #[serde(rename = "input_image")]
    InputImage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCustomToolCallOutputItemOutputV1ItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrCustomToolCallOutputItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCustomToolCallOutputItemTypeEnum {
    #[serde(rename = "custom_tool_call_output")]
    CustomToolCallOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCustomToolCallOutputItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrCustomToolFormatUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrCustomToolFormatUnion {
    Variant0(OrCustomToolFormatV0),
    Variant1(OrCustomToolFormatV1),
    Unknown(serde_json::Value),
}
impl Default for OrCustomToolFormatUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrCustomToolFormatV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCustomToolFormatV0 {
    #[serde(rename = "type")]
    pub type_: OrCustomToolFormatV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCustomToolFormatV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCustomToolFormatV0TypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCustomToolFormatV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrCustomToolFormatV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrCustomToolFormatV1 {
    pub definition: String,
    pub syntax: OrCustomToolFormatV1SyntaxEnum,
    #[serde(rename = "type")]
    pub type_: OrCustomToolFormatV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrCustomToolFormatV1SyntaxEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCustomToolFormatV1SyntaxEnum {
    #[serde(rename = "lark")]
    Lark,
    #[serde(rename = "regex")]
    Regex,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCustomToolFormatV1SyntaxEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrCustomToolFormatV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCustomToolFormatV1TypeEnum {
    #[serde(rename = "grammar")]
    Grammar,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCustomToolFormatV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrCustomToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrCustomToolTypeEnum {
    #[serde(rename = "custom")]
    Custom,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrCustomToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrDABenchmarkEntry`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDABenchmarkEntry {
    pub arena: String,
    pub category: String,
    pub elo: f64,
    pub rank: i64,
    pub win_rate: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrDatetimeServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDatetimeServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrDatetimeServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrDatetimeServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrDatetimeServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDatetimeServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrDatetimeServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrDatetimeServerToolTypeEnum {
    #[serde(rename = "openrouter:datetime")]
    OpenrouterDatetime,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrDatetimeServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrDefaultParameters`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDefaultParameters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrDeleteBYOKKeyResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDeleteBYOKKeyResponse {
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrDeleteGuardrailResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDeleteGuardrailResponse {
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrDeleteObservabilityDestinationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDeleteObservabilityDestinationResponse {
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrDeleteWorkspaceBudgetResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDeleteWorkspaceBudgetResponse {
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrDeleteWorkspaceResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDeleteWorkspaceResponse {
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrDeprecatedRoute`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrDeprecatedRoute {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrEasyInputMessage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrEasyInputMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<OrEasyInputMessageContentUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<OrEasyInputMessagePhaseUnion>,
    pub role: OrEasyInputMessageRoleUnion,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<OrEasyInputMessageTypeEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrEasyInputMessageContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrEasyInputMessageContentUnion {
    Variant0(Vec<OrEasyInputMessageContentV0ItemUnion>),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for OrEasyInputMessageContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrEasyInputMessageContentV0ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrEasyInputMessageContentV0ItemUnion {
    Variant0(OrInputText),
    Variant1(OrEasyInputMessageContentV0ItemV1),
    Variant2(OrInputFile),
    Variant3(OrInputAudio),
    Variant4(OrInputVideo),
    Unknown(serde_json::Value),
}
impl Default for OrEasyInputMessageContentV0ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrEasyInputMessageContentV0ItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrEasyInputMessageContentV0ItemV1 {
    pub detail: OrEasyInputMessageContentV0ItemV1DetailEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrEasyInputMessageContentV0ItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrEasyInputMessageContentV0ItemV1DetailEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessageContentV0ItemV1DetailEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "original")]
    Original,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessageContentV0ItemV1DetailEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrEasyInputMessageContentV0ItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessageContentV0ItemV1TypeEnum {
    #[serde(rename = "input_image")]
    InputImage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessageContentV0ItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrEasyInputMessagePhaseUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrEasyInputMessagePhaseUnion {
    Variant0(OrEasyInputMessagePhaseV0Enum),
    Variant1(OrEasyInputMessagePhaseV1Enum),
    Unknown(serde_json::Value),
}
impl Default for OrEasyInputMessagePhaseUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrEasyInputMessagePhaseV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessagePhaseV0Enum {
    #[serde(rename = "commentary")]
    Commentary,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessagePhaseV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrEasyInputMessagePhaseV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessagePhaseV1Enum {
    #[serde(rename = "final_answer")]
    FinalAnswer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessagePhaseV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrEasyInputMessageRoleUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrEasyInputMessageRoleUnion {
    Variant0(OrEasyInputMessageRoleV0Enum),
    Variant1(OrEasyInputMessageRoleV1Enum),
    Variant2(OrEasyInputMessageRoleV2Enum),
    Variant3(OrEasyInputMessageRoleV3Enum),
    Unknown(serde_json::Value),
}
impl Default for OrEasyInputMessageRoleUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrEasyInputMessageRoleV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessageRoleV0Enum {
    #[serde(rename = "user")]
    User,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessageRoleV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrEasyInputMessageRoleV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessageRoleV1Enum {
    #[serde(rename = "system")]
    System,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessageRoleV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrEasyInputMessageRoleV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessageRoleV2Enum {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessageRoleV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrEasyInputMessageRoleV3Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessageRoleV3Enum {
    #[serde(rename = "developer")]
    Developer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessageRoleV3Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrEasyInputMessageTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEasyInputMessageTypeEnum {
    #[serde(rename = "message")]
    Message,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEasyInputMessageTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrEndpointInfo`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrEndpointInfo {
    pub model: String,
    pub provider: String,
    pub selected: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrEndpointStatus`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrEndpointStatus {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrEndpointsMetadata`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrEndpointsMetadata {
    pub available: Vec<OrEndpointInfo>,
    pub total: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrEnumCapability`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrEnumCapability {
    #[serde(rename = "type")]
    pub type_: OrEnumCapabilityTypeEnum,
    pub values: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrEnumCapabilityTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrEnumCapabilityTypeEnum {
    #[serde(rename = "enum")]
    Enum,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrEnumCapabilityTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFileCitation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFileCitation {
    pub file_id: String,
    pub filename: String,
    pub index: i64,
    #[serde(rename = "type")]
    pub type_: OrFileCitationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFileCitationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFileCitationTypeEnum {
    #[serde(rename = "file_citation")]
    FileCitation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFileCitationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFileDeleteResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFileDeleteResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: OrFileDeleteResponseTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFileDeleteResponseTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFileDeleteResponseTypeEnum {
    #[serde(rename = "file_deleted")]
    FileDeleted,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFileDeleteResponseTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFileListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFileListResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub data: Vec<OrFileMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFileMetadata`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFileMetadata {
    pub created_at: String,
    pub downloadable: bool,
    pub filename: String,
    pub id: String,
    pub mime_type: String,
    pub size_bytes: i64,
    #[serde(rename = "type")]
    pub type_: OrFileMetadataTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFileMetadataTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFileMetadataTypeEnum {
    #[serde(rename = "file")]
    File,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFileMetadataTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFileParserPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFileParserPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub id: OrFileParserPluginIdEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf: Option<OrPDFParserOptions>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFileParserPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFileParserPluginIdEnum {
    #[serde(rename = "file-parser")]
    FileParser,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFileParserPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFilePath`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFilePath {
    pub file_id: String,
    pub index: i64,
    #[serde(rename = "type")]
    pub type_: OrFilePathTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFilePathTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFilePathTypeEnum {
    #[serde(rename = "file_path")]
    FilePath,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFilePathTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFileSearchServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFileSearchServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<OrFileSearchServerToolFiltersUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_num_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranking_options: Option<OrFileSearchServerToolRankingOptions>,
    #[serde(rename = "type")]
    pub type_: OrFileSearchServerToolTypeEnum,
    pub vector_store_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrFileSearchServerToolFiltersUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFileSearchServerToolFiltersUnion {
    Variant0(OrFileSearchServerToolFiltersV0),
    Variant1(OrCompoundFilter),
    Unknown(serde_json::Value),
}
impl Default for OrFileSearchServerToolFiltersUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrFileSearchServerToolFiltersV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFileSearchServerToolFiltersV0 {
    pub key: String,
    #[serde(rename = "type")]
    pub type_: OrFileSearchServerToolFiltersV0TypeEnum,
    pub value: OrFileSearchServerToolFiltersV0ValueUnion,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFileSearchServerToolFiltersV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFileSearchServerToolFiltersV0TypeEnum {
    #[serde(rename = "eq")]
    Eq,
    #[serde(rename = "ne")]
    Ne,
    #[serde(rename = "gt")]
    Gt,
    #[serde(rename = "gte")]
    Gte,
    #[serde(rename = "lt")]
    Lt,
    #[serde(rename = "lte")]
    Lte,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFileSearchServerToolFiltersV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrFileSearchServerToolFiltersV0ValueUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFileSearchServerToolFiltersV0ValueUnion {
    Variant0(String),
    Variant1(f64),
    Variant2(bool),
    Variant3(Vec<OrFileSearchServerToolFiltersV0ValueV3ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrFileSearchServerToolFiltersV0ValueUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrFileSearchServerToolFiltersV0ValueV3ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFileSearchServerToolFiltersV0ValueV3ItemUnion {
    Variant0(String),
    Variant1(f64),
    Unknown(serde_json::Value),
}
impl Default for OrFileSearchServerToolFiltersV0ValueV3ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrFileSearchServerToolRankingOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFileSearchServerToolRankingOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranker: Option<OrFileSearchServerToolRankingOptionsRankerEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFileSearchServerToolRankingOptionsRankerEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFileSearchServerToolRankingOptionsRankerEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "default-2024-11-15")]
    Default20241115,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFileSearchServerToolRankingOptionsRankerEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrFileSearchServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFileSearchServerToolTypeEnum {
    #[serde(rename = "file_search")]
    FileSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFileSearchServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFilesServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFilesServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrFilesServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrFilesServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFilesServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFilesServerToolConfig {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFilesServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFilesServerToolTypeEnum {
    #[serde(rename = "openrouter:files")]
    OpenrouterFiles,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFilesServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFormatJsonObjectConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFormatJsonObjectConfig {
    #[serde(rename = "type")]
    pub type_: OrFormatJsonObjectConfigTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFormatJsonObjectConfigTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFormatJsonObjectConfigTypeEnum {
    #[serde(rename = "json_object")]
    JsonObject,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFormatJsonObjectConfigTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFormatJsonSchemaConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFormatJsonSchemaConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub name: String,
    pub schema: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrFormatJsonSchemaConfigTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFormatJsonSchemaConfigTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFormatJsonSchemaConfigTypeEnum {
    #[serde(rename = "json_schema")]
    JsonSchema,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFormatJsonSchemaConfigTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFormatTextConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFormatTextConfig {
    #[serde(rename = "type")]
    pub type_: OrFormatTextConfigTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFormatTextConfigTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFormatTextConfigTypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFormatTextConfigTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrFormats`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFormats {
    Variant0(OrFormatTextConfig),
    Variant1(OrFormatJsonObjectConfig),
    Variant2(OrFormatJsonSchemaConfig),
    Unknown(serde_json::Value),
}
impl Default for OrFormats {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrFrameImage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFrameImage {
    pub image_url: OrFrameImageImageUrl,
    #[serde(rename = "type")]
    pub type_: OrFrameImageTypeEnum,
    pub frame_type: OrFrameImageFrameTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFrameImageFrameTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFrameImageFrameTypeEnum {
    #[serde(rename = "first_frame")]
    FirstFrame,
    #[serde(rename = "last_frame")]
    LastFrame,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFrameImageFrameTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFrameImageImageUrl`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFrameImageImageUrl {
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFrameImageTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFrameImageTypeEnum {
    #[serde(rename = "image_url")]
    ImageUrl,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFrameImageTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFunctionCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFunctionCallItem {
    pub arguments: String,
    pub call_id: String,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrToolCallStatus>,
    #[serde(rename = "type")]
    pub type_: OrFunctionCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFunctionCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFunctionCallItemTypeEnum {
    #[serde(rename = "function_call")]
    FunctionCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFunctionCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFunctionCallOutputItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFunctionCallOutputItem {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub output: OrFunctionCallOutputItemOutputUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrToolCallStatus>,
    #[serde(rename = "type")]
    pub type_: OrFunctionCallOutputItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrFunctionCallOutputItemOutputUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFunctionCallOutputItemOutputUnion {
    Variant0(String),
    Variant1(Vec<OrFunctionCallOutputItemOutputV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrFunctionCallOutputItemOutputUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrFunctionCallOutputItemOutputV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFunctionCallOutputItemOutputV1ItemUnion {
    Variant0(OrInputText),
    Variant1(OrFunctionCallOutputItemOutputV1ItemV1),
    Variant2(OrInputFile),
    Unknown(serde_json::Value),
}
impl Default for OrFunctionCallOutputItemOutputV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrFunctionCallOutputItemOutputV1ItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFunctionCallOutputItemOutputV1ItemV1 {
    pub detail: OrFunctionCallOutputItemOutputV1ItemV1DetailEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrFunctionCallOutputItemOutputV1ItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFunctionCallOutputItemOutputV1ItemV1DetailEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFunctionCallOutputItemOutputV1ItemV1DetailEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "original")]
    Original,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFunctionCallOutputItemOutputV1ItemV1DetailEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrFunctionCallOutputItemOutputV1ItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFunctionCallOutputItemOutputV1ItemV1TypeEnum {
    #[serde(rename = "input_image")]
    InputImage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFunctionCallOutputItemOutputV1ItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrFunctionCallOutputItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFunctionCallOutputItemTypeEnum {
    #[serde(rename = "function_call_output")]
    FunctionCallOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFunctionCallOutputItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFusionAnalysisResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionAnalysisResult {
    pub blind_spots: Vec<String>,
    pub consensus: Vec<String>,
    pub contradictions: Vec<OrFusionAnalysisResultContradictionsItem>,
    pub partial_coverage: Vec<OrFusionAnalysisResultPartialCoverageItem>,
    pub unique_insights: Vec<OrFusionAnalysisResultUniqueInsightsItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFusionAnalysisResultContradictionsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionAnalysisResultContradictionsItem {
    pub stances: Vec<OrFusionAnalysisResultContradictionsItemStancesItem>,
    pub topic: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFusionAnalysisResultContradictionsItemStancesItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionAnalysisResultContradictionsItemStancesItem {
    pub model: String,
    pub stance: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFusionAnalysisResultPartialCoverageItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionAnalysisResultPartialCoverageItem {
    pub models: Vec<String>,
    pub point: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFusionAnalysisResultUniqueInsightsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionAnalysisResultUniqueInsightsItem {
    pub insight: String,
    pub model: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFusionPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub id: OrFusionPluginIdEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<OrFusionPluginPresetEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OrFusionPluginToolsItem>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFusionPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFusionPluginIdEnum {
    #[serde(rename = "fusion")]
    Fusion,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFusionPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrFusionPluginPresetEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFusionPluginPresetEnum {
    #[serde(rename = "general-high")]
    GeneralHigh,
    #[serde(rename = "general-budget")]
    GeneralBudget,
    #[serde(rename = "general-fast")]
    GeneralFast,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFusionPluginPresetEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFusionPluginToolsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionPluginToolsItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters:
        Option<std::collections::BTreeMap<String, OrFusionPluginToolsItemParametersValueUnion>>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrFusionPluginToolsItemParametersValueUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFusionPluginToolsItemParametersValueUnion {
    Variant0(String),
    Variant1(f64),
    Variant2(bool),
    Variant4(Vec<OrFusionPluginToolsItemParametersValueV4ItemUnion>),
    Variant5(
        std::collections::BTreeMap<String, OrFusionPluginToolsItemParametersValueV5ValueUnion>,
    ),
    Unknown(serde_json::Value),
}
impl Default for OrFusionPluginToolsItemParametersValueUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrFusionPluginToolsItemParametersValueV4ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFusionPluginToolsItemParametersValueV4ItemUnion {
    Variant0(String),
    Variant1(f64),
    Variant2(bool),
    Unknown(serde_json::Value),
}
impl Default for OrFusionPluginToolsItemParametersValueV4ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrFusionPluginToolsItemParametersValueV5ValueUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrFusionPluginToolsItemParametersValueV5ValueUnion {
    Variant0(String),
    Variant1(f64),
    Variant2(bool),
    Unknown(serde_json::Value),
}
impl Default for OrFusionPluginToolsItemParametersValueV5ValueUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrFusionServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OrFusionServerToolConfigReasoning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OrFusionServerToolConfigToolsItem>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFusionServerToolConfigReasoning`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionServerToolConfigReasoning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<OrFusionServerToolConfigReasoningEffortEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFusionServerToolConfigReasoningEffortEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFusionServerToolConfigReasoningEffortEnum {
    #[serde(rename = "max")]
    Max,
    #[serde(rename = "xhigh")]
    Xhigh,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFusionServerToolConfigReasoningEffortEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFusionServerToolConfigToolsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionServerToolConfigToolsItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrFusionServerToolOpenRouter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionServerToolOpenRouter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrFusionServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrFusionServerToolOpenRouterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrFusionServerToolOpenRouterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrFusionServerToolOpenRouterTypeEnum {
    #[serde(rename = "openrouter:fusion")]
    OpenrouterFusion,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrFusionServerToolOpenRouterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrFusionSource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrFusionSource {
    pub title: String,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGenerationContentData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGenerationContentData {
    pub input: OrGenerationContentDataInputUnion,
    pub output: OrGenerationContentDataOutput,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrGenerationContentDataInputUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrGenerationContentDataInputUnion {
    Variant0(OrGenerationContentDataInputV0),
    Variant1(OrGenerationContentDataInputV1),
    Unknown(serde_json::Value),
}
impl Default for OrGenerationContentDataInputUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrGenerationContentDataInputV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGenerationContentDataInputV0 {
    pub prompt: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGenerationContentDataInputV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGenerationContentDataInputV1 {
    pub messages: Vec<serde_json::Value>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGenerationContentDataOutput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGenerationContentDataOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGenerationContentResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGenerationContentResponse {
    pub data: OrGenerationContentData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGenerationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGenerationResponse {
    pub data: OrGenerationResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGenerationResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGenerationResponseData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_type: Option<OrGenerationResponseDataApiTypeEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_discount: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancelled: Option<bool>,
    pub created_at: String,
    pub data_region: OrGenerationResponseDataDataRegionEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_referer: Option<String>,
    pub id: String,
    pub is_byok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency: Option<f64>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moderation_latency: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_tokens_cached: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_tokens_completion: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_tokens_completion_images: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_tokens_prompt: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_tokens_reasoning: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_fetches: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_input_audio_prompt: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_media_completion: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_media_prompt: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_search_results: Option<i64>,
    pub origin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_responses: Option<Vec<OrProviderResponse>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_cache_source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub router: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streamed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_completion: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_prompt: Option<i64>,
    pub total_cost: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_inference_cost: Option<f64>,
    pub usage: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search_engine: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrGenerationResponseDataApiTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrGenerationResponseDataApiTypeEnum {
    #[serde(rename = "completions")]
    Completions,
    #[serde(rename = "embeddings")]
    Embeddings,
    #[serde(rename = "rerank")]
    Rerank,
    #[serde(rename = "tts")]
    Tts,
    #[serde(rename = "stt")]
    Stt,
    #[serde(rename = "video")]
    Video,
    #[serde(rename = "image")]
    Image,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrGenerationResponseDataApiTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrGenerationResponseDataDataRegionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrGenerationResponseDataDataRegionEnum {
    #[serde(rename = "global")]
    Global,
    #[serde(rename = "europe")]
    Europe,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrGenerationResponseDataDataRegionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrGetBYOKKeyResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetBYOKKeyResponse {
    pub data: OrGetBYOKKeyResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGetBYOKKeyResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetBYOKKeyResponseData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_api_key_hashes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_user_ids: Option<Vec<String>>,
    pub created_at: String,
    pub disabled: bool,
    pub id: String,
    pub is_fallback: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub provider: OrBYOKProviderSlug,
    pub sort_order: i64,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGetGuardrailResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetGuardrailResponse {
    pub data: OrGetGuardrailResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGetGuardrailResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetGuardrailResponseData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_providers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filter_builtins: Option<Vec<OrContentFilterBuiltinEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filters: Option<Vec<OrContentFilterEntry>>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_anthropic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_google: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_openai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_other: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_xai: Option<bool>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_providers: Option<Vec<String>>,
    pub include_byok_in_budgets: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_usd: Option<f64>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_interval: Option<OrGuardrailInterval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGetObservabilityDestinationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetObservabilityDestinationResponse {
    pub data: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGetPresetResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetPresetResponse {
    pub data: OrPresetWithDesignatedVersion,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGetPresetVersionResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetPresetVersionResponse {
    pub data: OrPresetDesignatedVersion,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGetWorkspaceResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetWorkspaceResponse {
    pub data: OrGetWorkspaceResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGetWorkspaceResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGetWorkspaceResponseData {
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    pub default_guardrail_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_image_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider_sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_text_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_logging_api_key_ids: Option<Vec<i64>>,
    pub io_logging_sampling_rate: f64,
    pub is_data_discount_logging_enabled: bool,
    pub is_observability_broadcast_enabled: bool,
    pub is_observability_io_logging_enabled: bool,
    pub name: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGuardrail {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_providers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filter_builtins: Option<Vec<OrContentFilterBuiltinEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filters: Option<Vec<OrContentFilterEntry>>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_anthropic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_google: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_openai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_other: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_xai: Option<bool>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_providers: Option<Vec<String>>,
    pub include_byok_in_budgets: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_usd: Option<f64>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_interval: Option<OrGuardrailInterval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrGuardrailInterval`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrGuardrailInterval {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageConfig {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageEndpoint`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageEndpoint {
    pub allowed_passthrough_parameters: Vec<String>,
    pub pricing: Vec<OrImagePricingEntry>,
    pub provider_name: String,
    pub provider_slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_tag: Option<String>,
    pub supported_parameters: std::collections::BTreeMap<String, OrCapabilityDescriptor>,
    pub supports_streaming: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageGenerationProviderPreferences`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationProviderPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<OrImageGenerationProviderPreferencesIgnoreItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<OrImageGenerationProviderPreferencesOnlyItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<OrImageGenerationProviderPreferencesOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<OrImageGenerationProviderPreferencesOrderItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<OrImageGenerationProviderPreferencesSortUnion>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrImageGenerationProviderPreferencesIgnoreItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrImageGenerationProviderPreferencesIgnoreItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for OrImageGenerationProviderPreferencesIgnoreItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrImageGenerationProviderPreferencesOnlyItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrImageGenerationProviderPreferencesOnlyItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for OrImageGenerationProviderPreferencesOnlyItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrImageGenerationProviderPreferencesOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationProviderPreferencesOptions {
    #[serde(rename = "01ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_01ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai21: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "aion-labs")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aion_labs: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub akashml: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alibaba: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "amazon-bedrock")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amazon_bedrock: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "amazon-nova")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amazon_nova: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anyscale: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "arcee-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcee_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "atlas-cloud")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas_cloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atoma: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avian: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baidu: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseten: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "black-forest-labs")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub black_forest_labs: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byteplus: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centml: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cerebras: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chutes: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cirrascale: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarifai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflare: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohere: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coreweave: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crofai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crucible: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crusoe: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub darkbloom: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decart: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepgram: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepinfra: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepseek: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dekallm: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digitalocean: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enfer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "fake-provider")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fake_provider: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featherless: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fireworks: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "fish-audio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fish_audio: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub friendli: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gmicloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "google-ai-studio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_ai_studio: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "google-vertex")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_vertex: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gopomelo: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groq: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heygen: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huggingface: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperbolic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "hyperbolic-quantized")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperbolic_quantized: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inception: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inceptron: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "inferact-vllm")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inferact_vllm: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "inference-net")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_net: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infermatic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inflection: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inocloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "io-net")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_net: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ionstream: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub klusterai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub krea: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lepton: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lynn: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "lynn-private")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lynn_private: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mancer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "mancer-old")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mancer_old: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mara: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimax: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mistral: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelrun: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modular: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moonshotai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morph: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ncompass: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nebius: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "nex-agi")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nex_agi: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nextbit: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nineteen: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub novita: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub octoai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "open-inference")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_inference: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parasail: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perceptron: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perplexity: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phala: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poolside: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiver: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recraft: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursal: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reka: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relace: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runway: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sail-research")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sail_research: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sakana: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sakana-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sakana_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sambanova: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sambanova-cloaked")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sambanova_cloaked: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sf-compute")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sf_compute: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub siliconflow: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sourceful: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stealth: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stepfun: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streamlake: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switchpoint: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targon: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tencent: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenstorrent: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub together: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "together-lite")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub together_lite: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ubicloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstage: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venice: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wafer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wandb: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "wandb-legacy")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wandb_legacy: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xiaomi: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "z-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrImageGenerationProviderPreferencesOrderItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrImageGenerationProviderPreferencesOrderItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for OrImageGenerationProviderPreferencesOrderItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrImageGenerationProviderPreferencesSortUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrImageGenerationProviderPreferencesSortUnion {
    Variant0(OrProviderSort),
    Variant1(OrProviderSortConfig),
    Unknown(serde_json::Value),
}
impl Default for OrImageGenerationProviderPreferencesSortUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrImageGenerationRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<OrImageGenerationRequestAspectRatioEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<OrImageGenerationRequestBackgroundEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_references: Option<Vec<OrContentPartImage>>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_compression: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_format: Option<OrImageGenerationRequestOutputFormatEnum>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<OrImageGenerationProviderPreferences>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<OrImageGenerationRequestQualityEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<OrImageGenerationRequestResolutionEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrImageGenerationRequestAspectRatioEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationRequestAspectRatioEnum {
    #[serde(rename = "1:1")]
    T11,
    #[serde(rename = "1:2")]
    T12,
    #[serde(rename = "1:4")]
    T14,
    #[serde(rename = "1:8")]
    T18,
    #[serde(rename = "2:1")]
    T21,
    #[serde(rename = "2:3")]
    T23,
    #[serde(rename = "3:2")]
    T32,
    #[serde(rename = "3:4")]
    T34,
    #[serde(rename = "4:1")]
    T41,
    #[serde(rename = "4:3")]
    T43,
    #[serde(rename = "4:5")]
    T45,
    #[serde(rename = "5:4")]
    T54,
    #[serde(rename = "8:1")]
    T81,
    #[serde(rename = "9:16")]
    T916,
    #[serde(rename = "16:9")]
    T169,
    #[serde(rename = "9:19.5")]
    T9195,
    #[serde(rename = "19.5:9")]
    T1959,
    #[serde(rename = "9:20")]
    T920,
    #[serde(rename = "20:9")]
    T209,
    #[serde(rename = "9:21")]
    T921,
    #[serde(rename = "21:9")]
    T219,
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationRequestAspectRatioEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImageGenerationRequestBackgroundEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationRequestBackgroundEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "transparent")]
    Transparent,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationRequestBackgroundEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImageGenerationRequestOutputFormatEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationRequestOutputFormatEnum {
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "jpeg")]
    Jpeg,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "svg")]
    Svg,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationRequestOutputFormatEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImageGenerationRequestQualityEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationRequestQualityEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationRequestQualityEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImageGenerationRequestResolutionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationRequestResolutionEnum {
    #[serde(rename = "512")]
    T512,
    #[serde(rename = "1K")]
    T1K,
    #[serde(rename = "2K")]
    T2K,
    #[serde(rename = "4K")]
    T4K,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationRequestResolutionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrImageGenerationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationResponse {
    pub created: i64,
    pub data: Vec<OrImageGenerationResponseDataItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<OrImageGenerationUsage>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageGenerationResponseDataItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationResponseDataItem {
    pub b64_json: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageGenerationServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<OrImageGenerationServerToolBackgroundEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_fidelity: Option<OrImageGenerationServerToolInputFidelityEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_image_mask: Option<OrImageGenerationServerToolInputImageMask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moderation: Option<OrImageGenerationServerToolModerationEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_compression: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_format: Option<OrImageGenerationServerToolOutputFormatEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<OrImageGenerationServerToolQualityEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrImageGenerationServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrImageGenerationServerToolBackgroundEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationServerToolBackgroundEnum {
    #[serde(rename = "transparent")]
    Transparent,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationServerToolBackgroundEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrImageGenerationServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrImageGenerationServerToolInputFidelityEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationServerToolInputFidelityEnum {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "low")]
    Low,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationServerToolInputFidelityEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrImageGenerationServerToolInputImageMask`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationServerToolInputImageMask {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrImageGenerationServerToolModerationEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationServerToolModerationEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "low")]
    Low,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationServerToolModerationEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrImageGenerationServerToolOpenRouter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationServerToolOpenRouter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrImageGenerationServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrImageGenerationServerToolOpenRouterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrImageGenerationServerToolOpenRouterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationServerToolOpenRouterTypeEnum {
    #[serde(rename = "openrouter:image_generation")]
    OpenrouterImageGeneration,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationServerToolOpenRouterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImageGenerationServerToolOutputFormatEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationServerToolOutputFormatEnum {
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "jpeg")]
    Jpeg,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationServerToolOutputFormatEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImageGenerationServerToolQualityEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationServerToolQualityEnum {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationServerToolQualityEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImageGenerationServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationServerToolTypeEnum {
    #[serde(rename = "image_generation")]
    ImageGeneration,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImageGenerationStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageGenerationStatus {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "generating")]
    Generating,
    #[serde(rename = "failed")]
    Failed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageGenerationStatus {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrImageGenerationUsage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<OrAnthropicCacheCreation>,
    pub completion_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<OrImageGenerationUsageCompletionTokensDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_details: Option<OrCostDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_byok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterations: Option<Vec<OrAnthropicUsageIteration>>,
    pub prompt_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<OrImageGenerationUsagePromptTokensDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_tool_use: Option<OrImageGenerationUsageServerToolUse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<OrAnthropicSpeed>,
    pub total_tokens: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageGenerationUsageCompletionTokensDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationUsageCompletionTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageGenerationUsagePromptTokensDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationUsagePromptTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageGenerationUsageServerToolUse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageGenerationUsageServerToolUse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls_executed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls_requested: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search_requests: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrImageInputModality`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageInputModality {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "video")]
    Video,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageInputModality {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrImageModelArchitecture`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageModelArchitecture {
    pub input_modalities: Vec<OrImageInputModality>,
    pub output_modalities: Vec<OrImageOutputModality>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageModelEndpointsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageModelEndpointsResponse {
    pub endpoints: Vec<OrImageEndpoint>,
    pub id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageModelListItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageModelListItem {
    pub architecture: OrImageModelArchitecture,
    pub created: i64,
    pub description: String,
    pub endpoints: String,
    pub id: String,
    pub name: String,
    pub supported_parameters: OrSupportedParameters,
    pub supports_streaming: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrImageModelsListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImageModelsListResponse {
    pub data: Vec<OrImageModelListItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrImageOutputModality`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImageOutputModality {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "embeddings")]
    Embeddings,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "video")]
    Video,
    #[serde(rename = "rerank")]
    Rerank,
    #[serde(rename = "speech")]
    Speech,
    #[serde(rename = "transcription")]
    Transcription,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImageOutputModality {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrImagePricingEntry`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrImagePricingEntry {
    pub billable: OrImagePricingEntryBillableEnum,
    pub cost_usd: f64,
    pub unit: OrImagePricingEntryUnitEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrImagePricingEntryBillableEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImagePricingEntryBillableEnum {
    #[serde(rename = "output_image")]
    OutputImage,
    #[serde(rename = "input_image")]
    InputImage,
    #[serde(rename = "input_font")]
    InputFont,
    #[serde(rename = "input_reference")]
    InputReference,
    #[serde(rename = "input_text")]
    InputText,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImagePricingEntryBillableEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrImagePricingEntryUnitEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrImagePricingEntryUnitEnum {
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "megapixel")]
    Megapixel,
    #[serde(rename = "token")]
    Token,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrImagePricingEntryUnitEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrIncompleteDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrIncompleteDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<OrIncompleteDetailsReasonEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrIncompleteDetailsReasonEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrIncompleteDetailsReasonEnum {
    #[serde(rename = "max_output_tokens")]
    MaxOutputTokens,
    #[serde(rename = "content_filter")]
    ContentFilter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrIncompleteDetailsReasonEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrInputAudio`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInputAudio {
    pub input_audio: OrInputAudioInputAudio,
    #[serde(rename = "type")]
    pub type_: OrInputAudioTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrInputAudioInputAudio`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInputAudioInputAudio {
    pub data: String,
    pub format: OrInputAudioInputAudioFormatEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrInputAudioInputAudioFormatEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputAudioInputAudioFormatEnum {
    #[serde(rename = "mp3")]
    Mp3,
    #[serde(rename = "wav")]
    Wav,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputAudioInputAudioFormatEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrInputAudioTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputAudioTypeEnum {
    #[serde(rename = "input_audio")]
    InputAudio,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputAudioTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrInputFile`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInputFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrInputFileTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrInputFileTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputFileTypeEnum {
    #[serde(rename = "input_file")]
    InputFile,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputFileTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrInputImage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInputImage {
    pub detail: OrInputImageDetailEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrInputImageTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrInputImageDetailEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputImageDetailEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "original")]
    Original,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputImageDetailEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrInputImageTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputImageTypeEnum {
    #[serde(rename = "input_image")]
    InputImage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputImageTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrInputMessageItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInputMessageItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<OrInputMessageItemContentItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub role: OrInputMessageItemRoleUnion,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<OrInputMessageItemTypeEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrInputMessageItemContentItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrInputMessageItemContentItemUnion {
    Variant0(OrInputText),
    Variant1(OrInputMessageItemContentItemV1),
    Variant2(OrInputFile),
    Variant3(OrInputAudio),
    Variant4(OrInputVideo),
    Unknown(serde_json::Value),
}
impl Default for OrInputMessageItemContentItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrInputMessageItemContentItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInputMessageItemContentItemV1 {
    pub detail: OrInputMessageItemContentItemV1DetailEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrInputMessageItemContentItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrInputMessageItemContentItemV1DetailEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputMessageItemContentItemV1DetailEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "original")]
    Original,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputMessageItemContentItemV1DetailEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrInputMessageItemContentItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputMessageItemContentItemV1TypeEnum {
    #[serde(rename = "input_image")]
    InputImage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputMessageItemContentItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrInputMessageItemRoleUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrInputMessageItemRoleUnion {
    Variant0(OrInputMessageItemRoleV0Enum),
    Variant1(OrInputMessageItemRoleV1Enum),
    Variant2(OrInputMessageItemRoleV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrInputMessageItemRoleUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrInputMessageItemRoleV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputMessageItemRoleV0Enum {
    #[serde(rename = "user")]
    User,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputMessageItemRoleV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrInputMessageItemRoleV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputMessageItemRoleV1Enum {
    #[serde(rename = "system")]
    System,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputMessageItemRoleV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrInputMessageItemRoleV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputMessageItemRoleV2Enum {
    #[serde(rename = "developer")]
    Developer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputMessageItemRoleV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrInputMessageItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputMessageItemTypeEnum {
    #[serde(rename = "message")]
    Message,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputMessageItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrInputModality`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputModality {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "video")]
    Video,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputModality {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrInputReference`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrInputReference {
    Variant0(OrContentPartImage),
    Variant1(OrContentPartAudio),
    Variant2(OrContentPartVideo),
    Unknown(serde_json::Value),
}
impl Default for OrInputReference {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrInputText`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInputText {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_breakpoint: Option<OrPromptCacheBreakpoint>,
    pub text: String,
    #[serde(rename = "type")]
    pub type_: OrInputTextTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrInputTextTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputTextTypeEnum {
    #[serde(rename = "input_text")]
    InputText,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputTextTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrInputVideo`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInputVideo {
    #[serde(rename = "type")]
    pub type_: OrInputVideoTypeEnum,
    pub video_url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrInputVideoTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrInputVideoTypeEnum {
    #[serde(rename = "input_video")]
    InputVideo,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrInputVideoTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrInputs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrInputs {
    Variant0(String),
    Variant1(Vec<InputsV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrInputs {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrInstructType`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrInstructType {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrItemReferenceItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrItemReferenceItem {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: OrItemReferenceItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrItemReferenceItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrItemReferenceItemTypeEnum {
    #[serde(rename = "item_reference")]
    ItemReference,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrItemReferenceItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrKeyAssignment`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrKeyAssignment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_by: Option<String>,
    pub created_at: String,
    pub guardrail_id: String,
    pub id: String,
    pub key_hash: String,
    pub key_label: String,
    pub key_name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrLegacyChatContentVideo`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrLegacyChatContentVideo {
    #[serde(rename = "type")]
    pub type_: OrLegacyChatContentVideoTypeEnum,
    pub video_url: OrLegacyChatContentVideoInput,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrLegacyChatContentVideoInput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrLegacyChatContentVideoInput {
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrLegacyChatContentVideoTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrLegacyChatContentVideoTypeEnum {
    #[serde(rename = "input_video")]
    InputVideo,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrLegacyChatContentVideoTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrLegacyWebSearchServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrLegacyWebSearchServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebSearchEngineEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<OrWebSearchDomainFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<OrSearchContextSizeEnum>,
    #[serde(rename = "type")]
    pub type_: OrLegacyWebSearchServerToolTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrWebSearchUserLocation>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrLegacyWebSearchServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrLegacyWebSearchServerToolTypeEnum {
    #[serde(rename = "web_search")]
    WebSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrLegacyWebSearchServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrListBYOKKeysResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListBYOKKeysResponse {
    pub data: Vec<OrBYOKKey>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListEndpointsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListEndpointsResponse {
    pub architecture: OrListEndpointsResponseArchitecture,
    pub created: i64,
    pub description: String,
    pub endpoints: Vec<OrPublicEndpoint>,
    pub id: String,
    pub name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListEndpointsResponseArchitecture`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListEndpointsResponseArchitecture {
    pub input_modalities: Vec<OrInputModality>,
    pub instruct_type: OrInstructType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modality: Option<String>,
    pub output_modalities: Vec<OrOutputModality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<OrModelGroup>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListGuardrailsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListGuardrailsResponse {
    pub data: Vec<OrGuardrail>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListKeyAssignmentsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListKeyAssignmentsResponse {
    pub data: Vec<OrKeyAssignment>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListMemberAssignmentsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListMemberAssignmentsResponse {
    pub data: Vec<OrMemberAssignment>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListObservabilityDestinationsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListObservabilityDestinationsResponse {
    pub data: Vec<OrObservabilityDestination>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListPresetVersionsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListPresetVersionsResponse {
    pub data: Vec<OrPresetDesignatedVersion>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListPresetsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListPresetsResponse {
    pub data: Vec<OrPreset>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListWorkspaceBudgetsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListWorkspaceBudgetsResponse {
    pub data: Vec<OrWorkspaceBudget>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListWorkspaceMembersResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListWorkspaceMembersResponse {
    pub data: Vec<OrWorkspaceMember>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrListWorkspacesResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrListWorkspacesResponse {
    pub data: Vec<OrWorkspace>,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrLocalShellCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrLocalShellCallItem {
    pub action: OrLocalShellCallItemAction,
    pub call_id: String,
    pub id: String,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrLocalShellCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrLocalShellCallItemAction`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrLocalShellCallItemAction {
    pub command: Vec<String>,
    pub env: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
    #[serde(rename = "type")]
    pub type_: OrLocalShellCallItemActionTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrLocalShellCallItemActionTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrLocalShellCallItemActionTypeEnum {
    #[serde(rename = "exec")]
    Exec,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrLocalShellCallItemActionTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrLocalShellCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrLocalShellCallItemTypeEnum {
    #[serde(rename = "local_shell_call")]
    LocalShellCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrLocalShellCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrLocalShellCallOutputItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrLocalShellCallOutputItem {
    pub id: String,
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrToolCallStatus>,
    #[serde(rename = "type")]
    pub type_: OrLocalShellCallOutputItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrLocalShellCallOutputItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrLocalShellCallOutputItemTypeEnum {
    #[serde(rename = "local_shell_call_output")]
    LocalShellCallOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrLocalShellCallOutputItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMcpApprovalRequestItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpApprovalRequestItem {
    pub arguments: String,
    pub id: String,
    pub name: String,
    pub server_label: String,
    #[serde(rename = "type")]
    pub type_: OrMcpApprovalRequestItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMcpApprovalRequestItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMcpApprovalRequestItemTypeEnum {
    #[serde(rename = "mcp_approval_request")]
    McpApprovalRequest,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMcpApprovalRequestItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMcpApprovalResponseItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpApprovalResponseItem {
    pub approval_request_id: String,
    pub approve: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrMcpApprovalResponseItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMcpApprovalResponseItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMcpApprovalResponseItemTypeEnum {
    #[serde(rename = "mcp_approval_response")]
    McpApprovalResponse,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMcpApprovalResponseItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMcpCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpCallItem {
    pub arguments: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub server_label: String,
    #[serde(rename = "type")]
    pub type_: OrMcpCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMcpCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMcpCallItemTypeEnum {
    #[serde(rename = "mcp_call")]
    McpCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMcpCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMcpListToolsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpListToolsItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub id: String,
    pub server_label: String,
    pub tools: Vec<OrMcpListToolsItemToolsItem>,
    #[serde(rename = "type")]
    pub type_: OrMcpListToolsItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMcpListToolsItemToolsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpListToolsItemToolsItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: std::collections::BTreeMap<String, serde_json::Value>,
    pub name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMcpListToolsItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMcpListToolsItemTypeEnum {
    #[serde(rename = "mcp_list_tools")]
    McpListTools,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMcpListToolsItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMcpServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<OrMcpServerToolAllowedToolsUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<OrMcpServerToolConnectorIdEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_approval: Option<OrMcpServerToolRequireApprovalUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_description: Option<String>,
    pub server_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrMcpServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrMcpServerToolAllowedToolsUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMcpServerToolAllowedToolsUnion {
    Variant0(Vec<String>),
    Variant1(OrMcpServerToolAllowedToolsV1),
    Unknown(serde_json::Value),
}
impl Default for OrMcpServerToolAllowedToolsUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMcpServerToolAllowedToolsV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpServerToolAllowedToolsV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_names: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMcpServerToolConnectorIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMcpServerToolConnectorIdEnum {
    #[serde(rename = "connector_dropbox")]
    ConnectorDropbox,
    #[serde(rename = "connector_gmail")]
    ConnectorGmail,
    #[serde(rename = "connector_googlecalendar")]
    ConnectorGooglecalendar,
    #[serde(rename = "connector_googledrive")]
    ConnectorGoogledrive,
    #[serde(rename = "connector_microsoftteams")]
    ConnectorMicrosoftteams,
    #[serde(rename = "connector_outlookcalendar")]
    ConnectorOutlookcalendar,
    #[serde(rename = "connector_outlookemail")]
    ConnectorOutlookemail,
    #[serde(rename = "connector_sharepoint")]
    ConnectorSharepoint,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMcpServerToolConnectorIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrMcpServerToolRequireApprovalUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMcpServerToolRequireApprovalUnion {
    Variant0(OrMcpServerToolRequireApprovalV0),
    Variant1(OrMcpServerToolRequireApprovalV1Enum),
    Variant2(OrMcpServerToolRequireApprovalV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrMcpServerToolRequireApprovalUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMcpServerToolRequireApprovalV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpServerToolRequireApprovalV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always: Option<OrMcpServerToolRequireApprovalV0Always>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub never: Option<OrMcpServerToolRequireApprovalV0Never>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMcpServerToolRequireApprovalV0Always`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpServerToolRequireApprovalV0Always {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_names: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMcpServerToolRequireApprovalV0Never`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMcpServerToolRequireApprovalV0Never {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_names: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMcpServerToolRequireApprovalV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMcpServerToolRequireApprovalV1Enum {
    #[serde(rename = "always")]
    Always,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMcpServerToolRequireApprovalV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMcpServerToolRequireApprovalV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMcpServerToolRequireApprovalV2Enum {
    #[serde(rename = "never")]
    Never,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMcpServerToolRequireApprovalV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMcpServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMcpServerToolTypeEnum {
    #[serde(rename = "mcp")]
    Mcp,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMcpServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMemberAssignment`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMemberAssignment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_by: Option<String>,
    pub created_at: String,
    pub guardrail_id: String,
    pub id: String,
    pub organization_id: String,
    pub user_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesAdvisorToolResultBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesAdvisorToolResultBlock {
    pub content: std::collections::BTreeMap<String, serde_json::Value>,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesAdvisorToolResultBlockTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesAdvisorToolResultBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesAdvisorToolResultBlockTypeEnum {
    #[serde(rename = "advisor_tool_result")]
    AdvisorToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesAdvisorToolResultBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesFallbackParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesFallbackParam {
    pub model: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesMessageParam`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParam {
    pub content: OrMessagesMessageParamContentUnion,
    pub role: OrMessagesMessageParamRoleEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrMessagesMessageParamContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesMessageParamContentUnion {
    Variant0(String),
    Variant1(Vec<OrMessagesMessageParamContentV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesMessageParamContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrMessagesMessageParamContentV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesMessageParamContentV1ItemUnion {
    Variant0(OrAnthropicTextBlockParam),
    Variant1(OrAnthropicImageBlockParam),
    Variant2(OrAnthropicDocumentBlockParam),
    Variant3(OrMessagesMessageParamContentV1ItemV3),
    Variant4(OrMessagesMessageParamContentV1ItemV4),
    Variant5(OrMessagesMessageParamContentV1ItemV5),
    Variant6(OrMessagesMessageParamContentV1ItemV6),
    Variant7(OrMessagesMessageParamContentV1ItemV7),
    Variant8(OrMessagesMessageParamContentV1ItemV8),
    Variant9(OrAnthropicSearchResultBlockParam),
    Variant10(OrMessagesMessageParamContentV1ItemV10),
    Variant11(OrMessagesAdvisorToolResultBlock),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesMessageParamContentV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV10`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV10 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV10TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV10TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV10TypeEnum {
    #[serde(rename = "compaction")]
    Compaction,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV10TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV3`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV3 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV3TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV3TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV3TypeEnum {
    #[serde(rename = "tool_use")]
    ToolUse,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV3TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV4`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV4 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<OrMessagesMessageParamContentV1ItemV4ContentUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV4TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrMessagesMessageParamContentV1ItemV4ContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesMessageParamContentV1ItemV4ContentUnion {
    Variant0(String),
    Variant1(Vec<OrMessagesMessageParamContentV1ItemV4ContentV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesMessageParamContentV1ItemV4ContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrMessagesMessageParamContentV1ItemV4ContentV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesMessageParamContentV1ItemV4ContentV1ItemUnion {
    Variant0(OrAnthropicTextBlockParam),
    Variant1(OrAnthropicImageBlockParam),
    Variant2(OrMessagesMessageParamContentV1ItemV4ContentV1ItemV2),
    Variant3(OrAnthropicSearchResultBlockParam),
    Variant4(OrAnthropicDocumentBlockParam),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesMessageParamContentV1ItemV4ContentV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV4ContentV1ItemV2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV4ContentV1ItemV2 {
    pub tool_name: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV4ContentV1ItemV2TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV4ContentV1ItemV2TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV4ContentV1ItemV2TypeEnum {
    #[serde(rename = "tool_reference")]
    ToolReference,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV4ContentV1ItemV2TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV4TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV4TypeEnum {
    #[serde(rename = "tool_result")]
    ToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV4TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV5`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV5 {
    pub signature: String,
    pub thinking: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV5TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV5TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV5TypeEnum {
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV5TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV6`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV6 {
    pub data: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV6TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV6TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV6TypeEnum {
    #[serde(rename = "redacted_thinking")]
    RedactedThinking,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV6TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV7`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV7 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV7TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV7TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV7TypeEnum {
    #[serde(rename = "server_tool_use")]
    ServerToolUse,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV7TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV8`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV8 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    pub content: OrMessagesMessageParamContentV1ItemV8ContentUnion,
    pub tool_use_id: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV8TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrMessagesMessageParamContentV1ItemV8ContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesMessageParamContentV1ItemV8ContentUnion {
    Variant0(Vec<OrAnthropicWebSearchResultBlockParam>),
    Variant1(OrMessagesMessageParamContentV1ItemV8ContentV1),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesMessageParamContentV1ItemV8ContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMessagesMessageParamContentV1ItemV8ContentV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesMessageParamContentV1ItemV8ContentV1 {
    pub error_code: OrMessagesMessageParamContentV1ItemV8ContentV1ErrorCodeEnum,
    #[serde(rename = "type")]
    pub type_: OrMessagesMessageParamContentV1ItemV8ContentV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV8ContentV1ErrorCodeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV8ContentV1ErrorCodeEnum {
    #[serde(rename = "invalid_tool_input")]
    InvalidToolInput,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "max_uses_exceeded")]
    MaxUsesExceeded,
    #[serde(rename = "too_many_requests")]
    TooManyRequests,
    #[serde(rename = "query_too_long")]
    QueryTooLong,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV8ContentV1ErrorCodeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV8ContentV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV8ContentV1TypeEnum {
    #[serde(rename = "web_search_tool_result_error")]
    WebSearchToolResultError,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV8ContentV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesMessageParamContentV1ItemV8TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamContentV1ItemV8TypeEnum {
    #[serde(rename = "web_search_tool_result")]
    WebSearchToolResult,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamContentV1ItemV8TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesMessageParamRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesMessageParamRoleEnum {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "system")]
    System,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesMessageParamRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesOutputConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesOutputConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<OrMessagesOutputConfigEffortEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OrMessagesOutputConfigFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_budget: Option<OrMessagesOutputConfigTaskBudget>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesOutputConfigEffortEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesOutputConfigEffortEnum {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    Xhigh,
    #[serde(rename = "max")]
    Max,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesOutputConfigEffortEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesOutputConfigFormat`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesOutputConfigFormat {
    pub schema: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(rename = "type")]
    pub type_: OrMessagesOutputConfigFormatTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesOutputConfigFormatTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesOutputConfigFormatTypeEnum {
    #[serde(rename = "json_schema")]
    JsonSchema,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesOutputConfigFormatTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesOutputConfigTaskBudget`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesOutputConfigTaskBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining: Option<i64>,
    pub total: i64,
    #[serde(rename = "type")]
    pub type_: OrMessagesOutputConfigTaskBudgetTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesOutputConfigTaskBudgetTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesOutputConfigTaskBudgetTypeEnum {
    #[serde(rename = "tokens")]
    Tokens,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesOutputConfigTaskBudgetTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_management: Option<OrMessagesRequestContextManagement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallbacks: Option<Vec<OrMessagesFallbackParam>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<OrMessagesMessageParam>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<OrMessagesRequestMetadata>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OrMessagesOutputConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<OrMessagesRequestPluginsItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<OrProviderPreferences>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<OrDeprecatedRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_server_tools_when: Option<OrStopServerToolsWhen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<OrMessagesRequestSystemUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<OrMessagesRequestThinkingUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<OrMessagesRequestToolChoiceUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OrMessagesRequestToolsItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<OrTraceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesRequestContextManagement`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestContextManagement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<OrMessagesRequestContextManagementEditsItemUnion>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrMessagesRequestContextManagementEditsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestContextManagementEditsItemUnion {
    Variant0(OrMessagesRequestContextManagementEditsItemV0),
    Variant1(OrMessagesRequestContextManagementEditsItemV1),
    Variant2(OrMessagesRequestContextManagementEditsItemV2),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestContextManagementEditsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMessagesRequestContextManagementEditsItemV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestContextManagementEditsItemV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_at_least: Option<OrAnthropicInputTokensClearAtLeast>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_tool_inputs:
        Option<OrMessagesRequestContextManagementEditsItemV0ClearToolInputsUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<OrAnthropicToolUsesKeep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<OrMessagesRequestContextManagementEditsItemV0TriggerUnion>,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestContextManagementEditsItemV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrMessagesRequestContextManagementEditsItemV0ClearToolInputsUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestContextManagementEditsItemV0ClearToolInputsUnion {
    Variant0(bool),
    Variant1(Vec<String>),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestContextManagementEditsItemV0ClearToolInputsUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrMessagesRequestContextManagementEditsItemV0TriggerUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestContextManagementEditsItemV0TriggerUnion {
    Variant0(OrAnthropicInputTokensTrigger),
    Variant1(OrAnthropicToolUsesTrigger),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestContextManagementEditsItemV0TriggerUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrMessagesRequestContextManagementEditsItemV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestContextManagementEditsItemV0TypeEnum {
    #[serde(rename = "clear_tool_uses_20250919")]
    ClearToolUses20250919,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestContextManagementEditsItemV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestContextManagementEditsItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestContextManagementEditsItemV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<OrMessagesRequestContextManagementEditsItemV1KeepUnion>,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestContextManagementEditsItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrMessagesRequestContextManagementEditsItemV1KeepUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestContextManagementEditsItemV1KeepUnion {
    Variant0(OrAnthropicThinkingTurns),
    Variant1(OrMessagesRequestContextManagementEditsItemV1KeepV1),
    Variant2(OrMessagesRequestContextManagementEditsItemV1KeepV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestContextManagementEditsItemV1KeepUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMessagesRequestContextManagementEditsItemV1KeepV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestContextManagementEditsItemV1KeepV1 {
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestContextManagementEditsItemV1KeepV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestContextManagementEditsItemV1KeepV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestContextManagementEditsItemV1KeepV1TypeEnum {
    #[serde(rename = "all")]
    All,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestContextManagementEditsItemV1KeepV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesRequestContextManagementEditsItemV1KeepV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestContextManagementEditsItemV1KeepV2Enum {
    #[serde(rename = "all")]
    All,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestContextManagementEditsItemV1KeepV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesRequestContextManagementEditsItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestContextManagementEditsItemV1TypeEnum {
    #[serde(rename = "clear_thinking_20251015")]
    ClearThinking20251015,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestContextManagementEditsItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestContextManagementEditsItemV2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestContextManagementEditsItemV2 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_after_compaction: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<OrMessagesRequestContextManagementEditsItemV2Trigger>,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestContextManagementEditsItemV2TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesRequestContextManagementEditsItemV2Trigger`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestContextManagementEditsItemV2Trigger {
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestContextManagementEditsItemV2TriggerTypeEnum,
    pub value: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestContextManagementEditsItemV2TriggerTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestContextManagementEditsItemV2TriggerTypeEnum {
    #[serde(rename = "input_tokens")]
    InputTokens,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestContextManagementEditsItemV2TriggerTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesRequestContextManagementEditsItemV2TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestContextManagementEditsItemV2TypeEnum {
    #[serde(rename = "compact_20260112")]
    Compact20260112,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestContextManagementEditsItemV2TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestMetadata`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrMessagesRequestPluginsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestPluginsItemUnion {
    Variant0(OrAutoRouterPlugin),
    Variant1(OrAutoBetaRouterPlugin),
    Variant2(OrModerationPlugin),
    Variant3(OrWebSearchPlugin),
    Variant4(OrWebFetchPlugin),
    Variant5(OrFileParserPlugin),
    Variant6(OrResponseHealingPlugin),
    Variant7(OrContextCompressionPlugin),
    Variant8(OrParetoRouterPlugin),
    Variant9(OrFusionPlugin),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestPluginsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrMessagesRequestSystemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestSystemUnion {
    Variant0(String),
    Variant1(Vec<OrAnthropicTextBlockParam>),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestSystemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrMessagesRequestThinkingUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestThinkingUnion {
    Variant0(OrMessagesRequestThinkingV0),
    Variant1(OrMessagesRequestThinkingV1),
    Variant2(OrMessagesRequestThinkingV2),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestThinkingUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMessagesRequestThinkingV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestThinkingV0 {
    pub budget_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<OrAnthropicThinkingDisplay>,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestThinkingV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestThinkingV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestThinkingV0TypeEnum {
    #[serde(rename = "enabled")]
    Enabled,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestThinkingV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestThinkingV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestThinkingV1 {
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestThinkingV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestThinkingV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestThinkingV1TypeEnum {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestThinkingV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestThinkingV2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestThinkingV2 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<OrAnthropicThinkingDisplay>,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestThinkingV2TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestThinkingV2TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestThinkingV2TypeEnum {
    #[serde(rename = "adaptive")]
    Adaptive,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestThinkingV2TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrMessagesRequestToolChoiceUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestToolChoiceUnion {
    Variant0(OrMessagesRequestToolChoiceV0),
    Variant1(OrMessagesRequestToolChoiceV1),
    Variant2(OrMessagesRequestToolChoiceV2),
    Variant3(OrMessagesRequestToolChoiceV3),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestToolChoiceUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMessagesRequestToolChoiceV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolChoiceV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_parallel_tool_use: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolChoiceV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolChoiceV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolChoiceV0TypeEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolChoiceV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestToolChoiceV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolChoiceV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_parallel_tool_use: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolChoiceV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolChoiceV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolChoiceV1TypeEnum {
    #[serde(rename = "any")]
    Any,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolChoiceV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestToolChoiceV2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolChoiceV2 {
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolChoiceV2TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolChoiceV2TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolChoiceV2TypeEnum {
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolChoiceV2TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestToolChoiceV3`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolChoiceV3 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_parallel_tool_use: Option<bool>,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolChoiceV3TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolChoiceV3TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolChoiceV3TypeEnum {
    #[serde(rename = "tool")]
    Tool,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolChoiceV3TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrMessagesRequestToolsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrMessagesRequestToolsItemUnion {
    Variant0(OrMessagesRequestToolsItemV0),
    Variant1(OrMessagesRequestToolsItemV1),
    Variant2(OrMessagesRequestToolsItemV2),
    Variant3(OrMessagesRequestToolsItemV3),
    Variant4(OrMessagesRequestToolsItemV4),
    Variant5(OrMessagesRequestToolsItemV5),
    Variant6(OrBashServerTool),
    Variant7(OrDatetimeServerTool),
    Variant8(OrImageGenerationServerToolOpenRouter),
    Variant9(OrMessagesSearchModelsServerTool),
    Variant10(OrWebFetchServerTool),
    Variant11(OrOpenRouterWebSearchServerTool),
    Variant12(OrMessagesRequestToolsItemV12),
    Variant13(OrAnthropicToolSearchToolBm25),
    Variant14(OrAnthropicToolSearchToolRegex),
    Unknown(serde_json::Value),
}
impl Default for OrMessagesRequestToolsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrMessagesRequestToolsItemV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolsItemV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: OrMessagesRequestToolsItemV0InputSchema,
    pub name: String,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<OrMessagesRequestToolsItemV0TypeEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesRequestToolsItemV0InputSchema`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolsItemV0InputSchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolsItemV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV0TypeEnum {
    #[serde(rename = "custom")]
    Custom,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestToolsItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolsItemV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    pub name: OrMessagesRequestToolsItemV1NameEnum,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolsItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesRequestToolsItemV12`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolsItemV12 {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolsItemV1NameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV1NameEnum {
    #[serde(rename = "bash")]
    Bash,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV1NameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesRequestToolsItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV1TypeEnum {
    #[serde(rename = "bash_20250124")]
    Bash20250124,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestToolsItemV2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolsItemV2 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    pub name: OrMessagesRequestToolsItemV2NameEnum,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolsItemV2TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolsItemV2NameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV2NameEnum {
    #[serde(rename = "str_replace_editor")]
    StrReplaceEditor,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV2NameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesRequestToolsItemV2TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV2TypeEnum {
    #[serde(rename = "text_editor_20250124")]
    TextEditor20250124,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV2TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestToolsItemV3`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolsItemV3 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    pub name: OrMessagesRequestToolsItemV3NameEnum,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolsItemV3TypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrAnthropicWebSearchToolUserLocation>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolsItemV3NameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV3NameEnum {
    #[serde(rename = "web_search")]
    WebSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV3NameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesRequestToolsItemV3TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV3TypeEnum {
    #[serde(rename = "web_search_20250305")]
    WebSearch20250305,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV3TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestToolsItemV4`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolsItemV4 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<OrAnthropicAllowedCallers>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    pub name: OrMessagesRequestToolsItemV4NameEnum,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolsItemV4TypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrAnthropicWebSearchToolUserLocation>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolsItemV4NameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV4NameEnum {
    #[serde(rename = "web_search")]
    WebSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV4NameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesRequestToolsItemV4TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV4TypeEnum {
    #[serde(rename = "web_search_20260209")]
    WebSearch20260209,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV4TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesRequestToolsItemV5`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesRequestToolsItemV5 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<OrAnthropicAllowedCallers>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caching: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    pub model: String,
    pub name: OrMessagesRequestToolsItemV5NameEnum,
    #[serde(rename = "type")]
    pub type_: OrMessagesRequestToolsItemV5TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesRequestToolsItemV5NameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV5NameEnum {
    #[serde(rename = "advisor")]
    Advisor,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV5NameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesRequestToolsItemV5TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesRequestToolsItemV5TypeEnum {
    #[serde(rename = "advisor_20260301")]
    Advisor20260301,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesRequestToolsItemV5TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesResult {
    pub container: OrAnthropicContainer,
    pub content: Vec<OrORAnthropicContentBlock>,
    pub id: String,
    pub model: String,
    pub role: OrMessagesResultRoleEnum,
    pub stop_details: OrAnthropicRefusalStopDetails,
    pub stop_reason: OrORAnthropicStopReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrMessagesResultTypeEnum,
    pub usage: OrMessagesResultUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_management: Option<OrMessagesResultContextManagement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter_metadata: Option<OrOpenRouterMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<OrProviderName>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesResultContextManagement`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesResultContextManagement {
    pub applied_edits: Vec<OrMessagesResultContextManagementAppliedEditsItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesResultContextManagementAppliedEditsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesResultContextManagementAppliedEditsItem {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesResultRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesResultRoleEnum {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesResultRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrMessagesResultTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesResultTypeEnum {
    #[serde(rename = "message")]
    Message,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesResultTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMessagesResultUsage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesResultUsage {
    pub cache_creation: OrAnthropicCacheCreation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_geo: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub output_tokens_details: OrAnthropicOutputTokensDetails,
    pub server_tool_use: OrAnthropicServerToolUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_details: Option<OrCostDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_byok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterations: Option<Vec<OrAnthropicUsageIteration>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<OrAnthropicSpeed>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrMessagesSearchModelsServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMessagesSearchModelsServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrSearchModelsServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrMessagesSearchModelsServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrMessagesSearchModelsServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrMessagesSearchModelsServerToolTypeEnum {
    #[serde(rename = "openrouter:experimental__search_models")]
    OpenrouterExperimentalSearchModels,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrMessagesSearchModelsServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrModel`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModel {
    pub architecture: OrModelArchitecture,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmarks: Option<OrModelBenchmarks>,
    pub canonical_slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<i64>,
    pub created: i64,
    pub default_parameters: OrDefaultParameters,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hugging_face_id: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knowledge_cutoff: Option<String>,
    pub links: OrModelLinks,
    pub name: String,
    pub per_request_limits: OrPerRequestLimits,
    pub pricing: OrPublicPricing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OrModelReasoning>,
    pub supported_parameters: Vec<OrParameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_voices: Option<Vec<String>>,
    pub top_provider: OrTopProviderInfo,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelArchitecture`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelArchitecture {
    pub input_modalities: Vec<OrInputModality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruct_type: Option<OrInstructType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modality: Option<String>,
    pub output_modalities: Vec<OrOutputModality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<OrModelGroup>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelBenchmarks`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelBenchmarks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artificial_analysis: Option<OrAABenchmarkEntry>,
    pub design_arena: Vec<OrDABenchmarkEntry>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrModelGroup`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrModelGroup {
    #[serde(rename = "Router")]
    Router,
    #[serde(rename = "Media")]
    Media,
    #[serde(rename = "Other")]
    Other,
    #[serde(rename = "GPT")]
    GPT,
    #[serde(rename = "Claude")]
    Claude,
    #[serde(rename = "Gemini")]
    Gemini,
    #[serde(rename = "Gemma")]
    Gemma,
    #[serde(rename = "Grok")]
    Grok,
    #[serde(rename = "Cohere")]
    Cohere,
    #[serde(rename = "Nova")]
    Nova,
    #[serde(rename = "Qwen")]
    Qwen,
    #[serde(rename = "Yi")]
    Yi,
    #[serde(rename = "DeepSeek")]
    DeepSeek,
    #[serde(rename = "Mistral")]
    Mistral,
    #[serde(rename = "Llama2")]
    Llama2,
    #[serde(rename = "Llama3")]
    Llama3,
    #[serde(rename = "Llama4")]
    Llama4,
    #[serde(rename = "PaLM")]
    PaLM,
    #[serde(rename = "RWKV")]
    RWKV,
    #[serde(rename = "Qwen3")]
    Qwen3,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrModelGroup {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrModelLinks`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelLinks {
    pub details: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelName`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelName {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelReasoning`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelReasoning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_enabled: Option<bool>,
    pub mandatory: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_efforts: Option<Vec<OrReasoningEffort>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_max_tokens: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelResponse {
    pub data: OrModel,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelsCountResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelsCountResponse {
    pub data: OrModelsCountResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelsCountResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelsCountResponseData {
    pub count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelsListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelsListResponse {
    pub data: OrModelsListResponseData,
    pub links: OrModelsListResponseLinks,
    pub total_count: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelsListResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelsListResponseData {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModelsListResponseLinks`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModelsListResponseLinks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrModerationPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrModerationPlugin {
    pub id: OrModerationPluginIdEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrModerationPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrModerationPluginIdEnum {
    #[serde(rename = "moderation")]
    Moderation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrModerationPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrMultimodalMedia`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrMultimodalMedia {
    pub data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrNamespaceFunctionTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrNamespaceFunctionTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<Vec<OrNamespaceFunctionToolAllowedCallersItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrNamespaceFunctionToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrNamespaceFunctionToolAllowedCallersItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrNamespaceFunctionToolAllowedCallersItemEnum {
    #[serde(rename = "direct")]
    Direct,
    #[serde(rename = "programmatic")]
    Programmatic,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrNamespaceFunctionToolAllowedCallersItemEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrNamespaceFunctionToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrNamespaceFunctionToolTypeEnum {
    #[serde(rename = "function")]
    Function,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrNamespaceFunctionToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrNamespaceTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrNamespaceTool {
    pub description: String,
    pub name: String,
    pub tools: Vec<OrNamespaceToolToolsItemUnion>,
    #[serde(rename = "type")]
    pub type_: OrNamespaceToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrNamespaceToolToolsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrNamespaceToolToolsItemUnion {
    Variant0(OrNamespaceFunctionTool),
    Variant1(OrCustomTool),
    Unknown(serde_json::Value),
}
impl Default for OrNamespaceToolToolsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrNamespaceToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrNamespaceToolTypeEnum {
    #[serde(rename = "namespace")]
    Namespace,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrNamespaceToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrORAnthropicContentBlock`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrORAnthropicContentBlock {
    Variant0(OrAnthropicTextBlock),
    Variant1(OrAnthropicToolUseBlock),
    Variant2(OrAnthropicThinkingBlock),
    Variant3(OrAnthropicRedactedThinkingBlock),
    Variant4(OrORAnthropicServerToolUseBlock),
    Variant5(OrAnthropicWebSearchToolResult),
    Variant6(OrAnthropicWebFetchToolResult),
    Variant7(OrAnthropicCodeExecutionToolResult),
    Variant8(OrAnthropicBashCodeExecutionToolResult),
    Variant9(OrAnthropicTextEditorCodeExecutionToolResult),
    Variant10(OrAnthropicToolSearchToolResult),
    Variant11(OrAnthropicContainerUpload),
    Variant12(OrAnthropicCompactionBlock),
    Variant13(OrAnthropicAdvisorToolResult),
    Unknown(serde_json::Value),
}
impl Default for OrORAnthropicContentBlock {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrORAnthropicNullableCaller`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrORAnthropicNullableCaller {
    Variant0(OrAnthropicDirectCaller),
    Variant1(OrAnthropicCodeExecution20250825Caller),
    Variant2(OrAnthropicCodeExecution20260120Caller),
    Unknown(serde_json::Value),
}
impl Default for OrORAnthropicNullableCaller {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrORAnthropicServerToolUseBlock`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrORAnthropicServerToolUseBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<OrORAnthropicNullableCaller>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: OrORAnthropicServerToolUseBlockTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrORAnthropicServerToolUseBlockTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrORAnthropicServerToolUseBlockTypeEnum {
    #[serde(rename = "server_tool_use")]
    ServerToolUse,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrORAnthropicServerToolUseBlockTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrORAnthropicStopReason`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrORAnthropicStopReason {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityArizeDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityArizeDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityArizeDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityArizeDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityArizeDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityArizeDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "baseUrl")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(rename = "spaceKey")]
    pub space_key: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityArizeDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityArizeDestinationTypeEnum {
    #[serde(rename = "arize")]
    Arize,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityArizeDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityBraintrustDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityBraintrustDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityBraintrustDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityBraintrustDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityBraintrustDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityBraintrustDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "baseUrl")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityBraintrustDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityBraintrustDestinationTypeEnum {
    #[serde(rename = "braintrust")]
    Braintrust,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityBraintrustDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityClickhouseDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityClickhouseDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityClickhouseDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityClickhouseDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityClickhouseDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityClickhouseDestinationConfig {
    pub database: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    pub host: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    pub username: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityClickhouseDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityClickhouseDestinationTypeEnum {
    #[serde(rename = "clickhouse")]
    Clickhouse,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityClickhouseDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityDatadogDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityDatadogDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityDatadogDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityDatadogDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityDatadogDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityDatadogDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "mlApp")]
    pub ml_app: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityDatadogDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityDatadogDestinationTypeEnum {
    #[serde(rename = "datadog")]
    Datadog,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityDatadogDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrObservabilityDestination`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrObservabilityDestination {
    Variant0(OrObservabilityArizeDestination),
    Variant1(OrObservabilityBraintrustDestination),
    Variant2(OrObservabilityClickhouseDestination),
    Variant3(OrObservabilityDatadogDestination),
    Variant4(OrObservabilityGrafanaDestination),
    Variant5(OrObservabilityLangfuseDestination),
    Variant6(OrObservabilityLangsmithDestination),
    Variant7(OrObservabilityNewrelicDestination),
    Variant8(OrObservabilityOpikDestination),
    Variant9(OrObservabilityOtelCollectorDestination),
    Variant10(OrObservabilityPosthogDestination),
    Variant11(OrObservabilityRampDestination),
    Variant12(OrObservabilityS3Destination),
    Variant13(OrObservabilitySentryDestination),
    Variant14(OrObservabilitySnowflakeDestination),
    Variant15(OrObservabilityWeaveDestination),
    Variant16(OrObservabilityWebhookDestination),
    Unknown(serde_json::Value),
}
impl Default for OrObservabilityDestination {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrObservabilityFilterRuleGroup`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityFilterRuleGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logic: Option<OrObservabilityFilterRuleGroupLogicEnum>,
    pub rules: Vec<OrObservabilityFilterRuleGroupRulesItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityFilterRuleGroupLogicEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityFilterRuleGroupLogicEnum {
    #[serde(rename = "and")]
    And,
    #[serde(rename = "or")]
    Or,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityFilterRuleGroupLogicEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityFilterRuleGroupRulesItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityFilterRuleGroupRulesItem {
    pub field: OrObservabilityFilterRuleGroupRulesItemFieldEnum,
    pub operator: OrObservabilityFilterRuleGroupRulesItemOperatorEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<OrObservabilityFilterRuleGroupRulesItemValueUnion>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityFilterRuleGroupRulesItemFieldEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityFilterRuleGroupRulesItemFieldEnum {
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "provider")]
    Provider,
    #[serde(rename = "session_id")]
    SessionId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_name")]
    ApiKeyName,
    #[serde(rename = "finish_reason")]
    FinishReason,
    #[serde(rename = "input")]
    Input,
    #[serde(rename = "output")]
    Output,
    #[serde(rename = "total_cost")]
    TotalCost,
    #[serde(rename = "total_tokens")]
    TotalTokens,
    #[serde(rename = "prompt_tokens")]
    PromptTokens,
    #[serde(rename = "completion_tokens")]
    CompletionTokens,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityFilterRuleGroupRulesItemFieldEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrObservabilityFilterRuleGroupRulesItemOperatorEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityFilterRuleGroupRulesItemOperatorEnum {
    #[serde(rename = "equals")]
    Equals,
    #[serde(rename = "not_equals")]
    NotEquals,
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "not_contains")]
    NotContains,
    #[serde(rename = "regex")]
    Regex,
    #[serde(rename = "starts_with")]
    StartsWith,
    #[serde(rename = "ends_with")]
    EndsWith,
    #[serde(rename = "gt")]
    Gt,
    #[serde(rename = "lt")]
    Lt,
    #[serde(rename = "gte")]
    Gte,
    #[serde(rename = "lte")]
    Lte,
    #[serde(rename = "exists")]
    Exists,
    #[serde(rename = "not_exists")]
    NotExists,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityFilterRuleGroupRulesItemOperatorEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrObservabilityFilterRuleGroupRulesItemValueUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrObservabilityFilterRuleGroupRulesItemValueUnion {
    Variant0(String),
    Variant1(f64),
    Unknown(serde_json::Value),
}
impl Default for OrObservabilityFilterRuleGroupRulesItemValueUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrObservabilityFilterRulesConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityFilterRulesConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub groups: Vec<OrObservabilityFilterRuleGroup>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityFilterRulesConfigNullable`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityFilterRulesConfigNullable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub groups: Vec<OrObservabilityFilterRuleGroup>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityGrafanaDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityGrafanaDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityGrafanaDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityGrafanaDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityGrafanaDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityGrafanaDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "baseUrl")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "instanceId")]
    pub instance_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityGrafanaDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityGrafanaDestinationTypeEnum {
    #[serde(rename = "grafana")]
    Grafana,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityGrafanaDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityLangfuseDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityLangfuseDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityLangfuseDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityLangfuseDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityLangfuseDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityLangfuseDestinationConfig {
    #[serde(rename = "baseUrl")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    #[serde(rename = "secretKey")]
    pub secret_key: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityLangfuseDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityLangfuseDestinationTypeEnum {
    #[serde(rename = "langfuse")]
    Langfuse,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityLangfuseDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityLangsmithDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityLangsmithDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityLangsmithDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityLangsmithDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityLangsmithDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityLangsmithDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(rename = "workspaceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityLangsmithDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityLangsmithDestinationTypeEnum {
    #[serde(rename = "langsmith")]
    Langsmith,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityLangsmithDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityNewrelicDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityNewrelicDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityNewrelicDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityNewrelicDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityNewrelicDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityNewrelicDestinationConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "licenseKey")]
    pub license_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<OrObservabilityNewrelicDestinationConfigRegionEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityNewrelicDestinationConfigRegionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityNewrelicDestinationConfigRegionEnum {
    #[serde(rename = "us")]
    Us,
    #[serde(rename = "eu")]
    Eu,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityNewrelicDestinationConfigRegionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrObservabilityNewrelicDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityNewrelicDestinationTypeEnum {
    #[serde(rename = "newrelic")]
    Newrelic,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityNewrelicDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityOpikDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityOpikDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityOpikDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityOpikDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityOpikDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityOpikDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "projectName")]
    pub project_name: String,
    pub workspace: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityOpikDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityOpikDestinationTypeEnum {
    #[serde(rename = "opik")]
    Opik,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityOpikDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityOtelCollectorDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityOtelCollectorDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityOtelCollectorDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityOtelCollectorDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityOtelCollectorDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityOtelCollectorDestinationConfig {
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityOtelCollectorDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityOtelCollectorDestinationTypeEnum {
    #[serde(rename = "otel-collector")]
    OtelCollector,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityOtelCollectorDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityPosthogDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityPosthogDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityPosthogDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityPosthogDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityPosthogDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityPosthogDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityPosthogDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityPosthogDestinationTypeEnum {
    #[serde(rename = "posthog")]
    Posthog,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityPosthogDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityRampDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityRampDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityRampDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityRampDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityRampDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityRampDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "baseUrl")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityRampDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityRampDestinationTypeEnum {
    #[serde(rename = "ramp")]
    Ramp,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityRampDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityS3Destination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityS3Destination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityS3DestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityS3DestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityS3DestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityS3DestinationConfig {
    #[serde(rename = "accessKeyId")]
    pub access_key_id: String,
    #[serde(rename = "bucketName")]
    pub bucket_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "pathTemplate")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(rename = "secretAccessKey")]
    pub secret_access_key: String,
    #[serde(rename = "sessionToken")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityS3DestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityS3DestinationTypeEnum {
    #[serde(rename = "s3")]
    S3,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityS3DestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilitySentryDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilitySentryDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilitySentryDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilitySentryDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilitySentryDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilitySentryDestinationConfig {
    pub dsn: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(rename = "otlpEndpoint")]
    pub otlp_endpoint: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilitySentryDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilitySentryDestinationTypeEnum {
    #[serde(rename = "sentry")]
    Sentry,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilitySentryDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilitySnowflakeDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilitySnowflakeDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilitySnowflakeDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilitySnowflakeDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilitySnowflakeDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilitySnowflakeDestinationConfig {
    pub account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warehouse: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilitySnowflakeDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilitySnowflakeDestinationTypeEnum {
    #[serde(rename = "snowflake")]
    Snowflake,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilitySnowflakeDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityWeaveDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityWeaveDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityWeaveDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityWeaveDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityWeaveDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityWeaveDestinationConfig {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "baseUrl")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub entity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    pub project: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityWeaveDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityWeaveDestinationTypeEnum {
    #[serde(rename = "weave")]
    Weave,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityWeaveDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrObservabilityWebhookDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityWebhookDestination {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    pub config: OrObservabilityWebhookDestinationConfig,
    pub created_at: String,
    pub enabled: bool,
    pub filter_rules: OrObservabilityFilterRulesConfig,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub privacy_mode: bool,
    pub sampling_rate: f64,
    #[serde(rename = "type")]
    pub type_: OrObservabilityWebhookDestinationTypeEnum,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrObservabilityWebhookDestinationConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrObservabilityWebhookDestinationConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<OrObservabilityWebhookDestinationConfigMethodEnum>,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrObservabilityWebhookDestinationConfigMethodEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityWebhookDestinationConfigMethodEnum {
    #[serde(rename = "POST")]
    POST,
    #[serde(rename = "PUT")]
    PUT,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityWebhookDestinationConfigMethodEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrObservabilityWebhookDestinationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrObservabilityWebhookDestinationTypeEnum {
    #[serde(rename = "webhook")]
    Webhook,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrObservabilityWebhookDestinationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOpenAIResponseCustomToolCall`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenAIResponseCustomToolCall {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub input: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrOpenAIResponseCustomToolCallTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOpenAIResponseCustomToolCallOutput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenAIResponseCustomToolCallOutput {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub output: OrOpenAIResponseCustomToolCallOutputOutputUnion,
    #[serde(rename = "type")]
    pub type_: OrOpenAIResponseCustomToolCallOutputTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOpenAIResponseCustomToolCallOutputOutputUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenAIResponseCustomToolCallOutputOutputUnion {
    Variant0(String),
    Variant1(Vec<OrOpenAIResponseCustomToolCallOutputOutputV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrOpenAIResponseCustomToolCallOutputOutputUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrOpenAIResponseCustomToolCallOutputOutputV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenAIResponseCustomToolCallOutputOutputV1ItemUnion {
    Variant0(OrInputText),
    Variant1(OrInputImage),
    Variant2(OrInputFile),
    Unknown(serde_json::Value),
}
impl Default for OrOpenAIResponseCustomToolCallOutputOutputV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOpenAIResponseCustomToolCallOutputTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponseCustomToolCallOutputTypeEnum {
    #[serde(rename = "custom_tool_call_output")]
    CustomToolCallOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponseCustomToolCallOutputTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOpenAIResponseCustomToolCallTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponseCustomToolCallTypeEnum {
    #[serde(rename = "custom_tool_call")]
    CustomToolCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponseCustomToolCallTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOpenAIResponseFunctionToolCall`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenAIResponseFunctionToolCall {
    pub arguments: String,
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrToolCallStatus>,
    #[serde(rename = "type")]
    pub type_: OrOpenAIResponseFunctionToolCallTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOpenAIResponseFunctionToolCallOutput`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenAIResponseFunctionToolCallOutput {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub output: OrOpenAIResponseFunctionToolCallOutputOutputUnion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrToolCallStatus>,
    #[serde(rename = "type")]
    pub type_: OrOpenAIResponseFunctionToolCallOutputTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOpenAIResponseFunctionToolCallOutputOutputUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenAIResponseFunctionToolCallOutputOutputUnion {
    Variant0(String),
    Variant1(Vec<OrOpenAIResponseFunctionToolCallOutputOutputV1ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for OrOpenAIResponseFunctionToolCallOutputOutputUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrOpenAIResponseFunctionToolCallOutputOutputV1ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenAIResponseFunctionToolCallOutputOutputV1ItemUnion {
    Variant0(OrInputText),
    Variant1(OrInputImage),
    Variant2(OrInputFile),
    Unknown(serde_json::Value),
}
impl Default for OrOpenAIResponseFunctionToolCallOutputOutputV1ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOpenAIResponseFunctionToolCallOutputTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponseFunctionToolCallOutputTypeEnum {
    #[serde(rename = "function_call_output")]
    FunctionCallOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponseFunctionToolCallOutputTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOpenAIResponseFunctionToolCallTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponseFunctionToolCallTypeEnum {
    #[serde(rename = "function_call")]
    FunctionCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponseFunctionToolCallTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOpenAIResponseInputMessageItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenAIResponseInputMessageItem {
    pub content: Vec<OrOpenAIResponseInputMessageItemContentItemUnion>,
    pub id: String,
    pub role: OrOpenAIResponseInputMessageItemRoleUnion,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<OrOpenAIResponseInputMessageItemTypeEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOpenAIResponseInputMessageItemContentItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenAIResponseInputMessageItemContentItemUnion {
    Variant0(OrInputText),
    Variant1(OrInputImage),
    Variant2(OrInputFile),
    Variant3(OrInputAudio),
    Unknown(serde_json::Value),
}
impl Default for OrOpenAIResponseInputMessageItemContentItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrOpenAIResponseInputMessageItemRoleUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenAIResponseInputMessageItemRoleUnion {
    Variant0(OrOpenAIResponseInputMessageItemRoleV0Enum),
    Variant1(OrOpenAIResponseInputMessageItemRoleV1Enum),
    Variant2(OrOpenAIResponseInputMessageItemRoleV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrOpenAIResponseInputMessageItemRoleUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOpenAIResponseInputMessageItemRoleV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponseInputMessageItemRoleV0Enum {
    #[serde(rename = "user")]
    User,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponseInputMessageItemRoleV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOpenAIResponseInputMessageItemRoleV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponseInputMessageItemRoleV1Enum {
    #[serde(rename = "system")]
    System,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponseInputMessageItemRoleV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOpenAIResponseInputMessageItemRoleV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponseInputMessageItemRoleV2Enum {
    #[serde(rename = "developer")]
    Developer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponseInputMessageItemRoleV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOpenAIResponseInputMessageItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponseInputMessageItemTypeEnum {
    #[serde(rename = "message")]
    Message,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponseInputMessageItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrOpenAIResponsesAnnotation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenAIResponsesAnnotation {
    Variant0(OrFileCitation),
    Variant1(OrURLCitation),
    Variant2(OrFilePath),
    Unknown(serde_json::Value),
}
impl Default for OrOpenAIResponsesAnnotation {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrOpenAIResponsesRefusalContent`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenAIResponsesRefusalContent {
    pub refusal: String,
    #[serde(rename = "type")]
    pub type_: OrOpenAIResponsesRefusalContentTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOpenAIResponsesRefusalContentTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponsesRefusalContentTypeEnum {
    #[serde(rename = "refusal")]
    Refusal,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponsesRefusalContentTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOpenAIResponsesResponseStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenAIResponsesResponseStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "queued")]
    Queued,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenAIResponsesResponseStatus {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrOpenAIResponsesToolChoice`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenAIResponsesToolChoice {
    Variant0(OpenAIResponsesToolChoiceV0Enum),
    Variant1(OpenAIResponsesToolChoiceV1Enum),
    Variant2(OpenAIResponsesToolChoiceV2Enum),
    Variant3(OpenAIResponsesToolChoiceV3),
    Variant4(OpenAIResponsesToolChoiceV4),
    Variant5(OrToolChoiceAllowed),
    Variant6(OpenAIResponsesToolChoiceV6),
    Variant7(OpenAIResponsesToolChoiceV7),
    Unknown(serde_json::Value),
}
impl Default for OrOpenAIResponsesToolChoice {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrOpenAIResponsesTruncation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenAIResponsesTruncation {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOpenResponsesResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenResponsesResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<OrResponsesErrorField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incomplete_details: Option<OrIncompleteDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<OrBaseInputs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i64>,
    pub metadata: OrRequestMetadata,
    pub model: String,
    pub object: OrOpenResponsesResultObjectEnum,
    pub output: Vec<OrOutputItems>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_text: Option<String>,
    pub parallel_tool_calls: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<OrStoredPromptTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_options: Option<OrPromptCacheOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OrBaseReasoningConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    pub status: OrOpenAIResponsesResponseStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<OrTextExtendedConfig>,
    pub tool_choice: OrOpenAIResponsesToolChoice,
    pub tools: Vec<OrOpenResponsesResultToolsItemUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<OrTruncation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<OrUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_type: Option<OrApiErrorType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter_metadata: Option<OrOpenRouterMetadata>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOpenResponsesResultObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenResponsesResultObjectEnum {
    #[serde(rename = "response")]
    Response,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenResponsesResultObjectEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrOpenResponsesResultToolsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOpenResponsesResultToolsItemUnion {
    Variant0(OrOpenResponsesResultToolsItemV0),
    Variant1(OrPreviewWebSearchServerTool),
    Variant2(OrPreview20250311WebSearchServerTool),
    Variant3(OrLegacyWebSearchServerTool),
    Variant4(OrWebSearchServerTool),
    Variant5(OrFileSearchServerTool),
    Variant6(OrComputerUseServerTool),
    Variant7(OrCodeInterpreterServerTool),
    Variant8(OrMcpServerTool),
    Variant9(OrImageGenerationServerTool),
    Variant10(OrCodexLocalShellTool),
    Variant11(OrShellServerTool),
    Variant12(OrApplyPatchServerTool),
    Variant13(OrCustomTool),
    Variant14(OrNamespaceTool),
    Unknown(serde_json::Value),
}
impl Default for OrOpenResponsesResultToolsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrOpenResponsesResultToolsItemV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenResponsesResultToolsItemV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrOpenResponsesResultToolsItemV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOpenResponsesResultToolsItemV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenResponsesResultToolsItemV0TypeEnum {
    #[serde(rename = "function")]
    Function,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenResponsesResultToolsItemV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOpenRouterMetadata`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenRouterMetadata {
    pub attempt: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempts: Option<Vec<OrRouterAttempt>>,
    pub endpoints: OrEndpointsMetadata,
    pub is_byok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<OrRouterParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<Vec<OrPipelineStage>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    pub requested: String,
    pub strategy: OrRoutingStrategy,
    pub summary: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOpenRouterWebSearchServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOpenRouterWebSearchServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrWebSearchConfig>,
    #[serde(rename = "type")]
    pub type_: OrOpenRouterWebSearchServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOpenRouterWebSearchServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOpenRouterWebSearchServerToolTypeEnum {
    #[serde(rename = "openrouter:web_search")]
    OpenrouterWebSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOpenRouterWebSearchServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputAdvisorServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputAdvisorServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputAdvisorServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputAdvisorServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputAdvisorServerToolItemTypeEnum {
    #[serde(rename = "openrouter:advisor")]
    OpenrouterAdvisor,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputAdvisorServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputApplyPatchCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputApplyPatchCallItem {
    pub call_id: String,
    pub id: String,
    pub operation: OrApplyPatchCallOperation,
    pub status: OrApplyPatchCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputApplyPatchCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputApplyPatchCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputApplyPatchCallItemTypeEnum {
    #[serde(rename = "apply_patch_call")]
    ApplyPatchCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputApplyPatchCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputApplyPatchServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputApplyPatchServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<OrApplyPatchCallOperation>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputApplyPatchServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputApplyPatchServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputApplyPatchServerToolItemTypeEnum {
    #[serde(rename = "openrouter:apply_patch")]
    OpenrouterApplyPatch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputApplyPatchServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputBashServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputBashServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(rename = "exitCode")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrOutputBashServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputBashServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputBashServerToolItemTypeEnum {
    #[serde(rename = "openrouter:bash")]
    OpenrouterBash,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputBashServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputBrowserUseServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputBrowserUseServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "screenshotB64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_b64: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputBrowserUseServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputBrowserUseServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputBrowserUseServerToolItemTypeEnum {
    #[serde(rename = "openrouter:browser_use")]
    OpenrouterBrowserUse,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputBrowserUseServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputCodeInterpreterCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputCodeInterpreterCallItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub container_id: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<OrOutputCodeInterpreterCallItemOutputsItemUnion>>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputCodeInterpreterCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOutputCodeInterpreterCallItemOutputsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputCodeInterpreterCallItemOutputsItemUnion {
    Variant0(OrOutputCodeInterpreterCallItemOutputsItemV0),
    Variant1(OrOutputCodeInterpreterCallItemOutputsItemV1),
    Unknown(serde_json::Value),
}
impl Default for OrOutputCodeInterpreterCallItemOutputsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrOutputCodeInterpreterCallItemOutputsItemV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputCodeInterpreterCallItemOutputsItemV0 {
    #[serde(rename = "type")]
    pub type_: OrOutputCodeInterpreterCallItemOutputsItemV0TypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputCodeInterpreterCallItemOutputsItemV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputCodeInterpreterCallItemOutputsItemV0TypeEnum {
    #[serde(rename = "image")]
    Image,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputCodeInterpreterCallItemOutputsItemV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputCodeInterpreterCallItemOutputsItemV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputCodeInterpreterCallItemOutputsItemV1 {
    pub logs: String,
    #[serde(rename = "type")]
    pub type_: OrOutputCodeInterpreterCallItemOutputsItemV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputCodeInterpreterCallItemOutputsItemV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputCodeInterpreterCallItemOutputsItemV1TypeEnum {
    #[serde(rename = "logs")]
    Logs,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputCodeInterpreterCallItemOutputsItemV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputCodeInterpreterCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputCodeInterpreterCallItemTypeEnum {
    #[serde(rename = "code_interpreter_call")]
    CodeInterpreterCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputCodeInterpreterCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputCodeInterpreterServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputCodeInterpreterServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(rename = "exitCode")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrOutputCodeInterpreterServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputCodeInterpreterServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputCodeInterpreterServerToolItemTypeEnum {
    #[serde(rename = "openrouter:code_interpreter")]
    OpenrouterCodeInterpreter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputCodeInterpreterServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputComputerCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputComputerCallItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<serde_json::Value>,
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub pending_safety_checks: Vec<OrOutputComputerCallItemPendingSafetyChecksItem>,
    pub status: OrOutputComputerCallItemStatusEnum,
    #[serde(rename = "type")]
    pub type_: OrOutputComputerCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOutputComputerCallItemPendingSafetyChecksItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputComputerCallItemPendingSafetyChecksItem {
    pub code: String,
    pub id: String,
    pub message: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputComputerCallItemStatusEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputComputerCallItemStatusEnum {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputComputerCallItemStatusEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputComputerCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputComputerCallItemTypeEnum {
    #[serde(rename = "computer_call")]
    ComputerCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputComputerCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputCustomToolCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputCustomToolCallItem {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub input: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrOutputCustomToolCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputCustomToolCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputCustomToolCallItemTypeEnum {
    #[serde(rename = "custom_tool_call")]
    CustomToolCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputCustomToolCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputDatetimeItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputDatetimeItem {
    pub datetime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub status: OrToolCallStatus,
    pub timezone: String,
    #[serde(rename = "type")]
    pub type_: OrOutputDatetimeItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputDatetimeItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputDatetimeItemTypeEnum {
    #[serde(rename = "openrouter:datetime")]
    OpenrouterDatetime,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputDatetimeItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputFileSearchCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputFileSearchCallItem {
    pub id: String,
    pub queries: Vec<String>,
    pub status: OrWebSearchStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputFileSearchCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputFileSearchCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputFileSearchCallItemTypeEnum {
    #[serde(rename = "file_search_call")]
    FileSearchCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputFileSearchCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputFileSearchServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputFileSearchServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<String>>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputFileSearchServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputFileSearchServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputFileSearchServerToolItemTypeEnum {
    #[serde(rename = "openrouter:file_search")]
    OpenrouterFileSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputFileSearchServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputFilesServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputFilesServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputFilesServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputFilesServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputFilesServerToolItemTypeEnum {
    #[serde(rename = "openrouter:files")]
    OpenrouterFiles,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputFilesServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputFunctionCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputFunctionCallItem {
    pub arguments: String,
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrOutputFunctionCallItemStatusUnion>,
    #[serde(rename = "type")]
    pub type_: OrOutputFunctionCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOutputFunctionCallItemStatusUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputFunctionCallItemStatusUnion {
    Variant0(OrOutputFunctionCallItemStatusV0Enum),
    Variant1(OrOutputFunctionCallItemStatusV1Enum),
    Variant2(OrOutputFunctionCallItemStatusV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrOutputFunctionCallItemStatusUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOutputFunctionCallItemStatusV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputFunctionCallItemStatusV0Enum {
    #[serde(rename = "completed")]
    Completed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputFunctionCallItemStatusV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputFunctionCallItemStatusV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputFunctionCallItemStatusV1Enum {
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputFunctionCallItemStatusV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputFunctionCallItemStatusV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputFunctionCallItemStatusV2Enum {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputFunctionCallItemStatusV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputFunctionCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputFunctionCallItemTypeEnum {
    #[serde(rename = "function_call")]
    FunctionCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputFunctionCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputFusionServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputFusionServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<OrFusionAnalysisResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_models: Option<Vec<OrOutputFusionServerToolItemFailedModelsItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responses: Option<Vec<OrOutputFusionServerToolItemResponsesItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<OrFusionSource>>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputFusionServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOutputFusionServerToolItemFailedModelsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputFusionServerToolItemFailedModelsItem {
    pub error: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOutputFusionServerToolItemResponsesItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputFusionServerToolItemResponsesItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub model: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputFusionServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputFusionServerToolItemTypeEnum {
    #[serde(rename = "openrouter:fusion")]
    OpenrouterFusion,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputFusionServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputImageGenerationCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputImageGenerationCallItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub status: OrImageGenerationStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputImageGenerationCallItemTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputImageGenerationCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputImageGenerationCallItemTypeEnum {
    #[serde(rename = "image_generation_call")]
    ImageGenerationCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputImageGenerationCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputImageGenerationServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputImageGenerationServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "imageB64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_b64: Option<String>,
    #[serde(rename = "imageUrl")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(rename = "revisedPrompt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputImageGenerationServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputImageGenerationServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputImageGenerationServerToolItemTypeEnum {
    #[serde(rename = "openrouter:image_generation")]
    OpenrouterImageGeneration,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputImageGenerationServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputItemImageGenerationCall`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputItemImageGenerationCall {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub status: OrImageGenerationStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputItemImageGenerationCallTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputItemImageGenerationCallTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputItemImageGenerationCallTypeEnum {
    #[serde(rename = "image_generation_call")]
    ImageGenerationCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputItemImageGenerationCallTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrOutputItems`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputItems {
    Variant0(OrOutputMessageItem),
    Variant1(OrOutputReasoningItem),
    Variant2(OrOutputFunctionCallItem),
    Variant3(OrOutputWebSearchCallItem),
    Variant4(OrOutputFileSearchCallItem),
    Variant5(OrOutputImageGenerationCallItem),
    Variant6(OrOutputCodeInterpreterCallItem),
    Variant7(OrOutputComputerCallItem),
    Variant8(OrOutputDatetimeItem),
    Variant9(OrOutputWebSearchServerToolItem),
    Variant10(OrOutputCodeInterpreterServerToolItem),
    Variant11(OrOutputFileSearchServerToolItem),
    Variant12(OrOutputImageGenerationServerToolItem),
    Variant13(OrOutputBrowserUseServerToolItem),
    Variant14(OrOutputBashServerToolItem),
    Variant15(OrOutputTextEditorServerToolItem),
    Variant16(OrOutputApplyPatchServerToolItem),
    Variant17(OrOutputApplyPatchCallItem),
    Variant18(OrOutputShellCallItem),
    Variant19(OrOutputShellCallOutputItem),
    Variant20(OrOutputWebFetchServerToolItem),
    Variant21(OrOutputToolSearchServerToolItem),
    Variant22(OrOutputMemoryServerToolItem),
    Variant23(OrOutputMcpServerToolItem),
    Variant24(OrOutputSearchModelsServerToolItem),
    Variant25(OrOutputFusionServerToolItem),
    Variant26(OrOutputAdvisorServerToolItem),
    Variant27(OrOutputSubagentServerToolItem),
    Variant28(OrOutputFilesServerToolItem),
    Variant29(OrOutputCustomToolCallItem),
    Unknown(serde_json::Value),
}
impl Default for OrOutputItems {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrOutputMcpServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputMcpServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "serverLabel")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_label: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "toolName")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrOutputMcpServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputMcpServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMcpServerToolItemTypeEnum {
    #[serde(rename = "openrouter:mcp")]
    OpenrouterMcp,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMcpServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputMemoryServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputMemoryServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<OrOutputMemoryServerToolItemActionEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputMemoryServerToolItemTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputMemoryServerToolItemActionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMemoryServerToolItemActionEnum {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "delete")]
    Delete,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMemoryServerToolItemActionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMemoryServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMemoryServerToolItemTypeEnum {
    #[serde(rename = "openrouter:memory")]
    OpenrouterMemory,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMemoryServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputMessage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputMessage {
    pub content: Vec<OrOutputMessageContentItemUnion>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<OrOutputMessagePhaseUnion>,
    pub role: OrOutputMessageRoleEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrOutputMessageStatusUnion>,
    #[serde(rename = "type")]
    pub type_: OrOutputMessageTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOutputMessageContentItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputMessageContentItemUnion {
    Variant0(OrResponseOutputText),
    Variant1(OrOpenAIResponsesRefusalContent),
    Unknown(serde_json::Value),
}
impl Default for OrOutputMessageContentItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrOutputMessageItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputMessageItem {
    pub content: Vec<OrOutputMessageItemContentItemUnion>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<OrOutputMessageItemPhaseUnion>,
    pub role: OrOutputMessageItemRoleEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrOutputMessageItemStatusUnion>,
    #[serde(rename = "type")]
    pub type_: OrOutputMessageItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOutputMessageItemContentItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputMessageItemContentItemUnion {
    Variant0(OrResponseOutputText),
    Variant1(OrOpenAIResponsesRefusalContent),
    Unknown(serde_json::Value),
}
impl Default for OrOutputMessageItemContentItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrOutputMessageItemPhaseUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputMessageItemPhaseUnion {
    Variant0(OrOutputMessageItemPhaseV0Enum),
    Variant1(OrOutputMessageItemPhaseV1Enum),
    Unknown(serde_json::Value),
}
impl Default for OrOutputMessageItemPhaseUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOutputMessageItemPhaseV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageItemPhaseV0Enum {
    #[serde(rename = "commentary")]
    Commentary,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageItemPhaseV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageItemPhaseV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageItemPhaseV1Enum {
    #[serde(rename = "final_answer")]
    FinalAnswer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageItemPhaseV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageItemRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageItemRoleEnum {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageItemRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrOutputMessageItemStatusUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputMessageItemStatusUnion {
    Variant0(OrOutputMessageItemStatusV0Enum),
    Variant1(OrOutputMessageItemStatusV1Enum),
    Variant2(OrOutputMessageItemStatusV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrOutputMessageItemStatusUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOutputMessageItemStatusV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageItemStatusV0Enum {
    #[serde(rename = "completed")]
    Completed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageItemStatusV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageItemStatusV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageItemStatusV1Enum {
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageItemStatusV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageItemStatusV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageItemStatusV2Enum {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageItemStatusV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageItemTypeEnum {
    #[serde(rename = "message")]
    Message,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrOutputMessagePhaseUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputMessagePhaseUnion {
    Variant0(OrOutputMessagePhaseV0Enum),
    Variant1(OrOutputMessagePhaseV1Enum),
    Unknown(serde_json::Value),
}
impl Default for OrOutputMessagePhaseUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOutputMessagePhaseV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessagePhaseV0Enum {
    #[serde(rename = "commentary")]
    Commentary,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessagePhaseV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessagePhaseV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessagePhaseV1Enum {
    #[serde(rename = "final_answer")]
    FinalAnswer,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessagePhaseV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageRoleEnum {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrOutputMessageStatusUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputMessageStatusUnion {
    Variant0(OrOutputMessageStatusV0Enum),
    Variant1(OrOutputMessageStatusV1Enum),
    Variant2(OrOutputMessageStatusV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrOutputMessageStatusUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOutputMessageStatusV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageStatusV0Enum {
    #[serde(rename = "completed")]
    Completed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageStatusV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageStatusV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageStatusV1Enum {
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageStatusV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageStatusV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageStatusV2Enum {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageStatusV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputMessageTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputMessageTypeEnum {
    #[serde(rename = "message")]
    Message,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputMessageTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputModality`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputModality {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "embeddings")]
    Embeddings,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "video")]
    Video,
    #[serde(rename = "rerank")]
    Rerank,
    #[serde(rename = "speech")]
    Speech,
    #[serde(rename = "transcription")]
    Transcription,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputModality {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputModalityEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputModalityEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputModalityEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputReasoningItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputReasoningItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<OrReasoningTextContent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrOutputReasoningItemStatusUnion>,
    pub summary: Vec<OrReasoningSummaryText>,
    #[serde(rename = "type")]
    pub type_: OrOutputReasoningItemTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OrReasoningFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOutputReasoningItemStatusUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputReasoningItemStatusUnion {
    Variant0(OrOutputReasoningItemStatusV0Enum),
    Variant1(OrOutputReasoningItemStatusV1Enum),
    Variant2(OrOutputReasoningItemStatusV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrOutputReasoningItemStatusUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrOutputReasoningItemStatusV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputReasoningItemStatusV0Enum {
    #[serde(rename = "completed")]
    Completed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputReasoningItemStatusV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputReasoningItemStatusV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputReasoningItemStatusV1Enum {
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputReasoningItemStatusV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputReasoningItemStatusV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputReasoningItemStatusV2Enum {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputReasoningItemStatusV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputReasoningItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputReasoningItemTypeEnum {
    #[serde(rename = "reasoning")]
    Reasoning,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputReasoningItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputSearchModelsServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputSearchModelsServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputSearchModelsServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputSearchModelsServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputSearchModelsServerToolItemTypeEnum {
    #[serde(rename = "openrouter:experimental__search_models")]
    OpenrouterExperimentalSearchModels,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputSearchModelsServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputShellCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputShellCallItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<OrOutputShellCallItemAction>,
    pub call_id: String,
    pub id: String,
    pub status: OrShellCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputShellCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOutputShellCallItemAction`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputShellCallItemAction {
    pub commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_length: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputShellCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputShellCallItemTypeEnum {
    #[serde(rename = "shell_call")]
    ShellCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputShellCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputShellCallOutputItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputShellCallOutputItem {
    pub call_id: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_length: Option<i64>,
    pub output: Vec<OrOutputShellCallOutputItemOutputItem>,
    pub status: OrShellCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputShellCallOutputItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOutputShellCallOutputItemOutputItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputShellCallOutputItemOutputItem {
    pub outcome: OrOutputShellCallOutputItemOutputItemOutcomeUnion,
    pub stderr: String,
    pub stdout: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOutputShellCallOutputItemOutputItemOutcomeUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputShellCallOutputItemOutputItemOutcomeUnion {
    Variant0(OrOutputShellCallOutputItemOutputItemOutcomeV0),
    Variant1(OrOutputShellCallOutputItemOutputItemOutcomeV1),
    Unknown(serde_json::Value),
}
impl Default for OrOutputShellCallOutputItemOutputItemOutcomeUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrOutputShellCallOutputItemOutputItemOutcomeV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputShellCallOutputItemOutputItemOutcomeV0 {
    pub exit_code: i64,
    #[serde(rename = "type")]
    pub type_: OrOutputShellCallOutputItemOutputItemOutcomeV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputShellCallOutputItemOutputItemOutcomeV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputShellCallOutputItemOutputItemOutcomeV0TypeEnum {
    #[serde(rename = "exit")]
    Exit,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputShellCallOutputItemOutputItemOutcomeV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputShellCallOutputItemOutputItemOutcomeV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputShellCallOutputItemOutputItemOutcomeV1 {
    #[serde(rename = "type")]
    pub type_: OrOutputShellCallOutputItemOutputItemOutcomeV1TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputShellCallOutputItemOutputItemOutcomeV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputShellCallOutputItemOutputItemOutcomeV1TypeEnum {
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputShellCallOutputItemOutputItemOutcomeV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputShellCallOutputItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputShellCallOutputItemTypeEnum {
    #[serde(rename = "shell_call_output")]
    ShellCallOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputShellCallOutputItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputSubagentServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputSubagentServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<OrSubagentSessionItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_version: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<OrOutputSubagentServerToolItemStateEnum>,
    pub status: OrToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_name: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrOutputSubagentServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputSubagentServerToolItemStateEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputSubagentServerToolItemStateEnum {
    #[serde(rename = "completed")]
    Completed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputSubagentServerToolItemStateEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputSubagentServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputSubagentServerToolItemTypeEnum {
    #[serde(rename = "openrouter:subagent")]
    OpenrouterSubagent,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputSubagentServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputTextEditorServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputTextEditorServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<OrOutputTextEditorServerToolItemCommandEnum>,
    #[serde(rename = "filePath")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputTextEditorServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputTextEditorServerToolItemCommandEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputTextEditorServerToolItemCommandEnum {
    #[serde(rename = "view")]
    View,
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "str_replace")]
    StrReplace,
    #[serde(rename = "insert")]
    Insert,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputTextEditorServerToolItemCommandEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputTextEditorServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputTextEditorServerToolItemTypeEnum {
    #[serde(rename = "openrouter:text_editor")]
    OpenrouterTextEditor,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputTextEditorServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputToolSearchServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputToolSearchServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputToolSearchServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputToolSearchServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputToolSearchServerToolItemTypeEnum {
    #[serde(rename = "openrouter:tool_search")]
    OpenrouterToolSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputToolSearchServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputWebFetchServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputWebFetchServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(rename = "httpStatus")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrOutputWebFetchServerToolItemTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputWebFetchServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputWebFetchServerToolItemTypeEnum {
    #[serde(rename = "openrouter:web_fetch")]
    OpenrouterWebFetch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputWebFetchServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputWebSearchCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputWebSearchCallItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<OrOutputWebSearchCallItemActionUnion>,
    pub id: String,
    pub status: OrWebSearchStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputWebSearchCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrOutputWebSearchCallItemActionUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrOutputWebSearchCallItemActionUnion {
    Variant0(OrOutputWebSearchCallItemActionV0),
    Variant1(OrOutputWebSearchCallItemActionV1),
    Variant2(OrOutputWebSearchCallItemActionV2),
    Unknown(serde_json::Value),
}
impl Default for OrOutputWebSearchCallItemActionUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrOutputWebSearchCallItemActionV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputWebSearchCallItemActionV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<String>>,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<OrWebSearchSource>>,
    #[serde(rename = "type")]
    pub type_: OrOutputWebSearchCallItemActionV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputWebSearchCallItemActionV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputWebSearchCallItemActionV0TypeEnum {
    #[serde(rename = "search")]
    Search,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputWebSearchCallItemActionV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputWebSearchCallItemActionV1`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputWebSearchCallItemActionV1 {
    #[serde(rename = "type")]
    pub type_: OrOutputWebSearchCallItemActionV1TypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputWebSearchCallItemActionV1TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputWebSearchCallItemActionV1TypeEnum {
    #[serde(rename = "open_page")]
    OpenPage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputWebSearchCallItemActionV1TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputWebSearchCallItemActionV2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputWebSearchCallItemActionV2 {
    pub pattern: String,
    #[serde(rename = "type")]
    pub type_: OrOutputWebSearchCallItemActionV2TypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputWebSearchCallItemActionV2TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputWebSearchCallItemActionV2TypeEnum {
    #[serde(rename = "find_in_page")]
    FindInPage,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputWebSearchCallItemActionV2TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputWebSearchCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputWebSearchCallItemTypeEnum {
    #[serde(rename = "web_search_call")]
    WebSearchCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputWebSearchCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrOutputWebSearchServerToolItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputWebSearchServerToolItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<OrOutputWebSearchServerToolItemAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub status: OrToolCallStatus,
    #[serde(rename = "type")]
    pub type_: OrOutputWebSearchServerToolItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOutputWebSearchServerToolItemAction`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputWebSearchServerToolItemAction {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<OrOutputWebSearchServerToolItemActionSourcesItem>>,
    #[serde(rename = "type")]
    pub type_: OrOutputWebSearchServerToolItemActionTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrOutputWebSearchServerToolItemActionSourcesItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrOutputWebSearchServerToolItemActionSourcesItem {
    #[serde(rename = "type")]
    pub type_: OrOutputWebSearchServerToolItemActionSourcesItemTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrOutputWebSearchServerToolItemActionSourcesItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputWebSearchServerToolItemActionSourcesItemTypeEnum {
    #[serde(rename = "url")]
    Url,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputWebSearchServerToolItemActionSourcesItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputWebSearchServerToolItemActionTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputWebSearchServerToolItemActionTypeEnum {
    #[serde(rename = "search")]
    Search,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputWebSearchServerToolItemActionTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrOutputWebSearchServerToolItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrOutputWebSearchServerToolItemTypeEnum {
    #[serde(rename = "openrouter:web_search")]
    OpenrouterWebSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrOutputWebSearchServerToolItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrPDFParserEngine`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrPDFParserEngine {
    Variant0(PDFParserEngineV0Enum),
    Variant1(PDFParserEngineV1Enum),
    Unknown(serde_json::Value),
}
impl Default for OrPDFParserEngine {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrPDFParserOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPDFParserOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrPDFParserEngine>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrParameter`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrParameter {
    #[serde(rename = "temperature")]
    Temperature,
    #[serde(rename = "top_p")]
    TopP,
    #[serde(rename = "top_k")]
    TopK,
    #[serde(rename = "min_p")]
    MinP,
    #[serde(rename = "top_a")]
    TopA,
    #[serde(rename = "frequency_penalty")]
    FrequencyPenalty,
    #[serde(rename = "presence_penalty")]
    PresencePenalty,
    #[serde(rename = "repetition_penalty")]
    RepetitionPenalty,
    #[serde(rename = "max_tokens")]
    MaxTokens,
    #[serde(rename = "max_completion_tokens")]
    MaxCompletionTokens,
    #[serde(rename = "logit_bias")]
    LogitBias,
    #[serde(rename = "logprobs")]
    Logprobs,
    #[serde(rename = "top_logprobs")]
    TopLogprobs,
    #[serde(rename = "prediction")]
    Prediction,
    #[serde(rename = "seed")]
    Seed,
    #[serde(rename = "response_format")]
    ResponseFormat,
    #[serde(rename = "structured_outputs")]
    StructuredOutputs,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "tools")]
    Tools,
    #[serde(rename = "tool_choice")]
    ToolChoice,
    #[serde(rename = "parallel_tool_calls")]
    ParallelToolCalls,
    #[serde(rename = "include_reasoning")]
    IncludeReasoning,
    #[serde(rename = "reasoning")]
    Reasoning,
    #[serde(rename = "reasoning_effort")]
    ReasoningEffort,
    #[serde(rename = "web_search_options")]
    WebSearchOptions,
    #[serde(rename = "verbosity")]
    Verbosity,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrParameter {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrParetoRouterPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrParetoRouterPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub id: OrParetoRouterPluginIdEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_coding_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_source: Option<OrParetoRouterPluginPriceSourceEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrParetoRouterPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrParetoRouterPluginIdEnum {
    #[serde(rename = "pareto-router")]
    ParetoRouter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrParetoRouterPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrParetoRouterPluginPriceSourceEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrParetoRouterPluginPriceSourceEnum {
    #[serde(rename = "prompt")]
    Prompt,
    #[serde(rename = "weighted_avg")]
    WeightedAvg,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrParetoRouterPluginPriceSourceEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrPerRequestLimits`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPerRequestLimits {
    pub completion_tokens: f64,
    pub prompt_tokens: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPercentileLatencyCutoffs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPercentileLatencyCutoffs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p50: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p75: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p90: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p99: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPercentileStats`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPercentileStats {
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub p99: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPercentileThroughputCutoffs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPercentileThroughputCutoffs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p50: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p75: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p90: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p99: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPipelineStage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPipelineStage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardrail_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardrail_scope: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrPipelineStageType,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrPipelineStageType`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPipelineStageType {
    #[serde(rename = "guardrail")]
    Guardrail,
    #[serde(rename = "plugin")]
    Plugin,
    #[serde(rename = "server_tools")]
    ServerTools,
    #[serde(rename = "response_healing")]
    ResponseHealing,
    #[serde(rename = "context_compression")]
    ContextCompression,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPipelineStageType {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrPrediction`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPrediction {
    pub content: OrPredictionContentUnion,
    #[serde(rename = "type")]
    pub type_: OrPredictionTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPredictionContentText`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPredictionContentText {
    pub text: String,
    #[serde(rename = "type")]
    pub type_: OrPredictionContentTextTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrPredictionContentTextTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPredictionContentTextTypeEnum {
    #[serde(rename = "text")]
    Text,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPredictionContentTextTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrPredictionContentUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrPredictionContentUnion {
    Variant0(String),
    Variant1(Vec<OrPredictionContentText>),
    Unknown(serde_json::Value),
}
impl Default for OrPredictionContentUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrPredictionTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPredictionTypeEnum {
    #[serde(rename = "content")]
    Content,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPredictionTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrPreferredMaxLatency`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrPreferredMaxLatency {
    Variant0(f64),
    Variant1(OrPercentileLatencyCutoffs),
    Unknown(serde_json::Value),
}
impl Default for OrPreferredMaxLatency {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrPreferredMinThroughput`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrPreferredMinThroughput {
    Variant0(f64),
    Variant1(OrPercentileThroughputCutoffs),
    Unknown(serde_json::Value),
}
impl Default for OrPreferredMinThroughput {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrPreset`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPreset {
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designated_version_id: Option<String>,
    pub id: String,
    pub name: String,
    pub slug: String,
    pub status: OrPresetStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_updated_at: Option<String>,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPresetDesignatedVersion`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPresetDesignatedVersion {
    pub config: std::collections::BTreeMap<String, serde_json::Value>,
    pub created_at: String,
    pub creator_id: String,
    pub id: String,
    pub preset_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub updated_at: String,
    pub version: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrPresetStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPresetStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "archived")]
    Archived,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPresetStatus {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrPresetWithDesignatedVersion`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPresetWithDesignatedVersion {
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designated_version_id: Option<String>,
    pub id: String,
    pub name: String,
    pub slug: String,
    pub status: OrPresetStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_updated_at: Option<String>,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    pub designated_version: OrPresetDesignatedVersion,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPreview20250311WebSearchServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPreview20250311WebSearchServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebSearchEngineEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<OrWebSearchDomainFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<OrSearchContextSizeEnum>,
    #[serde(rename = "type")]
    pub type_: OrPreview20250311WebSearchServerToolTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrPreviewWebSearchUserLocation>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrPreview20250311WebSearchServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPreview20250311WebSearchServerToolTypeEnum {
    #[serde(rename = "web_search_preview_2025_03_11")]
    WebSearchPreview20250311,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPreview20250311WebSearchServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrPreviewWebSearchServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPreviewWebSearchServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebSearchEngineEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<OrWebSearchDomainFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<OrSearchContextSizeEnum>,
    #[serde(rename = "type")]
    pub type_: OrPreviewWebSearchServerToolTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrPreviewWebSearchUserLocation>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrPreviewWebSearchServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPreviewWebSearchServerToolTypeEnum {
    #[serde(rename = "web_search_preview")]
    WebSearchPreview,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPreviewWebSearchServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrPreviewWebSearchUserLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPreviewWebSearchUserLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrPreviewWebSearchUserLocationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrPreviewWebSearchUserLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPreviewWebSearchUserLocationTypeEnum {
    #[serde(rename = "approximate")]
    Approximate,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPreviewWebSearchUserLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrPricingOverride`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPricingOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_audio_cache: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_read: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write_1h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_prompt_tokens: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc_end: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc_start: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPromptCacheBreakpoint`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPromptCacheBreakpoint {
    pub mode: OrPromptCacheBreakpointModeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrPromptCacheBreakpointModeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPromptCacheBreakpointModeEnum {
    #[serde(rename = "explicit")]
    Explicit,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPromptCacheBreakpointModeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrPromptCacheOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPromptCacheOptions {
    pub mode: OrPromptCacheOptionsModeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrPromptCacheOptionsModeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPromptCacheOptionsModeEnum {
    #[serde(rename = "explicit")]
    Explicit,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPromptCacheOptionsModeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrPromptInjectionScanScope`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrPromptInjectionScanScope {
    #[serde(rename = "user_only")]
    UserOnly,
    #[serde(rename = "all_messages")]
    AllMessages,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrPromptInjectionScanScope {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrProviderName`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrProviderName {
    #[serde(rename = "AkashML")]
    AkashML,
    #[serde(rename = "AI21")]
    AI21,
    #[serde(rename = "AionLabs")]
    AionLabs,
    #[serde(rename = "Alibaba")]
    Alibaba,
    #[serde(rename = "Ambient")]
    Ambient,
    #[serde(rename = "Baidu")]
    Baidu,
    #[serde(rename = "Amazon Bedrock")]
    AmazonBedrock,
    #[serde(rename = "Amazon Nova")]
    AmazonNova,
    #[serde(rename = "Anthropic")]
    Anthropic,
    #[serde(rename = "Arcee AI")]
    ArceeAI,
    #[serde(rename = "AtlasCloud")]
    AtlasCloud,
    #[serde(rename = "Avian")]
    Avian,
    #[serde(rename = "Azure")]
    Azure,
    #[serde(rename = "BaseTen")]
    BaseTen,
    #[serde(rename = "BytePlus")]
    BytePlus,
    #[serde(rename = "Black Forest Labs")]
    BlackForestLabs,
    #[serde(rename = "Cerebras")]
    Cerebras,
    #[serde(rename = "Chutes")]
    Chutes,
    #[serde(rename = "Cirrascale")]
    Cirrascale,
    #[serde(rename = "Clarifai")]
    Clarifai,
    #[serde(rename = "Cloudflare")]
    Cloudflare,
    #[serde(rename = "Cohere")]
    Cohere,
    #[serde(rename = "CoreWeave")]
    CoreWeave,
    #[serde(rename = "Crucible")]
    Crucible,
    #[serde(rename = "Crusoe")]
    Crusoe,
    #[serde(rename = "Darkbloom")]
    Darkbloom,
    #[serde(rename = "Decart")]
    Decart,
    #[serde(rename = "Deepgram")]
    Deepgram,
    #[serde(rename = "DeepInfra")]
    DeepInfra,
    #[serde(rename = "DeepSeek")]
    DeepSeek,
    #[serde(rename = "DekaLLM")]
    DekaLLM,
    #[serde(rename = "DigitalOcean")]
    DigitalOcean,
    #[serde(rename = "Featherless")]
    Featherless,
    #[serde(rename = "Fireworks")]
    Fireworks,
    #[serde(rename = "Fish Audio")]
    FishAudio,
    #[serde(rename = "Friendli")]
    Friendli,
    #[serde(rename = "GMICloud")]
    GMICloud,
    #[serde(rename = "Google")]
    Google,
    #[serde(rename = "Google AI Studio")]
    GoogleAIStudio,
    #[serde(rename = "Groq")]
    Groq,
    #[serde(rename = "HeyGen")]
    HeyGen,
    #[serde(rename = "Inception")]
    Inception,
    #[serde(rename = "Inceptron")]
    Inceptron,
    #[serde(rename = "InferenceNet")]
    InferenceNet,
    #[serde(rename = "Ionstream")]
    Ionstream,
    #[serde(rename = "Infermatic")]
    Infermatic,
    #[serde(rename = "Io Net")]
    IoNet,
    #[serde(rename = "Inferact vLLM")]
    InferactVLLM,
    #[serde(rename = "Inflection")]
    Inflection,
    #[serde(rename = "Liquid")]
    Liquid,
    #[serde(rename = "Mara")]
    Mara,
    #[serde(rename = "Mancer 2")]
    Mancer2,
    #[serde(rename = "Meta")]
    Meta,
    #[serde(rename = "Minimax")]
    Minimax,
    #[serde(rename = "ModelRun")]
    ModelRun,
    #[serde(rename = "Mistral")]
    Mistral,
    #[serde(rename = "Modular")]
    Modular,
    #[serde(rename = "Moonshot AI")]
    MoonshotAI,
    #[serde(rename = "Morph")]
    Morph,
    #[serde(rename = "NCompass")]
    NCompass,
    #[serde(rename = "Nebius")]
    Nebius,
    #[serde(rename = "Nex AGI")]
    NexAGI,
    #[serde(rename = "NextBit")]
    NextBit,
    #[serde(rename = "Novita")]
    Novita,
    #[serde(rename = "Nvidia")]
    Nvidia,
    #[serde(rename = "OpenAI")]
    OpenAI,
    #[serde(rename = "OpenInference")]
    OpenInference,
    #[serde(rename = "Parasail")]
    Parasail,
    #[serde(rename = "Poolside")]
    Poolside,
    #[serde(rename = "Perceptron")]
    Perceptron,
    #[serde(rename = "Perplexity")]
    Perplexity,
    #[serde(rename = "Phala")]
    Phala,
    #[serde(rename = "Recraft")]
    Recraft,
    #[serde(rename = "Reka")]
    Reka,
    #[serde(rename = "Relace")]
    Relace,
    #[serde(rename = "Sail Research")]
    SailResearch,
    #[serde(rename = "Sakana AI")]
    SakanaAI,
    #[serde(rename = "SambaNova")]
    SambaNova,
    #[serde(rename = "Seed")]
    Seed,
    #[serde(rename = "SiliconFlow")]
    SiliconFlow,
    #[serde(rename = "Sourceful")]
    Sourceful,
    #[serde(rename = "StepFun")]
    StepFun,
    #[serde(rename = "Stealth")]
    Stealth,
    #[serde(rename = "StreamLake")]
    StreamLake,
    #[serde(rename = "Switchpoint")]
    Switchpoint,
    #[serde(rename = "Tencent")]
    Tencent,
    #[serde(rename = "Tenstorrent")]
    Tenstorrent,
    #[serde(rename = "Together")]
    Together,
    #[serde(rename = "Upstage")]
    Upstage,
    #[serde(rename = "Venice")]
    Venice,
    #[serde(rename = "Wafer")]
    Wafer,
    #[serde(rename = "WandB")]
    WandB,
    #[serde(rename = "Quiver")]
    Quiver,
    #[serde(rename = "Krea")]
    Krea,
    #[serde(rename = "Runway")]
    Runway,
    #[serde(rename = "Xiaomi")]
    Xiaomi,
    #[serde(rename = "xAI")]
    XAI,
    #[serde(rename = "Z.AI")]
    ZAI,
    #[serde(rename = "FakeProvider")]
    FakeProvider,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrProviderName {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrProviderOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrProviderOptions {
    #[serde(rename = "01ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_01ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai21: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "aion-labs")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aion_labs: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub akashml: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alibaba: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "amazon-bedrock")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amazon_bedrock: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "amazon-nova")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amazon_nova: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anyscale: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "arcee-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcee_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "atlas-cloud")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas_cloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atoma: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avian: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baidu: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseten: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "black-forest-labs")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub black_forest_labs: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byteplus: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centml: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cerebras: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chutes: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cirrascale: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarifai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflare: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohere: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coreweave: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crofai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crucible: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crusoe: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub darkbloom: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decart: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepgram: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepinfra: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepseek: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dekallm: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digitalocean: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enfer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "fake-provider")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fake_provider: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featherless: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fireworks: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "fish-audio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fish_audio: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub friendli: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gmicloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "google-ai-studio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_ai_studio: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "google-vertex")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_vertex: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gopomelo: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groq: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heygen: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huggingface: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperbolic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "hyperbolic-quantized")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperbolic_quantized: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inception: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inceptron: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "inferact-vllm")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inferact_vllm: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "inference-net")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_net: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infermatic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inflection: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inocloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "io-net")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_net: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ionstream: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub klusterai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub krea: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lepton: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lynn: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "lynn-private")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lynn_private: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mancer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "mancer-old")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mancer_old: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mara: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimax: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mistral: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelrun: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modular: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moonshotai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morph: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ncompass: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nebius: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "nex-agi")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nex_agi: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nextbit: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nineteen: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub novita: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub octoai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "open-inference")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_inference: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parasail: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perceptron: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perplexity: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phala: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poolside: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiver: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recraft: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursal: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reka: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relace: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runway: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sail-research")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sail_research: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sakana: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sakana-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sakana_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sambanova: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sambanova-cloaked")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sambanova_cloaked: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sf-compute")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sf_compute: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub siliconflow: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sourceful: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stealth: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stepfun: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streamlake: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switchpoint: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targon: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tencent: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenstorrent: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub together: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "together-lite")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub together_lite: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ubicloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstage: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venice: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wafer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wandb: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "wandb-legacy")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wandb_legacy: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xiaomi: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "z-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrProviderPreferences`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrProviderPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<OrProviderPreferencesDataCollectionEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<OrProviderPreferencesIgnoreItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<OrProviderPreferencesMaxPrice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<OrProviderPreferencesOnlyItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<OrProviderPreferencesOrderItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_max_latency: Option<OrPreferredMaxLatency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_min_throughput: Option<OrPreferredMinThroughput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<OrQuantization>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<OrProviderPreferencesSortUnion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrProviderPreferencesDataCollectionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrProviderPreferencesDataCollectionEnum {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "allow")]
    Allow,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrProviderPreferencesDataCollectionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrProviderPreferencesIgnoreItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrProviderPreferencesIgnoreItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for OrProviderPreferencesIgnoreItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrProviderPreferencesMaxPrice`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrProviderPreferencesMaxPrice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrProviderPreferencesOnlyItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrProviderPreferencesOnlyItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for OrProviderPreferencesOnlyItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrProviderPreferencesOrderItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrProviderPreferencesOrderItemUnion {
    Variant0(OrProviderName),
    Variant1(String),
    Unknown(serde_json::Value),
}
impl Default for OrProviderPreferencesOrderItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `OrProviderPreferencesSortUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrProviderPreferencesSortUnion {
    Variant0(OrProviderSort),
    Variant1(OrProviderSortConfig),
    Unknown(serde_json::Value),
}
impl Default for OrProviderPreferencesSortUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrProviderResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrProviderResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_byok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_permaslug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<OrProviderResponseProviderNameEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routed_service_tier: Option<OrProviderResponseRoutedServiceTierEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrProviderResponseProviderNameEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrProviderResponseProviderNameEnum {
    #[serde(rename = "AnyScale")]
    AnyScale,
    #[serde(rename = "Atoma")]
    Atoma,
    #[serde(rename = "Cent-ML")]
    CentML,
    #[serde(rename = "CrofAI")]
    CrofAI,
    #[serde(rename = "Enfer")]
    Enfer,
    #[serde(rename = "GoPomelo")]
    GoPomelo,
    #[serde(rename = "HuggingFace")]
    HuggingFace,
    #[serde(rename = "Hyperbolic")]
    Hyperbolic,
    #[serde(rename = "Hyperbolic 2")]
    Hyperbolic2,
    #[serde(rename = "InoCloud")]
    InoCloud,
    #[serde(rename = "Kluster")]
    Kluster,
    #[serde(rename = "Lambda")]
    Lambda,
    #[serde(rename = "Lepton")]
    Lepton,
    #[serde(rename = "Lynn 2")]
    Lynn2,
    #[serde(rename = "Lynn")]
    Lynn,
    #[serde(rename = "Mancer")]
    Mancer,
    #[serde(rename = "Modal")]
    Modal,
    #[serde(rename = "Nineteen")]
    Nineteen,
    #[serde(rename = "OctoAI")]
    OctoAI,
    #[serde(rename = "Recursal")]
    Recursal,
    #[serde(rename = "Reflection")]
    Reflection,
    #[serde(rename = "Replicate")]
    Replicate,
    #[serde(rename = "SambaNova 2")]
    SambaNova2,
    #[serde(rename = "SF Compute")]
    SFCompute,
    #[serde(rename = "Targon")]
    Targon,
    #[serde(rename = "Together 2")]
    Together2,
    #[serde(rename = "Ubicloud")]
    Ubicloud,
    #[serde(rename = "01.AI")]
    T01AI,
    #[serde(rename = "AkashML")]
    AkashML,
    #[serde(rename = "AI21")]
    AI21,
    #[serde(rename = "AionLabs")]
    AionLabs,
    #[serde(rename = "Alibaba")]
    Alibaba,
    #[serde(rename = "Ambient")]
    Ambient,
    #[serde(rename = "Baidu")]
    Baidu,
    #[serde(rename = "Amazon Bedrock")]
    AmazonBedrock,
    #[serde(rename = "Amazon Nova")]
    AmazonNova,
    #[serde(rename = "Anthropic")]
    Anthropic,
    #[serde(rename = "Arcee AI")]
    ArceeAI,
    #[serde(rename = "AtlasCloud")]
    AtlasCloud,
    #[serde(rename = "Avian")]
    Avian,
    #[serde(rename = "Azure")]
    Azure,
    #[serde(rename = "BaseTen")]
    BaseTen,
    #[serde(rename = "BytePlus")]
    BytePlus,
    #[serde(rename = "Black Forest Labs")]
    BlackForestLabs,
    #[serde(rename = "Cerebras")]
    Cerebras,
    #[serde(rename = "Chutes")]
    Chutes,
    #[serde(rename = "Cirrascale")]
    Cirrascale,
    #[serde(rename = "Clarifai")]
    Clarifai,
    #[serde(rename = "Cloudflare")]
    Cloudflare,
    #[serde(rename = "Cohere")]
    Cohere,
    #[serde(rename = "CoreWeave")]
    CoreWeave,
    #[serde(rename = "Crucible")]
    Crucible,
    #[serde(rename = "Crusoe")]
    Crusoe,
    #[serde(rename = "Darkbloom")]
    Darkbloom,
    #[serde(rename = "Decart")]
    Decart,
    #[serde(rename = "Deepgram")]
    Deepgram,
    #[serde(rename = "DeepInfra")]
    DeepInfra,
    #[serde(rename = "DeepSeek")]
    DeepSeek,
    #[serde(rename = "DekaLLM")]
    DekaLLM,
    #[serde(rename = "DigitalOcean")]
    DigitalOcean,
    #[serde(rename = "Featherless")]
    Featherless,
    #[serde(rename = "Fireworks")]
    Fireworks,
    #[serde(rename = "Fish Audio")]
    FishAudio,
    #[serde(rename = "Friendli")]
    Friendli,
    #[serde(rename = "GMICloud")]
    GMICloud,
    #[serde(rename = "Google")]
    Google,
    #[serde(rename = "Google AI Studio")]
    GoogleAIStudio,
    #[serde(rename = "Groq")]
    Groq,
    #[serde(rename = "HeyGen")]
    HeyGen,
    #[serde(rename = "Inception")]
    Inception,
    #[serde(rename = "Inceptron")]
    Inceptron,
    #[serde(rename = "InferenceNet")]
    InferenceNet,
    #[serde(rename = "Ionstream")]
    Ionstream,
    #[serde(rename = "Infermatic")]
    Infermatic,
    #[serde(rename = "Io Net")]
    IoNet,
    #[serde(rename = "Inferact vLLM")]
    InferactVLLM,
    #[serde(rename = "Inflection")]
    Inflection,
    #[serde(rename = "Liquid")]
    Liquid,
    #[serde(rename = "Mara")]
    Mara,
    #[serde(rename = "Mancer 2")]
    Mancer2,
    #[serde(rename = "Meta")]
    Meta,
    #[serde(rename = "Minimax")]
    Minimax,
    #[serde(rename = "ModelRun")]
    ModelRun,
    #[serde(rename = "Mistral")]
    Mistral,
    #[serde(rename = "Modular")]
    Modular,
    #[serde(rename = "Moonshot AI")]
    MoonshotAI,
    #[serde(rename = "Morph")]
    Morph,
    #[serde(rename = "NCompass")]
    NCompass,
    #[serde(rename = "Nebius")]
    Nebius,
    #[serde(rename = "Nex AGI")]
    NexAGI,
    #[serde(rename = "NextBit")]
    NextBit,
    #[serde(rename = "Novita")]
    Novita,
    #[serde(rename = "Nvidia")]
    Nvidia,
    #[serde(rename = "OpenAI")]
    OpenAI,
    #[serde(rename = "OpenInference")]
    OpenInference,
    #[serde(rename = "Parasail")]
    Parasail,
    #[serde(rename = "Poolside")]
    Poolside,
    #[serde(rename = "Perceptron")]
    Perceptron,
    #[serde(rename = "Perplexity")]
    Perplexity,
    #[serde(rename = "Phala")]
    Phala,
    #[serde(rename = "Recraft")]
    Recraft,
    #[serde(rename = "Reka")]
    Reka,
    #[serde(rename = "Relace")]
    Relace,
    #[serde(rename = "Sail Research")]
    SailResearch,
    #[serde(rename = "Sakana AI")]
    SakanaAI,
    #[serde(rename = "SambaNova")]
    SambaNova,
    #[serde(rename = "Seed")]
    Seed,
    #[serde(rename = "SiliconFlow")]
    SiliconFlow,
    #[serde(rename = "Sourceful")]
    Sourceful,
    #[serde(rename = "StepFun")]
    StepFun,
    #[serde(rename = "Stealth")]
    Stealth,
    #[serde(rename = "StreamLake")]
    StreamLake,
    #[serde(rename = "Switchpoint")]
    Switchpoint,
    #[serde(rename = "Tencent")]
    Tencent,
    #[serde(rename = "Tenstorrent")]
    Tenstorrent,
    #[serde(rename = "Together")]
    Together,
    #[serde(rename = "Upstage")]
    Upstage,
    #[serde(rename = "Venice")]
    Venice,
    #[serde(rename = "Wafer")]
    Wafer,
    #[serde(rename = "WandB")]
    WandB,
    #[serde(rename = "Quiver")]
    Quiver,
    #[serde(rename = "Krea")]
    Krea,
    #[serde(rename = "Runway")]
    Runway,
    #[serde(rename = "Xiaomi")]
    Xiaomi,
    #[serde(rename = "xAI")]
    XAI,
    #[serde(rename = "Z.AI")]
    ZAI,
    #[serde(rename = "FakeProvider")]
    FakeProvider,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrProviderResponseProviderNameEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrProviderResponseRoutedServiceTierEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrProviderResponseRoutedServiceTierEnum {
    #[serde(rename = "flex")]
    Flex,
    #[serde(rename = "priority")]
    Priority,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrProviderResponseRoutedServiceTierEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrProviderSort`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrProviderSort {
    #[serde(rename = "price")]
    Price,
    #[serde(rename = "throughput")]
    Throughput,
    #[serde(rename = "latency")]
    Latency,
    #[serde(rename = "exacto")]
    Exacto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrProviderSort {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrProviderSortConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrProviderSortConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<OrProviderSortConfigByEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<OrProviderSortConfigPartitionEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrProviderSortConfigByEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrProviderSortConfigByEnum {
    #[serde(rename = "price")]
    Price,
    #[serde(rename = "throughput")]
    Throughput,
    #[serde(rename = "latency")]
    Latency,
    #[serde(rename = "exacto")]
    Exacto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrProviderSortConfigByEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrProviderSortConfigPartitionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrProviderSortConfigPartitionEnum {
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrProviderSortConfigPartitionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrPublicEndpoint`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPublicEndpoint {
    pub context_length: i64,
    pub latency_last_30m: OrPercentileStats,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_prompt_tokens: Option<i64>,
    pub model_id: String,
    pub model_name: String,
    pub name: String,
    pub pricing: OrPublicEndpointPricing,
    pub provider_name: OrProviderName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization: Option<OrQuantization>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrEndpointStatus>,
    pub supported_parameters: Vec<OrParameter>,
    pub supports_implicit_caching: bool,
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throughput_last_30m: Option<OrPublicEndpointThroughputLast30m>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime_last_1d: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime_last_30m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime_last_5m: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPublicEndpointPricing`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPublicEndpointPricing {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_output: Option<String>,
    pub completion: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_audio_cache: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_read: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write_1h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Vec<OrPricingOverride>>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPublicEndpointThroughputLast30m`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPublicEndpointThroughputLast30m {
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub p99: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrPublicPricing`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrPublicPricing {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_output: Option<String>,
    pub completion: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_audio_cache: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_read: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write_1h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Vec<OrPricingOverride>>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrQuantization`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrQuantization {
    #[serde(rename = "int4")]
    Int4,
    #[serde(rename = "int8")]
    Int8,
    #[serde(rename = "fp4")]
    Fp4,
    #[serde(rename = "fp6")]
    Fp6,
    #[serde(rename = "fp8")]
    Fp8,
    #[serde(rename = "fp16")]
    Fp16,
    #[serde(rename = "bf16")]
    Bf16,
    #[serde(rename = "fp32")]
    Fp32,
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrQuantization {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrRangeCapability`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrRangeCapability {
    pub max: f64,
    pub min: f64,
    #[serde(rename = "type")]
    pub type_: OrRangeCapabilityTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrRangeCapabilityTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrRangeCapabilityTypeEnum {
    #[serde(rename = "range")]
    Range,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrRangeCapabilityTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrRankingsDailyItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrRankingsDailyItem {
    pub date: String,
    pub model_permaslug: String,
    pub total_tokens: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrRankingsDailyMeta`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrRankingsDailyMeta {
    pub as_of: String,
    pub end_date: String,
    pub start_date: String,
    pub version: OrRankingsDailyMetaVersionEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrRankingsDailyMetaVersionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrRankingsDailyMetaVersionEnum {
    #[serde(rename = "v1")]
    V1,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrRankingsDailyMetaVersionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrRankingsDailyResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrRankingsDailyResponse {
    pub data: Vec<OrRankingsDailyItem>,
    pub meta: OrRankingsDailyMeta,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrReasoningConfig`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrReasoningConfig {
    Variant0(ReasoningConfigV0),
    Unknown(serde_json::Value),
}
impl Default for OrReasoningConfig {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrReasoningContext`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrReasoningContext {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrReasoningEffort`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrReasoningEffort {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrReasoningFormat`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrReasoningFormat {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrReasoningItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrReasoningItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<OrReasoningTextContent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrReasoningItemStatusUnion>,
    pub summary: Vec<OrReasoningSummaryText>,
    #[serde(rename = "type")]
    pub type_: OrReasoningItemTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OrReasoningFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrReasoningItemStatusUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrReasoningItemStatusUnion {
    Variant0(OrReasoningItemStatusV0Enum),
    Variant1(OrReasoningItemStatusV1Enum),
    Variant2(OrReasoningItemStatusV2Enum),
    Unknown(serde_json::Value),
}
impl Default for OrReasoningItemStatusUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrReasoningItemStatusV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrReasoningItemStatusV0Enum {
    #[serde(rename = "completed")]
    Completed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrReasoningItemStatusV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrReasoningItemStatusV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrReasoningItemStatusV1Enum {
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrReasoningItemStatusV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrReasoningItemStatusV2Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrReasoningItemStatusV2Enum {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrReasoningItemStatusV2Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrReasoningItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrReasoningItemTypeEnum {
    #[serde(rename = "reasoning")]
    Reasoning,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrReasoningItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrReasoningMode`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrReasoningMode {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrReasoningSummaryText`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrReasoningSummaryText {
    pub text: String,
    #[serde(rename = "type")]
    pub type_: OrReasoningSummaryTextTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrReasoningSummaryTextTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrReasoningSummaryTextTypeEnum {
    #[serde(rename = "summary_text")]
    SummaryText,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrReasoningSummaryTextTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrReasoningSummaryVerbosity`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrReasoningSummaryVerbosity {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrReasoningTextContent`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrReasoningTextContent {
    pub text: String,
    #[serde(rename = "type")]
    pub type_: OrReasoningTextContentTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrReasoningTextContentTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrReasoningTextContentTypeEnum {
    #[serde(rename = "reasoning_text")]
    ReasoningText,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrReasoningTextContentTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrRequestMetadata`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrRequestMetadata {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrResponseHealingPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrResponseHealingPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub id: OrResponseHealingPluginIdEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrResponseHealingPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrResponseHealingPluginIdEnum {
    #[serde(rename = "response-healing")]
    ResponseHealing,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrResponseHealingPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrResponseIncludesEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrResponseIncludesEnum {
    #[serde(rename = "file_search_call.results")]
    FileSearchCallResults,
    #[serde(rename = "message.input_image.image_url")]
    MessageInputImageImageUrl,
    #[serde(rename = "computer_call_output.output.image_url")]
    ComputerCallOutputOutputImageUrl,
    #[serde(rename = "reasoning.encrypted_content")]
    ReasoningEncryptedContent,
    #[serde(rename = "code_interpreter_call.outputs")]
    CodeInterpreterCallOutputs,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrResponseIncludesEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrResponseOutputText`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrResponseOutputText {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<OrOpenAIResponsesAnnotation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<OrResponseOutputTextLogprobsItem>>,
    pub text: String,
    #[serde(rename = "type")]
    pub type_: OrResponseOutputTextTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrResponseOutputTextLogprobsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrResponseOutputTextLogprobsItem {
    pub bytes: Vec<i64>,
    pub logprob: f64,
    pub token: String,
    pub top_logprobs: Vec<OrResponseOutputTextLogprobsItemTopLogprobsItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrResponseOutputTextLogprobsItemTopLogprobsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrResponseOutputTextLogprobsItemTopLogprobsItem {
    pub bytes: Vec<i64>,
    pub logprob: f64,
    pub token: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrResponseOutputTextTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrResponseOutputTextTypeEnum {
    #[serde(rename = "output_text")]
    OutputText,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrResponseOutputTextTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrResponsesErrorField`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrResponsesErrorField {
    pub code: OrResponsesErrorFieldCodeEnum,
    pub message: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrResponsesErrorFieldCodeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrResponsesErrorFieldCodeEnum {
    #[serde(rename = "server_error")]
    ServerError,
    #[serde(rename = "rate_limit_exceeded")]
    RateLimitExceeded,
    #[serde(rename = "invalid_prompt")]
    InvalidPrompt,
    #[serde(rename = "vector_store_timeout")]
    VectorStoreTimeout,
    #[serde(rename = "invalid_image")]
    InvalidImage,
    #[serde(rename = "invalid_image_format")]
    InvalidImageFormat,
    #[serde(rename = "invalid_base64_image")]
    InvalidBase64Image,
    #[serde(rename = "invalid_image_url")]
    InvalidImageUrl,
    #[serde(rename = "image_too_large")]
    ImageTooLarge,
    #[serde(rename = "image_too_small")]
    ImageTooSmall,
    #[serde(rename = "image_parse_error")]
    ImageParseError,
    #[serde(rename = "image_content_policy_violation")]
    ImageContentPolicyViolation,
    #[serde(rename = "invalid_image_mode")]
    InvalidImageMode,
    #[serde(rename = "image_file_too_large")]
    ImageFileTooLarge,
    #[serde(rename = "unsupported_image_media_type")]
    UnsupportedImageMediaType,
    #[serde(rename = "empty_image_file")]
    EmptyImageFile,
    #[serde(rename = "failed_to_download_image")]
    FailedToDownloadImage,
    #[serde(rename = "image_file_not_found")]
    ImageFileNotFound,
    #[serde(rename = "bio_policy")]
    BioPolicy,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrResponsesErrorFieldCodeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrResponsesRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrResponsesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OrAnthropicCacheControlDirective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<OrChatDebugOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_config: Option<OrImageConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<OrResponseIncludesEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<OrInputs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<OrRequestMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<OrOutputModalityEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<OrResponsesRequestPluginsItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<OrStoredPromptTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_options: Option<OrPromptCacheOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<OrProviderPreferences>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OrReasoningConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<OrDeprecatedRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<OrResponsesRequestServiceTierEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_server_tools_when: Option<OrStopServerToolsWhen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<OrTextExtendedConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<OrOpenAIResponsesToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OrResponsesRequestToolsItemUnion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<OrTraceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<OrOpenAIResponsesTruncation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrResponsesRequestPluginsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrResponsesRequestPluginsItemUnion {
    Variant0(OrAutoRouterPlugin),
    Variant1(OrAutoBetaRouterPlugin),
    Variant2(OrModerationPlugin),
    Variant3(OrWebSearchPlugin),
    Variant4(OrWebFetchPlugin),
    Variant5(OrFileParserPlugin),
    Variant6(OrResponseHealingPlugin),
    Variant7(OrContextCompressionPlugin),
    Variant8(OrParetoRouterPlugin),
    Variant9(OrFusionPlugin),
    Unknown(serde_json::Value),
}
impl Default for OrResponsesRequestPluginsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrResponsesRequestServiceTierEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrResponsesRequestServiceTierEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "flex")]
    Flex,
    #[serde(rename = "priority")]
    Priority,
    #[serde(rename = "scale")]
    Scale,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrResponsesRequestServiceTierEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrResponsesRequestToolsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrResponsesRequestToolsItemUnion {
    Variant0(OrResponsesRequestToolsItemV0),
    Variant1(OrPreviewWebSearchServerTool),
    Variant2(OrPreview20250311WebSearchServerTool),
    Variant3(OrLegacyWebSearchServerTool),
    Variant4(OrWebSearchServerTool),
    Variant5(OrFileSearchServerTool),
    Variant6(OrComputerUseServerTool),
    Variant7(OrCodeInterpreterServerTool),
    Variant8(OrMcpServerTool),
    Variant9(OrImageGenerationServerTool),
    Variant10(OrCodexLocalShellTool),
    Variant11(OrShellServerTool),
    Variant12(OrApplyPatchServerTool),
    Variant13(OrCustomTool),
    Variant14(OrNamespaceTool),
    Variant15(OrAdvisorServerToolOpenRouter),
    Variant16(OrSubagentServerToolOpenRouter),
    Variant17(OrDatetimeServerTool),
    Variant18(OrFilesServerTool),
    Variant19(OrFusionServerToolOpenRouter),
    Variant20(OrImageGenerationServerToolOpenRouter),
    Variant21(OrSearchModelsServerToolOpenRouter),
    Variant22(OrWebFetchServerTool),
    Variant23(OrWebSearchServerToolOpenRouter),
    Variant24(OrApplyPatchServerToolOpenRouter),
    Variant25(OrBashServerTool),
    Variant26(OrShellServerToolOpenRouter),
    Unknown(serde_json::Value),
}
impl Default for OrResponsesRequestToolsItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrResponsesRequestToolsItemV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrResponsesRequestToolsItemV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(rename = "type")]
    pub type_: OrResponsesRequestToolsItemV0TypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrResponsesRequestToolsItemV0TypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrResponsesRequestToolsItemV0TypeEnum {
    #[serde(rename = "function")]
    Function,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrResponsesRequestToolsItemV0TypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrRouterAttempt`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrRouterAttempt {
    pub model: String,
    pub provider: String,
    pub status: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrRouterParams`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrRouterParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_floor: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throughput_floor: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_group: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrRoutingStrategy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrRoutingStrategy {
    #[serde(rename = "direct")]
    Direct,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "free")]
    Free,
    #[serde(rename = "latest")]
    Latest,
    #[serde(rename = "alias")]
    Alias,
    #[serde(rename = "fallback")]
    Fallback,
    #[serde(rename = "pareto")]
    Pareto,
    #[serde(rename = "bodybuilder")]
    Bodybuilder,
    #[serde(rename = "fusion")]
    Fusion,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrRoutingStrategy {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrSTTInputAudio`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSTTInputAudio {
    pub data: String,
    pub format: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSTTRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSTTRequest {
    pub input_audio: OrSTTInputAudio,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<OrSTTRequestProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<OrSTTRequestResponseFormatEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_granularities: Option<Vec<OrSTTTimestampGranularity>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSTTRequestProvider`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSTTRequestProvider {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<OrProviderOptions>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrSTTRequestResponseFormatEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSTTRequestResponseFormatEnum {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "verbose_json")]
    VerboseJson,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSTTRequestResponseFormatEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrSTTResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSTTResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<OrSTTSegment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<OrSTTUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<OrSTTWord>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSTTSegment`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSTTSegment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_logprob: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression_ratio: Option<f64>,
    pub end: f64,
    pub id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_speech_prob: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seek: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<i64>,
    pub start: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<i64>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrSTTTimestampGranularity`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSTTTimestampGranularity {
    #[serde(rename = "word")]
    Word,
    #[serde(rename = "segment")]
    Segment,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSTTTimestampGranularity {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrSTTUsage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSTTUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSTTWord`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSTTWord {
    pub end: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<i64>,
    pub start: f64,
    pub word: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSandboxSleepAfterSeconds`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSandboxSleepAfterSeconds {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrSearchContextSizeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSearchContextSizeEnum {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSearchContextSizeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrSearchModelsServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSearchModelsServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSearchModelsServerToolOpenRouter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSearchModelsServerToolOpenRouter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrSearchModelsServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrSearchModelsServerToolOpenRouterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrSearchModelsServerToolOpenRouterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSearchModelsServerToolOpenRouterTypeEnum {
    #[serde(rename = "openrouter:experimental__search_models")]
    OpenrouterExperimentalSearchModels,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSearchModelsServerToolOpenRouterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrSearchQualityLevel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSearchQualityLevel {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSearchQualityLevel {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrServerToolUseDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrServerToolUseDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls_executed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls_requested: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search_requests: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrShellCallItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrShellCallItem {
    pub action: OrShellCallItemAction,
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrToolCallStatus>,
    #[serde(rename = "type")]
    pub type_: OrShellCallItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrShellCallItemAction`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrShellCallItemAction {
    pub commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_length: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrShellCallItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrShellCallItemTypeEnum {
    #[serde(rename = "shell_call")]
    ShellCall,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrShellCallItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrShellCallOutputItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrShellCallOutputItem {
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_length: Option<i64>,
    pub output: Vec<OrShellCallOutputItemOutputItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OrToolCallStatus>,
    #[serde(rename = "type")]
    pub type_: OrShellCallOutputItemTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrShellCallOutputItemOutputItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrShellCallOutputItemOutputItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrShellCallOutputItemTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrShellCallOutputItemTypeEnum {
    #[serde(rename = "shell_call_output")]
    ShellCallOutput,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrShellCallOutputItemTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrShellCallStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrShellCallStatus {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrShellCallStatus {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrShellServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrShellServerTool {
    #[serde(rename = "type")]
    pub type_: OrShellServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrShellServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrShellServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrShellServerToolEngine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<OrShellServerToolEnvironment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sleep_after_seconds: Option<OrSandboxSleepAfterSeconds>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrShellServerToolEngine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrShellServerToolEngine {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "openrouter")]
    Openrouter,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrShellServerToolEngine {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrShellServerToolEnvironment`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrShellServerToolEnvironment {
    Variant0(OrContainerAutoEnvironment),
    Variant1(OrContainerReferenceEnvironment),
    Unknown(serde_json::Value),
}
impl Default for OrShellServerToolEnvironment {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrShellServerToolOpenRouter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrShellServerToolOpenRouter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrShellServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrShellServerToolOpenRouterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrShellServerToolOpenRouterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrShellServerToolOpenRouterTypeEnum {
    #[serde(rename = "openrouter:shell")]
    OpenrouterShell,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrShellServerToolOpenRouterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrShellServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrShellServerToolTypeEnum {
    #[serde(rename = "shell")]
    Shell,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrShellServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrSpeechRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSpeechRequest {
    pub input: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<OrSpeechRequestProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<OrSpeechRequestResponseFormatEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    pub voice: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSpeechRequestProvider`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSpeechRequestProvider {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<OrProviderOptions>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrSpeechRequestResponseFormatEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSpeechRequestResponseFormatEnum {
    #[serde(rename = "mp3")]
    Mp3,
    #[serde(rename = "pcm")]
    Pcm,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSpeechRequestResponseFormatEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrStopServerToolsWhen`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrStopServerToolsWhen {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrStoredPromptTemplate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrStoredPromptTemplate {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables:
        Option<std::collections::BTreeMap<String, OrStoredPromptTemplateVariablesValueUnion>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrStoredPromptTemplateVariablesValueUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrStoredPromptTemplateVariablesValueUnion {
    Variant0(String),
    Variant1(OrInputText),
    Variant2(OrInputImage),
    Variant3(OrInputFile),
    Unknown(serde_json::Value),
}
impl Default for OrStoredPromptTemplateVariablesValueUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrSubagentNestedTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSubagentNestedTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSubagentReasoning`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSubagentReasoning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<OrSubagentReasoningEffortEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrSubagentReasoningEffortEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSubagentReasoningEffortEnum {
    #[serde(rename = "max")]
    Max,
    #[serde(rename = "xhigh")]
    Xhigh,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSubagentReasoningEffortEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrSubagentServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSubagentServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OrSubagentReasoning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OrSubagentNestedTool>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSubagentServerToolOpenRouter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSubagentServerToolOpenRouter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrSubagentServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrSubagentServerToolOpenRouterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrSubagentServerToolOpenRouterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSubagentServerToolOpenRouterTypeEnum {
    #[serde(rename = "openrouter:subagent")]
    OpenrouterSubagent,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSubagentServerToolOpenRouterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrSubagentSessionItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSubagentSessionItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSubmitGenerationFeedbackRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSubmitGenerationFeedbackRequest {
    pub category: OrSubmitGenerationFeedbackRequestCategoryEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub generation_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrSubmitGenerationFeedbackRequestCategoryEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrSubmitGenerationFeedbackRequestCategoryEnum {
    #[serde(rename = "latency")]
    Latency,
    #[serde(rename = "incoherence")]
    Incoherence,
    #[serde(rename = "incorrect_response")]
    IncorrectResponse,
    #[serde(rename = "formatting")]
    Formatting,
    #[serde(rename = "billing")]
    Billing,
    #[serde(rename = "api_error")]
    ApiError,
    #[serde(rename = "other")]
    Other,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrSubmitGenerationFeedbackRequestCategoryEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrSubmitGenerationFeedbackResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSubmitGenerationFeedbackResponse {
    pub data: OrSubmitGenerationFeedbackResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSubmitGenerationFeedbackResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSubmitGenerationFeedbackResponseData {
    pub success: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrSupportedParameters`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrSupportedParameters {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrTaskClassificationItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTaskClassificationItem {
    pub category_token_share: f64,
    pub category_usage_share: f64,
    pub display_name: String,
    pub macro_category: String,
    pub models: Vec<OrTaskClassificationModel>,
    pub tag: String,
    pub token_share: f64,
    pub usage_share: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrTaskClassificationMacroCategory`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTaskClassificationMacroCategory {
    pub key: String,
    pub label: String,
    pub token_share: f64,
    pub usage_share: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrTaskClassificationModel`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTaskClassificationModel {
    pub id: String,
    pub tag_token_share: f64,
    pub tag_usage_share: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrTaskClassificationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTaskClassificationResponse {
    pub data: OrTaskClassificationResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrTaskClassificationResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTaskClassificationResponseData {
    pub as_of: String,
    pub classifications: Vec<OrTaskClassificationItem>,
    pub macro_categories: Vec<OrTaskClassificationMacroCategory>,
    pub window_days: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrTextExtendedConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTextExtendedConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OrFormats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<OrTextExtendedConfigVerbosityEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrTextExtendedConfigVerbosityEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrTextExtendedConfigVerbosityEnum {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    Xhigh,
    #[serde(rename = "max")]
    Max,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrTextExtendedConfigVerbosityEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrToolCallStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrToolCallStatus {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrToolCallStatus {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrToolChoiceAllowed`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrToolChoiceAllowed {
    pub mode: OrToolChoiceAllowedModeUnion,
    pub tools: Vec<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "type")]
    pub type_: OrToolChoiceAllowedTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrToolChoiceAllowedModeUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrToolChoiceAllowedModeUnion {
    Variant0(OrToolChoiceAllowedModeV0Enum),
    Variant1(OrToolChoiceAllowedModeV1Enum),
    Unknown(serde_json::Value),
}
impl Default for OrToolChoiceAllowedModeUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated string enum `OrToolChoiceAllowedModeV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrToolChoiceAllowedModeV0Enum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrToolChoiceAllowedModeV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrToolChoiceAllowedModeV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrToolChoiceAllowedModeV1Enum {
    #[serde(rename = "required")]
    Required,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrToolChoiceAllowedModeV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrToolChoiceAllowedTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrToolChoiceAllowedTypeEnum {
    #[serde(rename = "allowed_tools")]
    AllowedTools,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrToolChoiceAllowedTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrTopProviderInfo`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTopProviderInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<i64>,
    pub is_moderated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrTraceConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTraceConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrTruncation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrTruncation {
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrURLCitation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrURLCitation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub end_index: i64,
    pub start_index: i64,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: OrURLCitationTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrURLCitationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrURLCitationTypeEnum {
    #[serde(rename = "url_citation")]
    UrlCitation,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrURLCitationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrUnifiedBenchmarkPricing`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUnifiedBenchmarkPricing {
    pub completion: String,
    pub prompt: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUnifiedBenchmarksAAItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUnifiedBenchmarksAAItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agentic_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coding_index: Option<f64>,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intelligence_index: Option<f64>,
    pub model_permaslug: String,
    pub pricing: OrUnifiedBenchmarkPricing,
    pub source: OrUnifiedBenchmarksAAItemSourceEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrUnifiedBenchmarksAAItemSourceEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrUnifiedBenchmarksAAItemSourceEnum {
    #[serde(rename = "artificial-analysis")]
    ArtificialAnalysis,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrUnifiedBenchmarksAAItemSourceEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrUnifiedBenchmarksDAItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUnifiedBenchmarksDAItem {
    pub arena: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_generation_time_ms: Option<f64>,
    pub category: String,
    pub display_name: String,
    pub elo: f64,
    pub model_permaslug: String,
    pub pricing: OrUnifiedBenchmarkPricing,
    pub source: OrUnifiedBenchmarksDAItemSourceEnum,
    pub tournament_stats: OrUnifiedBenchmarksDAItemTournamentStats,
    pub win_rate: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrUnifiedBenchmarksDAItemSourceEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrUnifiedBenchmarksDAItemSourceEnum {
    #[serde(rename = "design-arena")]
    DesignArena,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrUnifiedBenchmarksDAItemSourceEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrUnifiedBenchmarksDAItemTournamentStats`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUnifiedBenchmarksDAItemTournamentStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_place: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fourth_place: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_place: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub third_place: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUnifiedBenchmarksMeta`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUnifiedBenchmarksMeta {
    pub as_of: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citation: Option<String>,
    pub model_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<OrUnifiedBenchmarksMetaSourceEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    pub version: OrUnifiedBenchmarksMetaVersionEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrUnifiedBenchmarksMetaSourceEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrUnifiedBenchmarksMetaSourceEnum {
    #[serde(rename = "artificial-analysis")]
    ArtificialAnalysis,
    #[serde(rename = "design-arena")]
    DesignArena,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrUnifiedBenchmarksMetaSourceEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrUnifiedBenchmarksMetaVersionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrUnifiedBenchmarksMetaVersionEnum {
    #[serde(rename = "v1")]
    V1,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrUnifiedBenchmarksMetaVersionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrUnifiedBenchmarksResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUnifiedBenchmarksResponse {
    pub data: Vec<OrUnifiedBenchmarksResponseDataItemUnion>,
    pub meta: OrUnifiedBenchmarksMeta,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `OrUnifiedBenchmarksResponseDataItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrUnifiedBenchmarksResponseDataItemUnion {
    Variant0(OrUnifiedBenchmarksAAItem),
    Variant1(OrUnifiedBenchmarksDAItem),
    Unknown(serde_json::Value),
}
impl Default for OrUnifiedBenchmarksResponseDataItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrUpdateBYOKKeyRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateBYOKKeyRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_fallback: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateBYOKKeyResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateBYOKKeyResponse {
    pub data: OrUpdateBYOKKeyResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateBYOKKeyResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateBYOKKeyResponseData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_api_key_hashes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_user_ids: Option<Vec<String>>,
    pub created_at: String,
    pub disabled: bool,
    pub id: String,
    pub is_fallback: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub provider: OrBYOKProviderSlug,
    pub sort_order: i64,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateGuardrailRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateGuardrailRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_providers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filter_builtins: Option<Vec<OrContentFilterBuiltinEntryInput>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filters: Option<Vec<OrContentFilterEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_anthropic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_google: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_openai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_other: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_xai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_providers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_byok_in_budgets: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_interval: Option<OrGuardrailInterval>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateGuardrailResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateGuardrailResponse {
    pub data: OrUpdateGuardrailResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateGuardrailResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateGuardrailResponseData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_providers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filter_builtins: Option<Vec<OrContentFilterBuiltinEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_filters: Option<Vec<OrContentFilterEntry>>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_anthropic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_google: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_openai: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_other: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_zdr_xai: Option<bool>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored_providers: Option<Vec<String>>,
    pub include_byok_in_budgets: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_usd: Option<f64>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_interval: Option<OrGuardrailInterval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateObservabilityDestinationRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateObservabilityDestinationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_hashes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_rules: Option<OrUpdateObservabilityDestinationRequestFilterRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_rate: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateObservabilityDestinationRequestFilterRules`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateObservabilityDestinationRequestFilterRules {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub groups: Vec<OrObservabilityFilterRuleGroup>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateObservabilityDestinationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateObservabilityDestinationResponse {
    pub data: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateWorkspaceRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateWorkspaceRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_image_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider_sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_text_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_logging_api_key_ids: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_logging_sampling_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_data_discount_logging_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_observability_broadcast_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_observability_io_logging_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateWorkspaceResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateWorkspaceResponse {
    pub data: OrUpdateWorkspaceResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpdateWorkspaceResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpdateWorkspaceResponseData {
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    pub default_guardrail_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_image_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider_sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_text_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_logging_api_key_ids: Option<Vec<i64>>,
    pub io_logging_sampling_rate: f64,
    pub is_data_discount_logging_enabled: bool,
    pub is_observability_broadcast_enabled: bool,
    pub is_observability_io_logging_enabled: bool,
    pub name: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpsertWorkspaceBudgetRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpsertWorkspaceBudgetRequest {
    pub limit_usd: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpsertWorkspaceBudgetResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpsertWorkspaceBudgetResponse {
    pub data: OrUpsertWorkspaceBudgetResponseData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrUpsertWorkspaceBudgetResponseData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrUpsertWorkspaceBudgetResponseData {
    pub created_at: String,
    pub id: String,
    pub limit_usd: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_interval: Option<OrUpsertWorkspaceBudgetResponseDataResetIntervalEnum>,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrUpsertWorkspaceBudgetResponseDataResetIntervalEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrUpsertWorkspaceBudgetResponseDataResetIntervalEnum {
    #[serde(rename = "daily")]
    Daily,
    #[serde(rename = "weekly")]
    Weekly,
    #[serde(rename = "monthly")]
    Monthly,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrUpsertWorkspaceBudgetResponseDataResetIntervalEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated union `OrUsage`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrUsage {
    Variant0(UsageV0),
    Unknown(serde_json::Value),
}
impl Default for OrUsage {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `OrVideoGenerationRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrVideoGenerationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<OrVideoGenerationRequestAspectRatioEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_images: Option<Vec<OrFrameImage>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generate_audio: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_references: Option<Vec<OrInputReference>>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<OrVideoGenerationRequestProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<OrVideoGenerationRequestResolutionEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrVideoGenerationRequestAspectRatioEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrVideoGenerationRequestAspectRatioEnum {
    #[serde(rename = "16:9")]
    T169,
    #[serde(rename = "9:16")]
    T916,
    #[serde(rename = "1:1")]
    T11,
    #[serde(rename = "4:3")]
    T43,
    #[serde(rename = "3:4")]
    T34,
    #[serde(rename = "3:2")]
    T32,
    #[serde(rename = "2:3")]
    T23,
    #[serde(rename = "21:9")]
    T219,
    #[serde(rename = "9:21")]
    T921,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrVideoGenerationRequestAspectRatioEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrVideoGenerationRequestProvider`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrVideoGenerationRequestProvider {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<OrVideoGenerationRequestProviderOptions>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrVideoGenerationRequestProviderOptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrVideoGenerationRequestProviderOptions {
    #[serde(rename = "01ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_01ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai21: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "aion-labs")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aion_labs: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub akashml: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alibaba: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "amazon-bedrock")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amazon_bedrock: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "amazon-nova")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amazon_nova: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anyscale: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "arcee-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcee_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "atlas-cloud")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas_cloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atoma: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avian: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baidu: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseten: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "black-forest-labs")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub black_forest_labs: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byteplus: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centml: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cerebras: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chutes: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cirrascale: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarifai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflare: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohere: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coreweave: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crofai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crucible: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crusoe: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub darkbloom: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decart: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepgram: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepinfra: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepseek: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dekallm: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digitalocean: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enfer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "fake-provider")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fake_provider: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featherless: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fireworks: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "fish-audio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fish_audio: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub friendli: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gmicloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "google-ai-studio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_ai_studio: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "google-vertex")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_vertex: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gopomelo: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groq: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heygen: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huggingface: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperbolic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "hyperbolic-quantized")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperbolic_quantized: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inception: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inceptron: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "inferact-vllm")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inferact_vllm: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "inference-net")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_net: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infermatic: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inflection: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inocloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "io-net")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_net: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ionstream: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub klusterai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub krea: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lepton: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lynn: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "lynn-private")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lynn_private: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mancer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "mancer-old")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mancer_old: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mara: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimax: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mistral: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelrun: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modular: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moonshotai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morph: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ncompass: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nebius: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "nex-agi")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nex_agi: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nextbit: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nineteen: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub novita: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub octoai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "open-inference")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_inference: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parasail: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perceptron: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perplexity: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phala: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poolside: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiver: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recraft: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursal: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reka: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relace: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runway: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sail-research")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sail_research: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sakana: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sakana-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sakana_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sambanova: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sambanova-cloaked")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sambanova_cloaked: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "sf-compute")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sf_compute: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub siliconflow: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sourceful: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stealth: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stepfun: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streamlake: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switchpoint: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targon: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tencent: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenstorrent: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub together: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "together-lite")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub together_lite: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ubicloud: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstage: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venice: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wafer: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wandb: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "wandb-legacy")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wandb_legacy: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xiaomi: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "z-ai")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_ai: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrVideoGenerationRequestResolutionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrVideoGenerationRequestResolutionEnum {
    #[serde(rename = "480p")]
    T480p,
    #[serde(rename = "720p")]
    T720p,
    #[serde(rename = "1080p")]
    T1080p,
    #[serde(rename = "1K")]
    T1K,
    #[serde(rename = "2K")]
    T2K,
    #[serde(rename = "4K")]
    T4K,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrVideoGenerationRequestResolutionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrVideoGenerationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrVideoGenerationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_id: Option<String>,
    pub id: String,
    pub polling_url: String,
    pub status: OrVideoGenerationResponseStatusEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsigned_urls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<OrVideoGenerationUsage>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrVideoGenerationResponseStatusEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrVideoGenerationResponseStatusEnum {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "expired")]
    Expired,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrVideoGenerationResponseStatusEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrVideoGenerationUsage`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrVideoGenerationUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_byok: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrVideoModel`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrVideoModel {
    pub allowed_passthrough_parameters: Vec<String>,
    pub canonical_slug: String,
    pub created: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generate_audio: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hugging_face_id: Option<String>,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing_skus: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_aspect_ratios: Option<Vec<OrVideoModelSupportedAspectRatiosItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_durations: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_frame_images: Option<Vec<OrVideoModelSupportedFrameImagesItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_resolutions: Option<Vec<OrVideoModelSupportedResolutionsItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_sizes: Option<Vec<OrVideoModelSupportedSizesItemEnum>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrVideoModelSupportedAspectRatiosItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrVideoModelSupportedAspectRatiosItemEnum {
    #[serde(rename = "16:9")]
    T169,
    #[serde(rename = "9:16")]
    T916,
    #[serde(rename = "1:1")]
    T11,
    #[serde(rename = "4:3")]
    T43,
    #[serde(rename = "3:4")]
    T34,
    #[serde(rename = "3:2")]
    T32,
    #[serde(rename = "2:3")]
    T23,
    #[serde(rename = "21:9")]
    T219,
    #[serde(rename = "9:21")]
    T921,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrVideoModelSupportedAspectRatiosItemEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrVideoModelSupportedFrameImagesItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrVideoModelSupportedFrameImagesItemEnum {
    #[serde(rename = "first_frame")]
    FirstFrame,
    #[serde(rename = "last_frame")]
    LastFrame,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrVideoModelSupportedFrameImagesItemEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrVideoModelSupportedResolutionsItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrVideoModelSupportedResolutionsItemEnum {
    #[serde(rename = "480p")]
    T480p,
    #[serde(rename = "720p")]
    T720p,
    #[serde(rename = "1080p")]
    T1080p,
    #[serde(rename = "1K")]
    T1K,
    #[serde(rename = "2K")]
    T2K,
    #[serde(rename = "4K")]
    T4K,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrVideoModelSupportedResolutionsItemEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrVideoModelSupportedSizesItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrVideoModelSupportedSizesItemEnum {
    #[serde(rename = "480x480")]
    T480x480,
    #[serde(rename = "480x640")]
    T480x640,
    #[serde(rename = "480x720")]
    T480x720,
    #[serde(rename = "480x854")]
    T480x854,
    #[serde(rename = "480x1120")]
    T480x1120,
    #[serde(rename = "640x480")]
    T640x480,
    #[serde(rename = "720x480")]
    T720x480,
    #[serde(rename = "720x720")]
    T720x720,
    #[serde(rename = "720x960")]
    T720x960,
    #[serde(rename = "720x1080")]
    T720x1080,
    #[serde(rename = "720x1280")]
    T720x1280,
    #[serde(rename = "720x1680")]
    T720x1680,
    #[serde(rename = "854x480")]
    T854x480,
    #[serde(rename = "960x720")]
    T960x720,
    #[serde(rename = "1080x720")]
    T1080x720,
    #[serde(rename = "1080x1080")]
    T1080x1080,
    #[serde(rename = "1080x1440")]
    T1080x1440,
    #[serde(rename = "1080x1620")]
    T1080x1620,
    #[serde(rename = "1080x1920")]
    T1080x1920,
    #[serde(rename = "1080x2520")]
    T1080x2520,
    #[serde(rename = "1120x480")]
    T1120x480,
    #[serde(rename = "1280x720")]
    T1280x720,
    #[serde(rename = "1440x1080")]
    T1440x1080,
    #[serde(rename = "1620x1080")]
    T1620x1080,
    #[serde(rename = "1680x720")]
    T1680x720,
    #[serde(rename = "1920x1080")]
    T1920x1080,
    #[serde(rename = "2160x2160")]
    T2160x2160,
    #[serde(rename = "2160x2880")]
    T2160x2880,
    #[serde(rename = "2160x3240")]
    T2160x3240,
    #[serde(rename = "2160x3840")]
    T2160x3840,
    #[serde(rename = "2160x5040")]
    T2160x5040,
    #[serde(rename = "2520x1080")]
    T2520x1080,
    #[serde(rename = "2880x2160")]
    T2880x2160,
    #[serde(rename = "3240x2160")]
    T3240x2160,
    #[serde(rename = "3840x2160")]
    T3840x2160,
    #[serde(rename = "5040x2160")]
    T5040x2160,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrVideoModelSupportedSizesItemEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrVideoModelsListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrVideoModelsListResponse {
    pub data: Vec<OrVideoModel>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebFetchEngineEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebFetchEngineEnum {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "openrouter")]
    Openrouter,
    #[serde(rename = "exa")]
    Exa,
    #[serde(rename = "parallel")]
    Parallel,
    #[serde(rename = "firecrawl")]
    Firecrawl,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebFetchEngineEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWebFetchPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebFetchPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
    pub id: OrWebFetchPluginIdEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_content_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebFetchPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebFetchPluginIdEnum {
    #[serde(rename = "web-fetch")]
    WebFetch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebFetchPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWebFetchServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebFetchServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrWebFetchServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrWebFetchServerToolTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrWebFetchServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebFetchServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebFetchEngineEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_content_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebFetchServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebFetchServerToolTypeEnum {
    #[serde(rename = "openrouter:web_fetch")]
    OpenrouterWebFetch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebFetchServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWebSearchConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebSearchEngineEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_characters: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<OrSearchQualityLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrWebSearchUserLocationServerTool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrWebSearchDomainFilter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchDomainFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_domains: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebSearchEngine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchEngine {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "exa")]
    Exa,
    #[serde(rename = "firecrawl")]
    Firecrawl,
    #[serde(rename = "parallel")]
    Parallel,
    #[serde(rename = "perplexity")]
    Perplexity,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchEngine {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrWebSearchEngineEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchEngineEnum {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "exa")]
    Exa,
    #[serde(rename = "parallel")]
    Parallel,
    #[serde(rename = "firecrawl")]
    Firecrawl,
    #[serde(rename = "perplexity")]
    Perplexity,
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchEngineEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWebSearchPlugin`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchPlugin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebSearchEngine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_domains: Option<Vec<String>>,
    pub id: OrWebSearchPluginIdEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrWebSearchPluginUserLocation>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebSearchPluginIdEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchPluginIdEnum {
    #[serde(rename = "web")]
    Web,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchPluginIdEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWebSearchPluginUserLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchPluginUserLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(rename = "type")]
    pub type_: OrWebSearchPluginUserLocationTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebSearchPluginUserLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchPluginUserLocationTypeEnum {
    #[serde(rename = "approximate")]
    Approximate,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchPluginUserLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWebSearchServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebSearchEngineEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<OrWebSearchDomainFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<OrSearchContextSizeEnum>,
    #[serde(rename = "type")]
    pub type_: OrWebSearchServerToolTypeEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrWebSearchUserLocation>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrWebSearchServerToolConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchServerToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<OrWebSearchEngineEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_characters: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_results: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<OrSearchQualityLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<OrWebSearchUserLocationServerTool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrWebSearchServerToolOpenRouter`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchServerToolOpenRouter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<OrWebSearchServerToolConfig>,
    #[serde(rename = "type")]
    pub type_: OrWebSearchServerToolOpenRouterTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebSearchServerToolOpenRouterTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchServerToolOpenRouterTypeEnum {
    #[serde(rename = "openrouter:web_search")]
    OpenrouterWebSearch,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchServerToolOpenRouterTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrWebSearchServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchServerToolTypeEnum {
    #[serde(rename = "web_search_2025_08_26")]
    WebSearch20250826,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWebSearchSource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchSource {
    #[serde(rename = "type")]
    pub type_: OrWebSearchSourceTypeEnum,
    pub url: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebSearchSourceTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchSourceTypeEnum {
    #[serde(rename = "url")]
    Url,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchSourceTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrWebSearchStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "searching")]
    Searching,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "failed")]
    Failed,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchStatus {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWebSearchUserLocation`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchUserLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<OrWebSearchUserLocationTypeEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrWebSearchUserLocationServerTool`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWebSearchUserLocationServerTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<OrWebSearchUserLocationServerToolTypeEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWebSearchUserLocationServerToolTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchUserLocationServerToolTypeEnum {
    #[serde(rename = "approximate")]
    Approximate,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchUserLocationServerToolTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `OrWebSearchUserLocationTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWebSearchUserLocationTypeEnum {
    #[serde(rename = "approximate")]
    Approximate,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWebSearchUserLocationTypeEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWorkspace`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWorkspace {
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    pub default_guardrail_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_image_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider_sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_text_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_logging_api_key_ids: Option<Vec<i64>>,
    pub io_logging_sampling_rate: f64,
    pub is_data_discount_logging_enabled: bool,
    pub is_observability_broadcast_enabled: bool,
    pub is_observability_io_logging_enabled: bool,
    pub name: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrWorkspaceBudget`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWorkspaceBudget {
    pub created_at: String,
    pub id: String,
    pub limit_usd: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_interval: Option<OrWorkspaceBudgetResetIntervalEnum>,
    pub updated_at: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWorkspaceBudgetResetIntervalEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWorkspaceBudgetResetIntervalEnum {
    #[serde(rename = "daily")]
    Daily,
    #[serde(rename = "weekly")]
    Weekly,
    #[serde(rename = "monthly")]
    Monthly,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWorkspaceBudgetResetIntervalEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `OrWorkspaceMember`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrWorkspaceMember {
    pub created_at: String,
    pub id: String,
    pub role: OrWorkspaceMemberRoleEnum,
    pub user_id: String,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrWorkspaceMemberRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrWorkspaceMemberRoleEnum {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "member")]
    Member,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for OrWorkspaceMemberRoleEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `PDFParserEngineV0Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PDFParserEngineV0Enum {
    #[serde(rename = "mistral-ocr")]
    MistralOcr,
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "cloudflare-ai")]
    CloudflareAi,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for PDFParserEngineV0Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated string enum `PDFParserEngineV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PDFParserEngineV1Enum {
    #[serde(rename = "pdf-text")]
    PdfText,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for PDFParserEngineV1Enum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `QueryAnalyticsBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifier_dimensions: Option<QueryAnalyticsBodyClassifierDimensions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifier_filters: Option<QueryAnalyticsBodyClassifierFilters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<Vec<QueryAnalyticsBodyFiltersItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granularity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    pub metrics: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_by: Option<QueryAnalyticsBodyOrderBy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_range: Option<QueryAnalyticsBodyTimeRange>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `QueryAnalyticsBodyClassifierDimensions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsBodyClassifierDimensions {
    pub classifier_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension_names: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_nulls: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `QueryAnalyticsBodyClassifierFilters`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsBodyClassifierFilters {
    pub classifier_id: String,
    pub filters: Vec<QueryAnalyticsBodyClassifierFiltersFiltersItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `QueryAnalyticsBodyClassifierFiltersFiltersItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsBodyClassifierFiltersFiltersItem {
    pub field: String,
    pub operator: String,
    pub value: QueryAnalyticsBodyClassifierFiltersFiltersItemValueUnion,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `QueryAnalyticsBodyClassifierFiltersFiltersItemValueUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryAnalyticsBodyClassifierFiltersFiltersItemValueUnion {
    Variant0(String),
    Variant1(f64),
    Variant2(Vec<QueryAnalyticsBodyClassifierFiltersFiltersItemValueV2ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for QueryAnalyticsBodyClassifierFiltersFiltersItemValueUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `QueryAnalyticsBodyClassifierFiltersFiltersItemValueV2ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryAnalyticsBodyClassifierFiltersFiltersItemValueV2ItemUnion {
    Variant0(String),
    Variant1(f64),
    Unknown(serde_json::Value),
}
impl Default for QueryAnalyticsBodyClassifierFiltersFiltersItemValueV2ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `QueryAnalyticsBodyFiltersItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsBodyFiltersItem {
    pub field: String,
    pub operator: String,
    pub value: QueryAnalyticsBodyFiltersItemValueUnion,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `QueryAnalyticsBodyFiltersItemValueUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryAnalyticsBodyFiltersItemValueUnion {
    Variant0(String),
    Variant1(f64),
    Variant2(Vec<QueryAnalyticsBodyFiltersItemValueV2ItemUnion>),
    Unknown(serde_json::Value),
}
impl Default for QueryAnalyticsBodyFiltersItemValueUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated union `QueryAnalyticsBodyFiltersItemValueV2ItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryAnalyticsBodyFiltersItemValueV2ItemUnion {
    Variant0(String),
    Variant1(f64),
    Unknown(serde_json::Value),
}
impl Default for QueryAnalyticsBodyFiltersItemValueV2ItemUnion {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Generated object `QueryAnalyticsBodyOrderBy`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsBodyOrderBy {
    pub direction: QueryAnalyticsBodyOrderByDirectionEnum,
    pub field: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `QueryAnalyticsBodyOrderByDirectionEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryAnalyticsBodyOrderByDirectionEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for QueryAnalyticsBodyOrderByDirectionEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Generated object `QueryAnalyticsBodyTimeRange`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsBodyTimeRange {
    pub end: String,
    pub start: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /analytics/query` (`queryAnalytics`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsParams {
    pub body: QueryAnalyticsBody,
}

/// JSON result for `queryAnalytics`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsResult {
    #[serde(flatten)]
    pub body: QueryAnalyticsResultBody,
}

/// Generated object `QueryAnalyticsResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsResultBody {
    pub data: QueryAnalyticsResultBodyData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `QueryAnalyticsResultBodyData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsResultBodyData {
    #[serde(rename = "cachedAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_at: Option<f64>,
    pub data: Vec<std::collections::BTreeMap<String, serde_json::Value>>,
    pub metadata: QueryAnalyticsResultBodyDataMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `QueryAnalyticsResultBodyDataMetadata`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QueryAnalyticsResultBodyDataMetadata {
    pub query_time_ms: f64,
    pub row_count: i64,
    pub truncated: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ReasoningConfigV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReasoningConfigV0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<OrReasoningContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<OrReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<OrReasoningMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<OrReasoningSummaryVerbosity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /chat/completions` (`sendChatCompletionRequest`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SendChatCompletionRequestParams {
    pub body: OrChatRequest,
}

/// JSON result for `sendChatCompletionRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SendChatCompletionRequestResult {
    #[serde(flatten)]
    pub body: OrChatResult,
}

/// SSE event stream for `sendChatCompletionRequest` (all frames preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendChatCompletionRequestSseResult {
    pub events: Vec<SseEvent>,
}

/// Typed params for `POST /generation/feedback` (`submitGenerationFeedback`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubmitGenerationFeedbackParams {
    pub body: OrSubmitGenerationFeedbackRequest,
}

/// JSON result for `submitGenerationFeedback`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubmitGenerationFeedbackResult {
    #[serde(flatten)]
    pub body: OrSubmitGenerationFeedbackResponse,
}

/// Typed params for `PATCH /byok/{id}` (`updateBYOKKey`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateBYOKKeyParams {
    pub id: String,
    pub body: OrUpdateBYOKKeyRequest,
}

/// JSON result for `updateBYOKKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateBYOKKeyResult {
    #[serde(flatten)]
    pub body: OrUpdateBYOKKeyResponse,
}

/// Typed params for `PATCH /guardrails/{id}` (`updateGuardrail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateGuardrailParams {
    pub id: String,
    pub body: OrUpdateGuardrailRequest,
}

/// JSON result for `updateGuardrail`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateGuardrailResult {
    #[serde(flatten)]
    pub body: OrUpdateGuardrailResponse,
}

/// Generated object `UpdateKeysBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateKeysBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_byok_in_limit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reset: Option<UpdateKeysBodyLimitResetEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UpdateKeysBodyLimitResetEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateKeysBodyLimitResetEnum {
    #[serde(rename = "daily")]
    Daily,
    #[serde(rename = "weekly")]
    Weekly,
    #[serde(rename = "monthly")]
    Monthly,
    #[serde(untagged)]
    UnknownValue(String),
}
impl Default for UpdateKeysBodyLimitResetEnum {
    fn default() -> Self {
        Self::UnknownValue(String::new())
    }
}

/// Typed params for `PATCH /keys/{hash}` (`updateKeys`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateKeysParams {
    pub hash: String,
    pub body: UpdateKeysBody,
}

/// JSON result for `updateKeys`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateKeysResult {
    #[serde(flatten)]
    pub body: UpdateKeysResultBody,
}

/// Generated object `UpdateKeysResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateKeysResultBody {
    pub data: UpdateKeysResultBodyData,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `UpdateKeysResultBodyData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateKeysResultBodyData {
    pub byok_usage: f64,
    pub byok_usage_daily: f64,
    pub byok_usage_monthly: f64,
    pub byok_usage_weekly: f64,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_user_id: Option<String>,
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub hash: String,
    pub include_byok_in_limit: bool,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_remaining: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reset: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub usage: f64,
    pub usage_daily: f64,
    pub usage_monthly: f64,
    pub usage_weekly: f64,
    pub workspace_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `PATCH /observability/destinations/{id}` (`updateObservabilityDestination`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateObservabilityDestinationParams {
    pub id: String,
    pub body: OrUpdateObservabilityDestinationRequest,
}

/// JSON result for `updateObservabilityDestination`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateObservabilityDestinationResult {
    #[serde(flatten)]
    pub body: OrUpdateObservabilityDestinationResponse,
}

/// Typed params for `PATCH /workspaces/{id}` (`updateWorkspace`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateWorkspaceParams {
    pub id: String,
    pub body: OrUpdateWorkspaceRequest,
}

/// JSON result for `updateWorkspace`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateWorkspaceResult {
    #[serde(flatten)]
    pub body: OrUpdateWorkspaceResponse,
}

/// Generated object `UploadFileBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UploadFileBody {
    pub file: Vec<u8>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /files` (`uploadFile`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UploadFileParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    pub body: UploadFileBody,
}

/// JSON result for `uploadFile`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UploadFileResult {
    #[serde(flatten)]
    pub body: OrFileMetadata,
}

/// Typed params for `PUT /workspaces/{id}/budgets/{interval}` (`upsertWorkspaceBudget`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpsertWorkspaceBudgetParams {
    pub id: String,
    pub interval: String,
    pub body: OrUpsertWorkspaceBudgetRequest,
}

/// JSON result for `upsertWorkspaceBudget`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpsertWorkspaceBudgetResult {
    #[serde(flatten)]
    pub body: OrUpsertWorkspaceBudgetResponse,
}

/// Generated object `UsageV0`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageV0 {
    pub input_tokens: i64,
    pub input_tokens_details: UsageV0InputTokensDetails,
    pub output_tokens: i64,
    pub output_tokens_details: UsageV0OutputTokensDetails,
    pub total_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_details: Option<UsageV0CostDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_byok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_tool_use_details: Option<OrServerToolUseDetails>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `UsageV0CostDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageV0CostDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_inference_cost: Option<f64>,
    pub upstream_inference_input_cost: f64,
    pub upstream_inference_output_cost: f64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `UsageV0InputTokensDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageV0InputTokensDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<i64>,
    pub cached_tokens: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `UsageV0OutputTokensDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageV0OutputTokensDetails {
    pub reasoning_tokens: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
