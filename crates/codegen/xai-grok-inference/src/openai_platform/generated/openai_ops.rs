//! Generated typed operations for openai platform baseline.
//! DO NOT EDIT BY HAND — regenerate via baselines/scripts/generate_platform_client.py

use super::super::error::{PlatformError, PlatformResult};
use super::super::transport::{CredentialKind, HttpRequestSpec, PlatformTransport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Request for `GET /assistants` (`listAssistants`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListAssistantsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /assistants` (`listAssistants`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListAssistantsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /assistants` (`createAssistant`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAssistantRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateAssistantBody,
}

/// Body for `createAssistant`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAssistantBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateAssistantBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /assistants` (`createAssistant`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAssistantResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /assistants/{assistant_id}` (`deleteAssistant`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteAssistantRequest {
    pub assistant_id: String,
}

/// Response for `DELETE /assistants/{assistant_id}` (`deleteAssistant`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteAssistantResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /assistants/{assistant_id}` (`getAssistant`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetAssistantRequest {
    pub assistant_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /assistants/{assistant_id}` (`getAssistant`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetAssistantResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /assistants/{assistant_id}` (`modifyAssistant`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyAssistantRequest {
    pub assistant_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyAssistantBody,
}

/// Body for `modifyAssistant`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyAssistantBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyAssistantBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /assistants/{assistant_id}` (`modifyAssistant`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyAssistantResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /audio/speech` (`createSpeech`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSpeechRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateSpeechBody,
}

/// Body for `createSpeech`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSpeechBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateSpeechBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /audio/speech` (`createSpeech`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSpeechResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /audio/transcriptions` (`createTranscription`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateTranscriptionRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateTranscriptionBody,
}

/// Body for `createTranscription`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateTranscriptionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateTranscriptionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /audio/transcriptions` (`createTranscription`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateTranscriptionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /audio/translations` (`createTranslation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateTranslationRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateTranslationBody,
}

/// Body for `createTranslation`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateTranslationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateTranslationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /audio/translations` (`createTranslation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateTranslationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /audio/voice_consents` (`listVoiceConsents`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVoiceConsentsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /audio/voice_consents` (`listVoiceConsents`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVoiceConsentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /audio/voice_consents` (`createVoiceConsent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVoiceConsentRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVoiceConsentBody,
}

/// Body for `createVoiceConsent`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVoiceConsentBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVoiceConsentBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /audio/voice_consents` (`createVoiceConsent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVoiceConsentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /audio/voice_consents/{consent_id}` (`deleteVoiceConsent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteVoiceConsentRequest {
    pub consent_id: String,
}

/// Response for `DELETE /audio/voice_consents/{consent_id}` (`deleteVoiceConsent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteVoiceConsentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /audio/voice_consents/{consent_id}` (`getVoiceConsent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVoiceConsentRequest {
    pub consent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /audio/voice_consents/{consent_id}` (`getVoiceConsent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVoiceConsentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /audio/voice_consents/{consent_id}` (`updateVoiceConsent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateVoiceConsentRequest {
    pub consent_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateVoiceConsentBody,
}

/// Body for `updateVoiceConsent`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateVoiceConsentBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateVoiceConsentBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /audio/voice_consents/{consent_id}` (`updateVoiceConsent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateVoiceConsentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /audio/voices` (`createVoice`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVoiceRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVoiceBody,
}

/// Body for `createVoice`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVoiceBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVoiceBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /audio/voices` (`createVoice`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVoiceResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /batches` (`listBatches`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListBatchesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /batches` (`listBatches`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListBatchesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /batches` (`createBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateBatchRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateBatchBody,
}

/// Body for `createBatch`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateBatchBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateBatchBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /batches` (`createBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateBatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /batches/{batch_id}` (`retrieveBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveBatchRequest {
    pub batch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /batches/{batch_id}` (`retrieveBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveBatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /batches/{batch_id}/cancel` (`cancelBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelBatchRequest {
    pub batch_id: String,
}

/// Response for `POST /batches/{batch_id}/cancel` (`cancelBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelBatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /chat/completions` (`listChatCompletions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListChatCompletionsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /chat/completions` (`listChatCompletions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListChatCompletionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /chat/completions` (`createChatCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateChatCompletionRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateChatCompletionBody,
}

/// Body for `createChatCompletion`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateChatCompletionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateChatCompletionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /chat/completions` (`createChatCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateChatCompletionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /chat/completions/{completion_id}` (`deleteChatCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteChatCompletionRequest {
    pub completion_id: String,
}

/// Response for `DELETE /chat/completions/{completion_id}` (`deleteChatCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteChatCompletionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /chat/completions/{completion_id}` (`getChatCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetChatCompletionRequest {
    pub completion_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /chat/completions/{completion_id}` (`getChatCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetChatCompletionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /chat/completions/{completion_id}` (`updateChatCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateChatCompletionRequest {
    pub completion_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateChatCompletionBody,
}

/// Body for `updateChatCompletion`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateChatCompletionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateChatCompletionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /chat/completions/{completion_id}` (`updateChatCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateChatCompletionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /chat/completions/{completion_id}/messages` (`getChatCompletionMessages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetChatCompletionMessagesRequest {
    pub completion_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /chat/completions/{completion_id}/messages` (`getChatCompletionMessages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetChatCompletionMessagesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /chatkit/sessions` (`CreateChatSessionMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateChatSessionMethodRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateChatSessionMethodBody,
}

/// Body for `CreateChatSessionMethod`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateChatSessionMethodBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateChatSessionMethodBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /chatkit/sessions` (`CreateChatSessionMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateChatSessionMethodResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /chatkit/sessions/{session_id}/cancel` (`CancelChatSessionMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelChatSessionMethodRequest {
    pub session_id: String,
}

/// Response for `POST /chatkit/sessions/{session_id}/cancel` (`CancelChatSessionMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelChatSessionMethodResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /chatkit/threads` (`ListThreadsMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListThreadsMethodRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /chatkit/threads` (`ListThreadsMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListThreadsMethodResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /chatkit/threads/{thread_id}` (`DeleteThreadMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteThreadMethodRequest {
    pub thread_id: String,
}

/// Response for `DELETE /chatkit/threads/{thread_id}` (`DeleteThreadMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteThreadMethodResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /chatkit/threads/{thread_id}` (`GetThreadMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetThreadMethodRequest {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /chatkit/threads/{thread_id}` (`GetThreadMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetThreadMethodResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /chatkit/threads/{thread_id}/items` (`ListThreadItemsMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListThreadItemsMethodRequest {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /chatkit/threads/{thread_id}/items` (`ListThreadItemsMethod`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListThreadItemsMethodResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /completions` (`createCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateCompletionRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateCompletionBody,
}

/// Body for `createCompletion`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateCompletionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateCompletionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /completions` (`createCompletion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateCompletionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /containers` (`ListContainers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListContainersRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /containers` (`ListContainers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListContainersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /containers` (`CreateContainer`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateContainerRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateContainerBody,
}

/// Body for `CreateContainer`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateContainerBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateContainerBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /containers` (`CreateContainer`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateContainerResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /containers/{container_id}` (`DeleteContainer`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteContainerRequest {
    pub container_id: String,
}

/// Response for `DELETE /containers/{container_id}` (`DeleteContainer`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteContainerResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /containers/{container_id}` (`RetrieveContainer`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveContainerRequest {
    pub container_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /containers/{container_id}` (`RetrieveContainer`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveContainerResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /containers/{container_id}/files` (`ListContainerFiles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListContainerFilesRequest {
    pub container_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /containers/{container_id}/files` (`ListContainerFiles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListContainerFilesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /containers/{container_id}/files` (`CreateContainerFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateContainerFileRequest {
    pub container_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateContainerFileBody,
}

/// Body for `CreateContainerFile`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateContainerFileBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateContainerFileBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /containers/{container_id}/files` (`CreateContainerFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateContainerFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /containers/{container_id}/files/{file_id}` (`DeleteContainerFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteContainerFileRequest {
    pub container_id: String,
    pub file_id: String,
}

/// Response for `DELETE /containers/{container_id}/files/{file_id}` (`DeleteContainerFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteContainerFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /containers/{container_id}/files/{file_id}` (`RetrieveContainerFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveContainerFileRequest {
    pub container_id: String,
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /containers/{container_id}/files/{file_id}` (`RetrieveContainerFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveContainerFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /containers/{container_id}/files/{file_id}/content` (`RetrieveContainerFileContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveContainerFileContentRequest {
    pub container_id: String,
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /containers/{container_id}/files/{file_id}/content` (`RetrieveContainerFileContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveContainerFileContentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /conversations` (`createConversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateConversationRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateConversationBody,
}

/// Body for `createConversation`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateConversationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateConversationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /conversations` (`createConversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateConversationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /conversations/{conversation_id}` (`deleteConversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteConversationRequest {
    pub conversation_id: String,
}

/// Response for `DELETE /conversations/{conversation_id}` (`deleteConversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteConversationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /conversations/{conversation_id}` (`getConversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetConversationRequest {
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /conversations/{conversation_id}` (`getConversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetConversationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /conversations/{conversation_id}` (`updateConversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateConversationRequest {
    pub conversation_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateConversationBody,
}

/// Body for `updateConversation`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateConversationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateConversationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /conversations/{conversation_id}` (`updateConversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateConversationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /conversations/{conversation_id}/items` (`listConversationItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListConversationItemsRequest {
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /conversations/{conversation_id}/items` (`listConversationItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListConversationItemsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /conversations/{conversation_id}/items` (`createConversationItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateConversationItemsRequest {
    pub conversation_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateConversationItemsBody,
}

/// Body for `createConversationItems`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateConversationItemsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateConversationItemsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /conversations/{conversation_id}/items` (`createConversationItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateConversationItemsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /conversations/{conversation_id}/items/{item_id}` (`deleteConversationItem`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteConversationItemRequest {
    pub conversation_id: String,
    pub item_id: String,
}

/// Response for `DELETE /conversations/{conversation_id}/items/{item_id}` (`deleteConversationItem`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteConversationItemResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /conversations/{conversation_id}/items/{item_id}` (`getConversationItem`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetConversationItemRequest {
    pub conversation_id: String,
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /conversations/{conversation_id}/items/{item_id}` (`getConversationItem`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetConversationItemResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /embeddings` (`createEmbedding`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEmbeddingRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateEmbeddingBody,
}

/// Body for `createEmbedding`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEmbeddingBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateEmbeddingBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /embeddings` (`createEmbedding`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEmbeddingResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /evals` (`listEvals`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListEvalsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /evals` (`listEvals`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListEvalsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /evals` (`createEval`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEvalRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateEvalBody,
}

/// Body for `createEval`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEvalBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateEvalBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /evals` (`createEval`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEvalResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /evals/{eval_id}` (`deleteEval`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteEvalRequest {
    pub eval_id: String,
}

/// Response for `DELETE /evals/{eval_id}` (`deleteEval`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteEvalResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /evals/{eval_id}` (`getEval`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRequest {
    pub eval_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /evals/{eval_id}` (`getEval`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /evals/{eval_id}` (`updateEval`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateEvalRequest {
    pub eval_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateEvalBody,
}

/// Body for `updateEval`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateEvalBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateEvalBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /evals/{eval_id}` (`updateEval`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateEvalResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /evals/{eval_id}/runs` (`getEvalRuns`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRunsRequest {
    pub eval_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /evals/{eval_id}/runs` (`getEvalRuns`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRunsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /evals/{eval_id}/runs` (`createEvalRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEvalRunRequest {
    pub eval_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateEvalRunBody,
}

/// Body for `createEvalRun`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEvalRunBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateEvalRunBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /evals/{eval_id}/runs` (`createEvalRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEvalRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /evals/{eval_id}/runs/{run_id}` (`deleteEvalRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteEvalRunRequest {
    pub eval_id: String,
    pub run_id: String,
}

/// Response for `DELETE /evals/{eval_id}/runs/{run_id}` (`deleteEvalRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteEvalRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /evals/{eval_id}/runs/{run_id}` (`getEvalRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRunRequest {
    pub eval_id: String,
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /evals/{eval_id}/runs/{run_id}` (`getEvalRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /evals/{eval_id}/runs/{run_id}` (`cancelEvalRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelEvalRunRequest {
    pub eval_id: String,
    pub run_id: String,
}

/// Response for `POST /evals/{eval_id}/runs/{run_id}` (`cancelEvalRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelEvalRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /evals/{eval_id}/runs/{run_id}/output_items` (`getEvalRunOutputItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRunOutputItemsRequest {
    pub eval_id: String,
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /evals/{eval_id}/runs/{run_id}/output_items` (`getEvalRunOutputItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRunOutputItemsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /evals/{eval_id}/runs/{run_id}/output_items/{output_item_id}` (`getEvalRunOutputItem`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRunOutputItemRequest {
    pub eval_id: String,
    pub run_id: String,
    pub output_item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /evals/{eval_id}/runs/{run_id}/output_items/{output_item_id}` (`getEvalRunOutputItem`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetEvalRunOutputItemResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /files` (`listFiles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFilesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /files` (`listFiles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFilesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /files` (`createFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFileRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateFileBody,
}

/// Body for `createFile`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFileBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateFileBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /files` (`createFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /files/{file_id}` (`deleteFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteFileRequest {
    pub file_id: String,
}

/// Response for `DELETE /files/{file_id}` (`deleteFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /files/{file_id}` (`retrieveFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveFileRequest {
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /files/{file_id}` (`retrieveFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /files/{file_id}/content` (`downloadFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DownloadFileRequest {
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /files/{file_id}/content` (`downloadFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DownloadFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /fine_tuning/alpha/graders/run` (`runGrader`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunGraderRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: RunGraderBody,
}

/// Body for `runGrader`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunGraderBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl RunGraderBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /fine_tuning/alpha/graders/run` (`runGrader`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunGraderResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /fine_tuning/alpha/graders/validate` (`validateGrader`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ValidateGraderRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ValidateGraderBody,
}

/// Body for `validateGrader`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ValidateGraderBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ValidateGraderBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /fine_tuning/alpha/graders/validate` (`validateGrader`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ValidateGraderResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions` (`listFineTuningCheckpointPermissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFineTuningCheckpointPermissionsRequest {
    pub fine_tuned_model_checkpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions` (`listFineTuningCheckpointPermissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFineTuningCheckpointPermissionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions` (`createFineTuningCheckpointPermission`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFineTuningCheckpointPermissionRequest {
    pub fine_tuned_model_checkpoint: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateFineTuningCheckpointPermissionBody,
}

/// Body for `createFineTuningCheckpointPermission`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFineTuningCheckpointPermissionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateFineTuningCheckpointPermissionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions` (`createFineTuningCheckpointPermission`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFineTuningCheckpointPermissionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions/{permission_id}` (`deleteFineTuningCheckpointPermission`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteFineTuningCheckpointPermissionRequest {
    pub fine_tuned_model_checkpoint: String,
    pub permission_id: String,
}

/// Response for `DELETE /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions/{permission_id}` (`deleteFineTuningCheckpointPermission`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteFineTuningCheckpointPermissionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /fine_tuning/jobs` (`listPaginatedFineTuningJobs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListPaginatedFineTuningJobsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /fine_tuning/jobs` (`listPaginatedFineTuningJobs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListPaginatedFineTuningJobsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /fine_tuning/jobs` (`createFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFineTuningJobRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateFineTuningJobBody,
}

/// Body for `createFineTuningJob`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFineTuningJobBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateFineTuningJobBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /fine_tuning/jobs` (`createFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateFineTuningJobResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /fine_tuning/jobs/{fine_tuning_job_id}` (`retrieveFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveFineTuningJobRequest {
    pub fine_tuning_job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /fine_tuning/jobs/{fine_tuning_job_id}` (`retrieveFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveFineTuningJobResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /fine_tuning/jobs/{fine_tuning_job_id}/cancel` (`cancelFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelFineTuningJobRequest {
    pub fine_tuning_job_id: String,
}

/// Response for `POST /fine_tuning/jobs/{fine_tuning_job_id}/cancel` (`cancelFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelFineTuningJobResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /fine_tuning/jobs/{fine_tuning_job_id}/checkpoints` (`listFineTuningJobCheckpoints`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFineTuningJobCheckpointsRequest {
    pub fine_tuning_job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /fine_tuning/jobs/{fine_tuning_job_id}/checkpoints` (`listFineTuningJobCheckpoints`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFineTuningJobCheckpointsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /fine_tuning/jobs/{fine_tuning_job_id}/events` (`listFineTuningEvents`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFineTuningEventsRequest {
    pub fine_tuning_job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /fine_tuning/jobs/{fine_tuning_job_id}/events` (`listFineTuningEvents`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFineTuningEventsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /fine_tuning/jobs/{fine_tuning_job_id}/pause` (`pauseFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PauseFineTuningJobRequest {
    pub fine_tuning_job_id: String,
}

/// Response for `POST /fine_tuning/jobs/{fine_tuning_job_id}/pause` (`pauseFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PauseFineTuningJobResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /fine_tuning/jobs/{fine_tuning_job_id}/resume` (`resumeFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResumeFineTuningJobRequest {
    pub fine_tuning_job_id: String,
}

/// Response for `POST /fine_tuning/jobs/{fine_tuning_job_id}/resume` (`resumeFineTuningJob`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResumeFineTuningJobResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /images/edits` (`createImageEdit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageEditRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateImageEditBody,
}

/// Body for `createImageEdit`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageEditBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateImageEditBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /images/edits` (`createImageEdit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageEditResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /images/generations` (`createImage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateImageBody,
}

/// Body for `createImage`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateImageBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /images/generations` (`createImage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /images/variations` (`createImageVariation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageVariationRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateImageVariationBody,
}

/// Body for `createImageVariation`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageVariationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateImageVariationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /images/variations` (`createImageVariation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImageVariationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /models` (`listModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListModelsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /models` (`listModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListModelsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /models/{model}` (`deleteModel`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteModelRequest {
    pub model: String,
}

/// Response for `DELETE /models/{model}` (`deleteModel`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteModelResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /models/{model}` (`retrieveModel`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveModelRequest {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /models/{model}` (`retrieveModel`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveModelResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /moderations` (`createModeration`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateModerationRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateModerationBody,
}

/// Body for `createModeration`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateModerationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateModerationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /moderations` (`createModeration`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateModerationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /projects/{project_id}/groups/{group_id}/roles` (`list-project-group-role-assignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectGroupRoleAssignmentsRequest {
    pub project_id: String,
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /projects/{project_id}/groups/{group_id}/roles` (`list-project-group-role-assignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectGroupRoleAssignmentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /projects/{project_id}/groups/{group_id}/roles` (`assign-project-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignProjectGroupRoleRequest {
    pub project_id: String,
    pub group_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AssignProjectGroupRoleBody,
}

/// Body for `assign-project-group-role`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignProjectGroupRoleBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AssignProjectGroupRoleBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /projects/{project_id}/groups/{group_id}/roles` (`assign-project-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignProjectGroupRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /projects/{project_id}/groups/{group_id}/roles/{role_id}` (`unassign-project-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnassignProjectGroupRoleRequest {
    pub project_id: String,
    pub group_id: String,
    pub role_id: String,
}

/// Response for `DELETE /projects/{project_id}/groups/{group_id}/roles/{role_id}` (`unassign-project-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnassignProjectGroupRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /projects/{project_id}/groups/{group_id}/roles/{role_id}` (`retrieve-project-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectGroupRoleRequest {
    pub project_id: String,
    pub group_id: String,
    pub role_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /projects/{project_id}/groups/{group_id}/roles/{role_id}` (`retrieve-project-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectGroupRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /projects/{project_id}/roles` (`list-project-roles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectRolesRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /projects/{project_id}/roles` (`list-project-roles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectRolesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /projects/{project_id}/roles` (`create-project-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectRoleRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateProjectRoleBody,
}

/// Body for `create-project-role`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectRoleBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateProjectRoleBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /projects/{project_id}/roles` (`create-project-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /projects/{project_id}/roles/{role_id}` (`delete-project-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectRoleRequest {
    pub project_id: String,
    pub role_id: String,
}

/// Response for `DELETE /projects/{project_id}/roles/{role_id}` (`delete-project-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /projects/{project_id}/roles/{role_id}` (`retrieve-project-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectRoleRequest {
    pub project_id: String,
    pub role_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /projects/{project_id}/roles/{role_id}` (`retrieve-project-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /projects/{project_id}/roles/{role_id}` (`update-project-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectRoleRequest {
    pub project_id: String,
    pub role_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateProjectRoleBody,
}

/// Body for `update-project-role`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectRoleBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateProjectRoleBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /projects/{project_id}/roles/{role_id}` (`update-project-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /projects/{project_id}/users/{user_id}/roles` (`list-project-user-role-assignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectUserRoleAssignmentsRequest {
    pub project_id: String,
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /projects/{project_id}/users/{user_id}/roles` (`list-project-user-role-assignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectUserRoleAssignmentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /projects/{project_id}/users/{user_id}/roles` (`assign-project-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignProjectUserRoleRequest {
    pub project_id: String,
    pub user_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AssignProjectUserRoleBody,
}

/// Body for `assign-project-user-role`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignProjectUserRoleBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AssignProjectUserRoleBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /projects/{project_id}/users/{user_id}/roles` (`assign-project-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignProjectUserRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /projects/{project_id}/users/{user_id}/roles/{role_id}` (`unassign-project-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnassignProjectUserRoleRequest {
    pub project_id: String,
    pub user_id: String,
    pub role_id: String,
}

/// Response for `DELETE /projects/{project_id}/users/{user_id}/roles/{role_id}` (`unassign-project-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnassignProjectUserRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /projects/{project_id}/users/{user_id}/roles/{role_id}` (`retrieve-project-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectUserRoleRequest {
    pub project_id: String,
    pub user_id: String,
    pub role_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /projects/{project_id}/users/{user_id}/roles/{role_id}` (`retrieve-project-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectUserRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/calls` (`create-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeCallRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateRealtimeCallBody,
}

/// Body for `create-realtime-call`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeCallBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateRealtimeCallBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /realtime/calls` (`create-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeCallResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/calls/{call_id}/accept` (`accept-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AcceptRealtimeCallRequest {
    pub call_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AcceptRealtimeCallBody,
}

/// Body for `accept-realtime-call`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AcceptRealtimeCallBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AcceptRealtimeCallBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /realtime/calls/{call_id}/accept` (`accept-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AcceptRealtimeCallResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/calls/{call_id}/hangup` (`hangup-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HangupRealtimeCallRequest {
    pub call_id: String,
}

/// Response for `POST /realtime/calls/{call_id}/hangup` (`hangup-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HangupRealtimeCallResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/calls/{call_id}/refer` (`refer-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReferRealtimeCallRequest {
    pub call_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ReferRealtimeCallBody,
}

/// Body for `refer-realtime-call`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReferRealtimeCallBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ReferRealtimeCallBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /realtime/calls/{call_id}/refer` (`refer-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReferRealtimeCallResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/calls/{call_id}/reject` (`reject-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RejectRealtimeCallRequest {
    pub call_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: RejectRealtimeCallBody,
}

/// Body for `reject-realtime-call`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RejectRealtimeCallBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl RejectRealtimeCallBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /realtime/calls/{call_id}/reject` (`reject-realtime-call`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RejectRealtimeCallResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/client_secrets` (`create-realtime-client-secret`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeClientSecretRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateRealtimeClientSecretBody,
}

/// Body for `create-realtime-client-secret`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeClientSecretBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateRealtimeClientSecretBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /realtime/client_secrets` (`create-realtime-client-secret`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeClientSecretResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/sessions` (`create-realtime-session`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeSessionRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateRealtimeSessionBody,
}

/// Body for `create-realtime-session`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeSessionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateRealtimeSessionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /realtime/sessions` (`create-realtime-session`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeSessionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/transcription_sessions` (`create-realtime-transcription-session`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeTranscriptionSessionRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateRealtimeTranscriptionSessionBody,
}

/// Body for `create-realtime-transcription-session`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeTranscriptionSessionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateRealtimeTranscriptionSessionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /realtime/transcription_sessions` (`create-realtime-transcription-session`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeTranscriptionSessionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /realtime/translations/client_secrets` (`create-realtime-translation-client-secret`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeTranslationClientSecretRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateRealtimeTranslationClientSecretBody,
}

/// Body for `create-realtime-translation-client-secret`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeTranslationClientSecretBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateRealtimeTranslationClientSecretBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /realtime/translations/client_secrets` (`create-realtime-translation-client-secret`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRealtimeTranslationClientSecretResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses` (`createResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateResponseRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateResponseBody,
}

/// Body for `createResponse`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateResponseBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateResponseBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /responses` (`createResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateResponseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses/compact` (`Compactconversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompactconversationRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CompactconversationBody,
}

/// Body for `Compactconversation`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompactconversationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CompactconversationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /responses/compact` (`Compactconversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompactconversationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses/compact?beta=true` (`beta_Compactconversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaCompactconversationRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BetaCompactconversationBody,
}

/// Body for `beta_Compactconversation`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaCompactconversationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BetaCompactconversationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /responses/compact?beta=true` (`beta_Compactconversation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaCompactconversationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses/input_tokens` (`Getinputtokencounts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetinputtokencountsRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: GetinputtokencountsBody,
}

/// Body for `Getinputtokencounts`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetinputtokencountsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl GetinputtokencountsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /responses/input_tokens` (`Getinputtokencounts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetinputtokencountsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses/input_tokens?beta=true` (`beta_Getinputtokencounts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaGetinputtokencountsRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BetaGetinputtokencountsBody,
}

/// Body for `beta_Getinputtokencounts`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaGetinputtokencountsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BetaGetinputtokencountsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /responses/input_tokens?beta=true` (`beta_Getinputtokencounts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaGetinputtokencountsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /responses/{response_id}` (`deleteResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteResponseRequest {
    pub response_id: String,
}

/// Response for `DELETE /responses/{response_id}` (`deleteResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteResponseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /responses/{response_id}` (`getResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetResponseRequest {
    pub response_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /responses/{response_id}` (`getResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetResponseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses/{response_id}/cancel` (`cancelResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelResponseRequest {
    pub response_id: String,
}

/// Response for `POST /responses/{response_id}/cancel` (`cancelResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelResponseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses/{response_id}/cancel?beta=true` (`beta_cancelResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaCancelResponseRequest {
    pub response_id: String,
}

/// Response for `POST /responses/{response_id}/cancel?beta=true` (`beta_cancelResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaCancelResponseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /responses/{response_id}/input_items` (`listInputItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListInputItemsRequest {
    pub response_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /responses/{response_id}/input_items` (`listInputItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListInputItemsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /responses/{response_id}/input_items?beta=true` (`beta_listInputItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaListInputItemsRequest {
    pub response_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /responses/{response_id}/input_items?beta=true` (`beta_listInputItems`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaListInputItemsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /responses/{response_id}?beta=true` (`beta_deleteResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaDeleteResponseRequest {
    pub response_id: String,
}

/// Response for `DELETE /responses/{response_id}?beta=true` (`beta_deleteResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaDeleteResponseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /responses/{response_id}?beta=true` (`beta_getResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaGetResponseRequest {
    pub response_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /responses/{response_id}?beta=true` (`beta_getResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaGetResponseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses?beta=true` (`beta_createResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaCreateResponseRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BetaCreateResponseBody,
}

/// Body for `beta_createResponse`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaCreateResponseBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BetaCreateResponseBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /responses?beta=true` (`beta_createResponse`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BetaCreateResponseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /skills` (`ListSkills`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListSkillsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /skills` (`ListSkills`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListSkillsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /skills` (`CreateSkill`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSkillRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateSkillBody,
}

/// Body for `CreateSkill`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSkillBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateSkillBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /skills` (`CreateSkill`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSkillResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /skills/{skill_id}` (`DeleteSkill`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteSkillRequest {
    pub skill_id: String,
}

/// Response for `DELETE /skills/{skill_id}` (`DeleteSkill`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteSkillResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /skills/{skill_id}` (`GetSkill`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetSkillRequest {
    pub skill_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /skills/{skill_id}` (`GetSkill`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetSkillResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /skills/{skill_id}` (`UpdateSkillDefaultVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateSkillDefaultVersionRequest {
    pub skill_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateSkillDefaultVersionBody,
}

/// Body for `UpdateSkillDefaultVersion`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateSkillDefaultVersionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateSkillDefaultVersionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /skills/{skill_id}` (`UpdateSkillDefaultVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateSkillDefaultVersionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /skills/{skill_id}/content` (`GetSkillContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetSkillContentRequest {
    pub skill_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /skills/{skill_id}/content` (`GetSkillContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetSkillContentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /skills/{skill_id}/versions` (`ListSkillVersions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListSkillVersionsRequest {
    pub skill_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /skills/{skill_id}/versions` (`ListSkillVersions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListSkillVersionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /skills/{skill_id}/versions` (`CreateSkillVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSkillVersionRequest {
    pub skill_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateSkillVersionBody,
}

/// Body for `CreateSkillVersion`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSkillVersionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateSkillVersionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /skills/{skill_id}/versions` (`CreateSkillVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateSkillVersionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /skills/{skill_id}/versions/{version}` (`DeleteSkillVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteSkillVersionRequest {
    pub skill_id: String,
    pub version: String,
}

/// Response for `DELETE /skills/{skill_id}/versions/{version}` (`DeleteSkillVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteSkillVersionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /skills/{skill_id}/versions/{version}` (`GetSkillVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetSkillVersionRequest {
    pub skill_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /skills/{skill_id}/versions/{version}` (`GetSkillVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetSkillVersionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /skills/{skill_id}/versions/{version}/content` (`GetSkillVersionContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetSkillVersionContentRequest {
    pub skill_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /skills/{skill_id}/versions/{version}/content` (`GetSkillVersionContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetSkillVersionContentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads` (`createThread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateThreadRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateThreadBody,
}

/// Body for `createThread`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateThreadBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateThreadBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /threads` (`createThread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateThreadResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads/runs` (`createThreadAndRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateThreadAndRunRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateThreadAndRunBody,
}

/// Body for `createThreadAndRun`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateThreadAndRunBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateThreadAndRunBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /threads/runs` (`createThreadAndRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateThreadAndRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /threads/{thread_id}` (`deleteThread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteThreadRequest {
    pub thread_id: String,
}

/// Response for `DELETE /threads/{thread_id}` (`deleteThread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteThreadResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /threads/{thread_id}` (`getThread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetThreadRequest {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /threads/{thread_id}` (`getThread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetThreadResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads/{thread_id}` (`modifyThread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyThreadRequest {
    pub thread_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyThreadBody,
}

/// Body for `modifyThread`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyThreadBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyThreadBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /threads/{thread_id}` (`modifyThread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyThreadResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /threads/{thread_id}/messages` (`listMessages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListMessagesRequest {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /threads/{thread_id}/messages` (`listMessages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListMessagesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads/{thread_id}/messages` (`createMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateMessageRequest {
    pub thread_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateMessageBody,
}

/// Body for `createMessage`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateMessageBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateMessageBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /threads/{thread_id}/messages` (`createMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateMessageResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /threads/{thread_id}/messages/{message_id}` (`deleteMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteMessageRequest {
    pub thread_id: String,
    pub message_id: String,
}

/// Response for `DELETE /threads/{thread_id}/messages/{message_id}` (`deleteMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteMessageResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /threads/{thread_id}/messages/{message_id}` (`getMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetMessageRequest {
    pub thread_id: String,
    pub message_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /threads/{thread_id}/messages/{message_id}` (`getMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetMessageResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads/{thread_id}/messages/{message_id}` (`modifyMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyMessageRequest {
    pub thread_id: String,
    pub message_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyMessageBody,
}

/// Body for `modifyMessage`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyMessageBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyMessageBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /threads/{thread_id}/messages/{message_id}` (`modifyMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyMessageResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /threads/{thread_id}/runs` (`listRuns`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListRunsRequest {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /threads/{thread_id}/runs` (`listRuns`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListRunsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads/{thread_id}/runs` (`createRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRunRequest {
    pub thread_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateRunBody,
}

/// Body for `createRun`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRunBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateRunBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /threads/{thread_id}/runs` (`createRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /threads/{thread_id}/runs/{run_id}` (`getRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetRunRequest {
    pub thread_id: String,
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /threads/{thread_id}/runs/{run_id}` (`getRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads/{thread_id}/runs/{run_id}` (`modifyRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyRunRequest {
    pub thread_id: String,
    pub run_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyRunBody,
}

/// Body for `modifyRun`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyRunBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyRunBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /threads/{thread_id}/runs/{run_id}` (`modifyRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads/{thread_id}/runs/{run_id}/cancel` (`cancelRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelRunRequest {
    pub thread_id: String,
    pub run_id: String,
}

/// Response for `POST /threads/{thread_id}/runs/{run_id}/cancel` (`cancelRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /threads/{thread_id}/runs/{run_id}/steps` (`listRunSteps`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListRunStepsRequest {
    pub thread_id: String,
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /threads/{thread_id}/runs/{run_id}/steps` (`listRunSteps`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListRunStepsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /threads/{thread_id}/runs/{run_id}/steps/{step_id}` (`getRunStep`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetRunStepRequest {
    pub thread_id: String,
    pub run_id: String,
    pub step_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /threads/{thread_id}/runs/{run_id}/steps/{step_id}` (`getRunStep`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetRunStepResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /threads/{thread_id}/runs/{run_id}/submit_tool_outputs` (`submitToolOuputsToRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SubmitToolOuputsToRunRequest {
    pub thread_id: String,
    pub run_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: SubmitToolOuputsToRunBody,
}

/// Body for `submitToolOuputsToRun`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SubmitToolOuputsToRunBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl SubmitToolOuputsToRunBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /threads/{thread_id}/runs/{run_id}/submit_tool_outputs` (`submitToolOuputsToRun`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SubmitToolOuputsToRunResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /uploads` (`createUpload`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateUploadRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateUploadBody,
}

/// Body for `createUpload`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateUploadBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateUploadBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /uploads` (`createUpload`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateUploadResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /uploads/{upload_id}/cancel` (`cancelUpload`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelUploadRequest {
    pub upload_id: String,
}

/// Response for `POST /uploads/{upload_id}/cancel` (`cancelUpload`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelUploadResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /uploads/{upload_id}/complete` (`completeUpload`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompleteUploadRequest {
    pub upload_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CompleteUploadBody,
}

/// Body for `completeUpload`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompleteUploadBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CompleteUploadBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /uploads/{upload_id}/complete` (`completeUpload`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompleteUploadResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /uploads/{upload_id}/parts` (`addUploadPart`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddUploadPartRequest {
    pub upload_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AddUploadPartBody,
}

/// Body for `addUploadPart`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddUploadPartBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AddUploadPartBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /uploads/{upload_id}/parts` (`addUploadPart`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddUploadPartResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /vector_stores` (`listVectorStores`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVectorStoresRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /vector_stores` (`listVectorStores`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVectorStoresResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /vector_stores` (`createVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVectorStoreBody,
}

/// Body for `createVectorStore`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVectorStoreBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /vector_stores` (`createVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /vector_stores/{vector_store_id}` (`deleteVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteVectorStoreRequest {
    pub vector_store_id: String,
}

/// Response for `DELETE /vector_stores/{vector_store_id}` (`deleteVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteVectorStoreResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /vector_stores/{vector_store_id}` (`getVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVectorStoreRequest {
    pub vector_store_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /vector_stores/{vector_store_id}` (`getVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVectorStoreResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /vector_stores/{vector_store_id}` (`modifyVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyVectorStoreRequest {
    pub vector_store_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyVectorStoreBody,
}

/// Body for `modifyVectorStore`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyVectorStoreBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyVectorStoreBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /vector_stores/{vector_store_id}` (`modifyVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyVectorStoreResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /vector_stores/{vector_store_id}/file_batches` (`createVectorStoreFileBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreFileBatchRequest {
    pub vector_store_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVectorStoreFileBatchBody,
}

/// Body for `createVectorStoreFileBatch`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreFileBatchBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVectorStoreFileBatchBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /vector_stores/{vector_store_id}/file_batches` (`createVectorStoreFileBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreFileBatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /vector_stores/{vector_store_id}/file_batches/{batch_id}` (`getVectorStoreFileBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVectorStoreFileBatchRequest {
    pub vector_store_id: String,
    pub batch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /vector_stores/{vector_store_id}/file_batches/{batch_id}` (`getVectorStoreFileBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVectorStoreFileBatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /vector_stores/{vector_store_id}/file_batches/{batch_id}/cancel` (`cancelVectorStoreFileBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelVectorStoreFileBatchRequest {
    pub vector_store_id: String,
    pub batch_id: String,
}

/// Response for `POST /vector_stores/{vector_store_id}/file_batches/{batch_id}/cancel` (`cancelVectorStoreFileBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CancelVectorStoreFileBatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /vector_stores/{vector_store_id}/file_batches/{batch_id}/files` (`listFilesInVectorStoreBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFilesInVectorStoreBatchRequest {
    pub vector_store_id: String,
    pub batch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /vector_stores/{vector_store_id}/file_batches/{batch_id}/files` (`listFilesInVectorStoreBatch`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListFilesInVectorStoreBatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /vector_stores/{vector_store_id}/files` (`listVectorStoreFiles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVectorStoreFilesRequest {
    pub vector_store_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /vector_stores/{vector_store_id}/files` (`listVectorStoreFiles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVectorStoreFilesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /vector_stores/{vector_store_id}/files` (`createVectorStoreFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreFileRequest {
    pub vector_store_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVectorStoreFileBody,
}

/// Body for `createVectorStoreFile`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreFileBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVectorStoreFileBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /vector_stores/{vector_store_id}/files` (`createVectorStoreFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVectorStoreFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /vector_stores/{vector_store_id}/files/{file_id}` (`deleteVectorStoreFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteVectorStoreFileRequest {
    pub vector_store_id: String,
    pub file_id: String,
}

/// Response for `DELETE /vector_stores/{vector_store_id}/files/{file_id}` (`deleteVectorStoreFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteVectorStoreFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /vector_stores/{vector_store_id}/files/{file_id}` (`getVectorStoreFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVectorStoreFileRequest {
    pub vector_store_id: String,
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /vector_stores/{vector_store_id}/files/{file_id}` (`getVectorStoreFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVectorStoreFileResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /vector_stores/{vector_store_id}/files/{file_id}` (`updateVectorStoreFileAttributes`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateVectorStoreFileAttributesRequest {
    pub vector_store_id: String,
    pub file_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateVectorStoreFileAttributesBody,
}

/// Body for `updateVectorStoreFileAttributes`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateVectorStoreFileAttributesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateVectorStoreFileAttributesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /vector_stores/{vector_store_id}/files/{file_id}` (`updateVectorStoreFileAttributes`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateVectorStoreFileAttributesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /vector_stores/{vector_store_id}/files/{file_id}/content` (`retrieveVectorStoreFileContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveVectorStoreFileContentRequest {
    pub vector_store_id: String,
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /vector_stores/{vector_store_id}/files/{file_id}/content` (`retrieveVectorStoreFileContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveVectorStoreFileContentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /vector_stores/{vector_store_id}/search` (`searchVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SearchVectorStoreRequest {
    pub vector_store_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: SearchVectorStoreBody,
}

/// Body for `searchVectorStore`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SearchVectorStoreBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl SearchVectorStoreBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /vector_stores/{vector_store_id}/search` (`searchVectorStore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SearchVectorStoreResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /videos` (`ListVideos`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVideosRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /videos` (`ListVideos`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVideosResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /videos` (`createVideo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVideoBody,
}

/// Body for `createVideo`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVideoBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /videos` (`createVideo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /videos/characters` (`CreateVideoCharacter`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoCharacterRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVideoCharacterBody,
}

/// Body for `CreateVideoCharacter`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoCharacterBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVideoCharacterBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /videos/characters` (`CreateVideoCharacter`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoCharacterResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /videos/characters/{character_id}` (`GetVideoCharacter`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVideoCharacterRequest {
    pub character_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /videos/characters/{character_id}` (`GetVideoCharacter`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVideoCharacterResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /videos/edits` (`CreateVideoEdit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoEditRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVideoEditBody,
}

/// Body for `CreateVideoEdit`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoEditBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVideoEditBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /videos/edits` (`CreateVideoEdit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoEditResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /videos/extensions` (`CreateVideoExtend`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoExtendRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVideoExtendBody,
}

/// Body for `CreateVideoExtend`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoExtendBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVideoExtendBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /videos/extensions` (`CreateVideoExtend`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoExtendResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /videos/{video_id}` (`DeleteVideo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteVideoRequest {
    pub video_id: String,
}

/// Response for `DELETE /videos/{video_id}` (`DeleteVideo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteVideoResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /videos/{video_id}` (`GetVideo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVideoRequest {
    pub video_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /videos/{video_id}` (`GetVideo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVideoResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /videos/{video_id}/content` (`RetrieveVideoContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveVideoContentRequest {
    pub video_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /videos/{video_id}/content` (`RetrieveVideoContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveVideoContentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /videos/{video_id}/remix` (`CreateVideoRemix`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoRemixRequest {
    pub video_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVideoRemixBody,
}

/// Body for `CreateVideoRemix`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoRemixBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVideoRemixBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /videos/{video_id}/remix` (`CreateVideoRemix`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideoRemixResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl crate::openai_platform::client::OpenAiClient {
    /// `GET /assistants` — `listAssistants`.
    pub async fn list_assistants(
        &self,
        request: ListAssistantsRequest,
    ) -> PlatformResult<ListAssistantsResponse> {
        let mut path = String::from("/assistants");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listAssistants",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /assistants` — `createAssistant`.
    pub async fn create_assistant(
        &self,
        request: CreateAssistantRequest,
    ) -> PlatformResult<CreateAssistantResponse> {
        let mut path = String::from("/assistants");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createAssistant",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /assistants/{assistant_id}` — `deleteAssistant`.
    pub async fn delete_assistant(
        &self,
        request: DeleteAssistantRequest,
    ) -> PlatformResult<DeleteAssistantResponse> {
        let mut path = String::from("/assistants/{assistant_id}");
        path = path.replace("{assistant_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.assistant_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteAssistant",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /assistants/{assistant_id}` — `getAssistant`.
    pub async fn get_assistant(
        &self,
        request: GetAssistantRequest,
    ) -> PlatformResult<GetAssistantResponse> {
        let mut path = String::from("/assistants/{assistant_id}");
        path = path.replace("{assistant_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.assistant_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getAssistant",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /assistants/{assistant_id}` — `modifyAssistant`.
    pub async fn modify_assistant(
        &self,
        request: ModifyAssistantRequest,
    ) -> PlatformResult<ModifyAssistantResponse> {
        let mut path = String::from("/assistants/{assistant_id}");
        path = path.replace("{assistant_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.assistant_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modifyAssistant",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/speech` — `createSpeech`.
    pub async fn create_speech(
        &self,
        request: CreateSpeechRequest,
    ) -> PlatformResult<CreateSpeechResponse> {
        let mut path = String::from("/audio/speech");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: true,
            multipart: false,
            operation_id: "createSpeech",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/transcriptions` — `createTranscription`.
    pub async fn create_transcription(
        &self,
        request: CreateTranscriptionRequest,
    ) -> PlatformResult<CreateTranscriptionResponse> {
        let mut path = String::from("/audio/transcriptions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: true,
            operation_id: "createTranscription",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/translations` — `createTranslation`.
    pub async fn create_translation(
        &self,
        request: CreateTranslationRequest,
    ) -> PlatformResult<CreateTranslationResponse> {
        let mut path = String::from("/audio/translations");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "createTranslation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /audio/voice_consents` — `listVoiceConsents`.
    pub async fn list_voice_consents(
        &self,
        request: ListVoiceConsentsRequest,
    ) -> PlatformResult<ListVoiceConsentsResponse> {
        let mut path = String::from("/audio/voice_consents");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listVoiceConsents",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/voice_consents` — `createVoiceConsent`.
    pub async fn create_voice_consent(
        &self,
        request: CreateVoiceConsentRequest,
    ) -> PlatformResult<CreateVoiceConsentResponse> {
        let mut path = String::from("/audio/voice_consents");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "createVoiceConsent",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /audio/voice_consents/{consent_id}` — `deleteVoiceConsent`.
    pub async fn delete_voice_consent(
        &self,
        request: DeleteVoiceConsentRequest,
    ) -> PlatformResult<DeleteVoiceConsentResponse> {
        let mut path = String::from("/audio/voice_consents/{consent_id}");
        path = path.replace("{consent_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.consent_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteVoiceConsent",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /audio/voice_consents/{consent_id}` — `getVoiceConsent`.
    pub async fn get_voice_consent(
        &self,
        request: GetVoiceConsentRequest,
    ) -> PlatformResult<GetVoiceConsentResponse> {
        let mut path = String::from("/audio/voice_consents/{consent_id}");
        path = path.replace("{consent_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.consent_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getVoiceConsent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/voice_consents/{consent_id}` — `updateVoiceConsent`.
    pub async fn update_voice_consent(
        &self,
        request: UpdateVoiceConsentRequest,
    ) -> PlatformResult<UpdateVoiceConsentResponse> {
        let mut path = String::from("/audio/voice_consents/{consent_id}");
        path = path.replace("{consent_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.consent_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateVoiceConsent",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/voices` — `createVoice`.
    pub async fn create_voice(
        &self,
        request: CreateVoiceRequest,
    ) -> PlatformResult<CreateVoiceResponse> {
        let mut path = String::from("/audio/voices");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "createVoice",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /batches` — `listBatches`.
    pub async fn list_batches(
        &self,
        request: ListBatchesRequest,
    ) -> PlatformResult<ListBatchesResponse> {
        let mut path = String::from("/batches");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listBatches",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /batches` — `createBatch`.
    pub async fn create_batch(
        &self,
        request: CreateBatchRequest,
    ) -> PlatformResult<CreateBatchResponse> {
        let mut path = String::from("/batches");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createBatch",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /batches/{batch_id}` — `retrieveBatch`.
    pub async fn retrieve_batch(
        &self,
        request: RetrieveBatchRequest,
    ) -> PlatformResult<RetrieveBatchResponse> {
        let mut path = String::from("/batches/{batch_id}");
        path = path.replace("{batch_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieveBatch",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /batches/{batch_id}/cancel` — `cancelBatch`.
    pub async fn cancel_batch(
        &self,
        request: CancelBatchRequest,
    ) -> PlatformResult<CancelBatchResponse> {
        let mut path = String::from("/batches/{batch_id}/cancel");
        path = path.replace("{batch_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "cancelBatch",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chat/completions` — `listChatCompletions`.
    pub async fn list_chat_completions(
        &self,
        request: ListChatCompletionsRequest,
    ) -> PlatformResult<ListChatCompletionsResponse> {
        let mut path = String::from("/chat/completions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listChatCompletions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chat/completions` — `createChatCompletion`.
    pub async fn create_chat_completion(
        &self,
        request: CreateChatCompletionRequest,
    ) -> PlatformResult<CreateChatCompletionResponse> {
        let mut path = String::from("/chat/completions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createChatCompletion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /chat/completions/{completion_id}` — `deleteChatCompletion`.
    pub async fn delete_chat_completion(
        &self,
        request: DeleteChatCompletionRequest,
    ) -> PlatformResult<DeleteChatCompletionResponse> {
        let mut path = String::from("/chat/completions/{completion_id}");
        path = path.replace("{completion_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.completion_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteChatCompletion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chat/completions/{completion_id}` — `getChatCompletion`.
    pub async fn get_chat_completion(
        &self,
        request: GetChatCompletionRequest,
    ) -> PlatformResult<GetChatCompletionResponse> {
        let mut path = String::from("/chat/completions/{completion_id}");
        path = path.replace("{completion_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.completion_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getChatCompletion",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chat/completions/{completion_id}` — `updateChatCompletion`.
    pub async fn update_chat_completion(
        &self,
        request: UpdateChatCompletionRequest,
    ) -> PlatformResult<UpdateChatCompletionResponse> {
        let mut path = String::from("/chat/completions/{completion_id}");
        path = path.replace("{completion_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.completion_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateChatCompletion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chat/completions/{completion_id}/messages` — `getChatCompletionMessages`.
    pub async fn get_chat_completion_messages(
        &self,
        request: GetChatCompletionMessagesRequest,
    ) -> PlatformResult<GetChatCompletionMessagesResponse> {
        let mut path = String::from("/chat/completions/{completion_id}/messages");
        path = path.replace("{completion_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.completion_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getChatCompletionMessages",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chatkit/sessions` — `CreateChatSessionMethod`.
    pub async fn create_chat_session_method(
        &self,
        request: CreateChatSessionMethodRequest,
    ) -> PlatformResult<CreateChatSessionMethodResponse> {
        let mut path = String::from("/chatkit/sessions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "CreateChatSessionMethod",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chatkit/sessions/{session_id}/cancel` — `CancelChatSessionMethod`.
    pub async fn cancel_chat_session_method(
        &self,
        request: CancelChatSessionMethodRequest,
    ) -> PlatformResult<CancelChatSessionMethodResponse> {
        let mut path = String::from("/chatkit/sessions/{session_id}/cancel");
        path = path.replace("{session_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.session_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "CancelChatSessionMethod",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chatkit/threads` — `ListThreadsMethod`.
    pub async fn list_threads_method(
        &self,
        request: ListThreadsMethodRequest,
    ) -> PlatformResult<ListThreadsMethodResponse> {
        let mut path = String::from("/chatkit/threads");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "ListThreadsMethod",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /chatkit/threads/{thread_id}` — `DeleteThreadMethod`.
    pub async fn delete_thread_method(
        &self,
        request: DeleteThreadMethodRequest,
    ) -> PlatformResult<DeleteThreadMethodResponse> {
        let mut path = String::from("/chatkit/threads/{thread_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "DeleteThreadMethod",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chatkit/threads/{thread_id}` — `GetThreadMethod`.
    pub async fn get_thread_method(
        &self,
        request: GetThreadMethodRequest,
    ) -> PlatformResult<GetThreadMethodResponse> {
        let mut path = String::from("/chatkit/threads/{thread_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "GetThreadMethod",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chatkit/threads/{thread_id}/items` — `ListThreadItemsMethod`.
    pub async fn list_thread_items_method(
        &self,
        request: ListThreadItemsMethodRequest,
    ) -> PlatformResult<ListThreadItemsMethodResponse> {
        let mut path = String::from("/chatkit/threads/{thread_id}/items");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "ListThreadItemsMethod",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /completions` — `createCompletion`.
    pub async fn create_completion(
        &self,
        request: CreateCompletionRequest,
    ) -> PlatformResult<CreateCompletionResponse> {
        let mut path = String::from("/completions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createCompletion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers` — `ListContainers`.
    pub async fn list_containers(
        &self,
        request: ListContainersRequest,
    ) -> PlatformResult<ListContainersResponse> {
        let mut path = String::from("/containers");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "ListContainers",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /containers` — `CreateContainer`.
    pub async fn create_container(
        &self,
        request: CreateContainerRequest,
    ) -> PlatformResult<CreateContainerResponse> {
        let mut path = String::from("/containers");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "CreateContainer",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /containers/{container_id}` — `DeleteContainer`.
    pub async fn delete_container(
        &self,
        request: DeleteContainerRequest,
    ) -> PlatformResult<DeleteContainerResponse> {
        let mut path = String::from("/containers/{container_id}");
        path = path.replace("{container_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.container_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "DeleteContainer",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers/{container_id}` — `RetrieveContainer`.
    pub async fn retrieve_container(
        &self,
        request: RetrieveContainerRequest,
    ) -> PlatformResult<RetrieveContainerResponse> {
        let mut path = String::from("/containers/{container_id}");
        path = path.replace("{container_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.container_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "RetrieveContainer",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers/{container_id}/files` — `ListContainerFiles`.
    pub async fn list_container_files(
        &self,
        request: ListContainerFilesRequest,
    ) -> PlatformResult<ListContainerFilesResponse> {
        let mut path = String::from("/containers/{container_id}/files");
        path = path.replace("{container_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.container_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "ListContainerFiles",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /containers/{container_id}/files` — `CreateContainerFile`.
    pub async fn create_container_file(
        &self,
        request: CreateContainerFileRequest,
    ) -> PlatformResult<CreateContainerFileResponse> {
        let mut path = String::from("/containers/{container_id}/files");
        path = path.replace("{container_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.container_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "CreateContainerFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /containers/{container_id}/files/{file_id}` — `DeleteContainerFile`.
    pub async fn delete_container_file(
        &self,
        request: DeleteContainerFileRequest,
    ) -> PlatformResult<DeleteContainerFileResponse> {
        let mut path = String::from("/containers/{container_id}/files/{file_id}");
        path = path.replace("{container_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.container_id));
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "DeleteContainerFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers/{container_id}/files/{file_id}` — `RetrieveContainerFile`.
    pub async fn retrieve_container_file(
        &self,
        request: RetrieveContainerFileRequest,
    ) -> PlatformResult<RetrieveContainerFileResponse> {
        let mut path = String::from("/containers/{container_id}/files/{file_id}");
        path = path.replace("{container_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.container_id));
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "RetrieveContainerFile",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers/{container_id}/files/{file_id}/content` — `RetrieveContainerFileContent`.
    pub async fn retrieve_container_file_content(
        &self,
        request: RetrieveContainerFileContentRequest,
    ) -> PlatformResult<RetrieveContainerFileContentResponse> {
        let mut path = String::from("/containers/{container_id}/files/{file_id}/content");
        path = path.replace("{container_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.container_id));
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "RetrieveContainerFileContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /conversations` — `createConversation`.
    pub async fn create_conversation(
        &self,
        request: CreateConversationRequest,
    ) -> PlatformResult<CreateConversationResponse> {
        let mut path = String::from("/conversations");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createConversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /conversations/{conversation_id}` — `deleteConversation`.
    pub async fn delete_conversation(
        &self,
        request: DeleteConversationRequest,
    ) -> PlatformResult<DeleteConversationResponse> {
        let mut path = String::from("/conversations/{conversation_id}");
        path = path.replace("{conversation_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteConversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /conversations/{conversation_id}` — `getConversation`.
    pub async fn get_conversation(
        &self,
        request: GetConversationRequest,
    ) -> PlatformResult<GetConversationResponse> {
        let mut path = String::from("/conversations/{conversation_id}");
        path = path.replace("{conversation_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getConversation",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /conversations/{conversation_id}` — `updateConversation`.
    pub async fn update_conversation(
        &self,
        request: UpdateConversationRequest,
    ) -> PlatformResult<UpdateConversationResponse> {
        let mut path = String::from("/conversations/{conversation_id}");
        path = path.replace("{conversation_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateConversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /conversations/{conversation_id}/items` — `listConversationItems`.
    pub async fn list_conversation_items(
        &self,
        request: ListConversationItemsRequest,
    ) -> PlatformResult<ListConversationItemsResponse> {
        let mut path = String::from("/conversations/{conversation_id}/items");
        path = path.replace("{conversation_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listConversationItems",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /conversations/{conversation_id}/items` — `createConversationItems`.
    pub async fn create_conversation_items(
        &self,
        request: CreateConversationItemsRequest,
    ) -> PlatformResult<CreateConversationItemsResponse> {
        let mut path = String::from("/conversations/{conversation_id}/items");
        path = path.replace("{conversation_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createConversationItems",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /conversations/{conversation_id}/items/{item_id}` — `deleteConversationItem`.
    pub async fn delete_conversation_item(
        &self,
        request: DeleteConversationItemRequest,
    ) -> PlatformResult<DeleteConversationItemResponse> {
        let mut path = String::from("/conversations/{conversation_id}/items/{item_id}");
        path = path.replace("{conversation_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id));
        path = path.replace("{item_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.item_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteConversationItem",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /conversations/{conversation_id}/items/{item_id}` — `getConversationItem`.
    pub async fn get_conversation_item(
        &self,
        request: GetConversationItemRequest,
    ) -> PlatformResult<GetConversationItemResponse> {
        let mut path = String::from("/conversations/{conversation_id}/items/{item_id}");
        path = path.replace("{conversation_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id));
        path = path.replace("{item_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.item_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getConversationItem",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /embeddings` — `createEmbedding`.
    pub async fn create_embedding(
        &self,
        request: CreateEmbeddingRequest,
    ) -> PlatformResult<CreateEmbeddingResponse> {
        let mut path = String::from("/embeddings");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createEmbedding",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals` — `listEvals`.
    pub async fn list_evals(
        &self,
        request: ListEvalsRequest,
    ) -> PlatformResult<ListEvalsResponse> {
        let mut path = String::from("/evals");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listEvals",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /evals` — `createEval`.
    pub async fn create_eval(
        &self,
        request: CreateEvalRequest,
    ) -> PlatformResult<CreateEvalResponse> {
        let mut path = String::from("/evals");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createEval",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /evals/{eval_id}` — `deleteEval`.
    pub async fn delete_eval(
        &self,
        request: DeleteEvalRequest,
    ) -> PlatformResult<DeleteEvalResponse> {
        let mut path = String::from("/evals/{eval_id}");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteEval",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}` — `getEval`.
    pub async fn get_eval(
        &self,
        request: GetEvalRequest,
    ) -> PlatformResult<GetEvalResponse> {
        let mut path = String::from("/evals/{eval_id}");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getEval",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /evals/{eval_id}` — `updateEval`.
    pub async fn update_eval(
        &self,
        request: UpdateEvalRequest,
    ) -> PlatformResult<UpdateEvalResponse> {
        let mut path = String::from("/evals/{eval_id}");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateEval",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}/runs` — `getEvalRuns`.
    pub async fn get_eval_runs(
        &self,
        request: GetEvalRunsRequest,
    ) -> PlatformResult<GetEvalRunsResponse> {
        let mut path = String::from("/evals/{eval_id}/runs");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getEvalRuns",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /evals/{eval_id}/runs` — `createEvalRun`.
    pub async fn create_eval_run(
        &self,
        request: CreateEvalRunRequest,
    ) -> PlatformResult<CreateEvalRunResponse> {
        let mut path = String::from("/evals/{eval_id}/runs");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createEvalRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /evals/{eval_id}/runs/{run_id}` — `deleteEvalRun`.
    pub async fn delete_eval_run(
        &self,
        request: DeleteEvalRunRequest,
    ) -> PlatformResult<DeleteEvalRunResponse> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteEvalRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}/runs/{run_id}` — `getEvalRun`.
    pub async fn get_eval_run(
        &self,
        request: GetEvalRunRequest,
    ) -> PlatformResult<GetEvalRunResponse> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getEvalRun",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /evals/{eval_id}/runs/{run_id}` — `cancelEvalRun`.
    pub async fn cancel_eval_run(
        &self,
        request: CancelEvalRunRequest,
    ) -> PlatformResult<CancelEvalRunResponse> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "cancelEvalRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}/runs/{run_id}/output_items` — `getEvalRunOutputItems`.
    pub async fn get_eval_run_output_items(
        &self,
        request: GetEvalRunOutputItemsRequest,
    ) -> PlatformResult<GetEvalRunOutputItemsResponse> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}/output_items");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getEvalRunOutputItems",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}/runs/{run_id}/output_items/{output_item_id}` — `getEvalRunOutputItem`.
    pub async fn get_eval_run_output_item(
        &self,
        request: GetEvalRunOutputItemRequest,
    ) -> PlatformResult<GetEvalRunOutputItemResponse> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}/output_items/{output_item_id}");
        path = path.replace("{eval_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        path = path.replace("{output_item_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.output_item_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getEvalRunOutputItem",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /files` — `listFiles`.
    pub async fn list_files(
        &self,
        request: ListFilesRequest,
    ) -> PlatformResult<ListFilesResponse> {
        let mut path = String::from("/files");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listFiles",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /files` — `createFile`.
    pub async fn create_file(
        &self,
        request: CreateFileRequest,
    ) -> PlatformResult<CreateFileResponse> {
        let mut path = String::from("/files");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "createFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /files/{file_id}` — `deleteFile`.
    pub async fn delete_file(
        &self,
        request: DeleteFileRequest,
    ) -> PlatformResult<DeleteFileResponse> {
        let mut path = String::from("/files/{file_id}");
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /files/{file_id}` — `retrieveFile`.
    pub async fn retrieve_file(
        &self,
        request: RetrieveFileRequest,
    ) -> PlatformResult<RetrieveFileResponse> {
        let mut path = String::from("/files/{file_id}");
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieveFile",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /files/{file_id}/content` — `downloadFile`.
    pub async fn download_file(
        &self,
        request: DownloadFileRequest,
    ) -> PlatformResult<DownloadFileResponse> {
        let mut path = String::from("/files/{file_id}/content");
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "downloadFile",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/alpha/graders/run` — `runGrader`.
    pub async fn run_grader(
        &self,
        request: RunGraderRequest,
    ) -> PlatformResult<RunGraderResponse> {
        let mut path = String::from("/fine_tuning/alpha/graders/run");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "runGrader",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/alpha/graders/validate` — `validateGrader`.
    pub async fn validate_grader(
        &self,
        request: ValidateGraderRequest,
    ) -> PlatformResult<ValidateGraderResponse> {
        let mut path = String::from("/fine_tuning/alpha/graders/validate");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "validateGrader",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions` — `listFineTuningCheckpointPermissions`.
    pub async fn list_fine_tuning_checkpoint_permissions(
        &self,
        request: ListFineTuningCheckpointPermissionsRequest,
    ) -> PlatformResult<ListFineTuningCheckpointPermissionsResponse> {
        let mut path = String::from("/fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions");
        path = path.replace("{fine_tuned_model_checkpoint}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuned_model_checkpoint));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listFineTuningCheckpointPermissions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions` — `createFineTuningCheckpointPermission`.
    pub async fn create_fine_tuning_checkpoint_permission(
        &self,
        request: CreateFineTuningCheckpointPermissionRequest,
    ) -> PlatformResult<CreateFineTuningCheckpointPermissionResponse> {
        let mut path = String::from("/fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions");
        path = path.replace("{fine_tuned_model_checkpoint}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuned_model_checkpoint));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createFineTuningCheckpointPermission",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions/{permission_id}` — `deleteFineTuningCheckpointPermission`.
    pub async fn delete_fine_tuning_checkpoint_permission(
        &self,
        request: DeleteFineTuningCheckpointPermissionRequest,
    ) -> PlatformResult<DeleteFineTuningCheckpointPermissionResponse> {
        let mut path = String::from("/fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions/{permission_id}");
        path = path.replace("{fine_tuned_model_checkpoint}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuned_model_checkpoint));
        path = path.replace("{permission_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.permission_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteFineTuningCheckpointPermission",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/jobs` — `listPaginatedFineTuningJobs`.
    pub async fn list_paginated_fine_tuning_jobs(
        &self,
        request: ListPaginatedFineTuningJobsRequest,
    ) -> PlatformResult<ListPaginatedFineTuningJobsResponse> {
        let mut path = String::from("/fine_tuning/jobs");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listPaginatedFineTuningJobs",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/jobs` — `createFineTuningJob`.
    pub async fn create_fine_tuning_job(
        &self,
        request: CreateFineTuningJobRequest,
    ) -> PlatformResult<CreateFineTuningJobResponse> {
        let mut path = String::from("/fine_tuning/jobs");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createFineTuningJob",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/jobs/{fine_tuning_job_id}` — `retrieveFineTuningJob`.
    pub async fn retrieve_fine_tuning_job(
        &self,
        request: RetrieveFineTuningJobRequest,
    ) -> PlatformResult<RetrieveFineTuningJobResponse> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}");
        path = path.replace("{fine_tuning_job_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieveFineTuningJob",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/jobs/{fine_tuning_job_id}/cancel` — `cancelFineTuningJob`.
    pub async fn cancel_fine_tuning_job(
        &self,
        request: CancelFineTuningJobRequest,
    ) -> PlatformResult<CancelFineTuningJobResponse> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/cancel");
        path = path.replace("{fine_tuning_job_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "cancelFineTuningJob",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/jobs/{fine_tuning_job_id}/checkpoints` — `listFineTuningJobCheckpoints`.
    pub async fn list_fine_tuning_job_checkpoints(
        &self,
        request: ListFineTuningJobCheckpointsRequest,
    ) -> PlatformResult<ListFineTuningJobCheckpointsResponse> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/checkpoints");
        path = path.replace("{fine_tuning_job_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listFineTuningJobCheckpoints",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/jobs/{fine_tuning_job_id}/events` — `listFineTuningEvents`.
    pub async fn list_fine_tuning_events(
        &self,
        request: ListFineTuningEventsRequest,
    ) -> PlatformResult<ListFineTuningEventsResponse> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/events");
        path = path.replace("{fine_tuning_job_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listFineTuningEvents",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/jobs/{fine_tuning_job_id}/pause` — `pauseFineTuningJob`.
    pub async fn pause_fine_tuning_job(
        &self,
        request: PauseFineTuningJobRequest,
    ) -> PlatformResult<PauseFineTuningJobResponse> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/pause");
        path = path.replace("{fine_tuning_job_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "pauseFineTuningJob",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/jobs/{fine_tuning_job_id}/resume` — `resumeFineTuningJob`.
    pub async fn resume_fine_tuning_job(
        &self,
        request: ResumeFineTuningJobRequest,
    ) -> PlatformResult<ResumeFineTuningJobResponse> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/resume");
        path = path.replace("{fine_tuning_job_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "resumeFineTuningJob",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /images/edits` — `createImageEdit`.
    pub async fn create_image_edit(
        &self,
        request: CreateImageEditRequest,
    ) -> PlatformResult<CreateImageEditResponse> {
        let mut path = String::from("/images/edits");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: true,
            operation_id: "createImageEdit",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /images/generations` — `createImage`.
    pub async fn create_image(
        &self,
        request: CreateImageRequest,
    ) -> PlatformResult<CreateImageResponse> {
        let mut path = String::from("/images/generations");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createImage",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /images/variations` — `createImageVariation`.
    pub async fn create_image_variation(
        &self,
        request: CreateImageVariationRequest,
    ) -> PlatformResult<CreateImageVariationResponse> {
        let mut path = String::from("/images/variations");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "createImageVariation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /models` — `listModels`.
    pub async fn list_models(
        &self,
        request: ListModelsRequest,
    ) -> PlatformResult<ListModelsResponse> {
        let mut path = String::from("/models");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listModels",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /models/{model}` — `deleteModel`.
    pub async fn delete_model(
        &self,
        request: DeleteModelRequest,
    ) -> PlatformResult<DeleteModelResponse> {
        let mut path = String::from("/models/{model}");
        path = path.replace("{model}", &crate::openai_platform::url_policy::encode_path_segment(&request.model));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteModel",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /models/{model}` — `retrieveModel`.
    pub async fn retrieve_model(
        &self,
        request: RetrieveModelRequest,
    ) -> PlatformResult<RetrieveModelResponse> {
        let mut path = String::from("/models/{model}");
        path = path.replace("{model}", &crate::openai_platform::url_policy::encode_path_segment(&request.model));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieveModel",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /moderations` — `createModeration`.
    pub async fn create_moderation(
        &self,
        request: CreateModerationRequest,
    ) -> PlatformResult<CreateModerationResponse> {
        let mut path = String::from("/moderations");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createModeration",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/groups/{group_id}/roles` — `list-project-group-role-assignments`.
    pub async fn list_project_group_role_assignments(
        &self,
        request: ListProjectGroupRoleAssignmentsRequest,
    ) -> PlatformResult<ListProjectGroupRoleAssignmentsResponse> {
        let mut path = String::from("/projects/{project_id}/groups/{group_id}/roles");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-group-role-assignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /projects/{project_id}/groups/{group_id}/roles` — `assign-project-group-role`.
    pub async fn assign_project_group_role(
        &self,
        request: AssignProjectGroupRoleRequest,
    ) -> PlatformResult<AssignProjectGroupRoleResponse> {
        let mut path = String::from("/projects/{project_id}/groups/{group_id}/roles");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "assign-project-group-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /projects/{project_id}/groups/{group_id}/roles/{role_id}` — `unassign-project-group-role`.
    pub async fn unassign_project_group_role(
        &self,
        request: UnassignProjectGroupRoleRequest,
    ) -> PlatformResult<UnassignProjectGroupRoleResponse> {
        let mut path = String::from("/projects/{project_id}/groups/{group_id}/roles/{role_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "unassign-project-group-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/groups/{group_id}/roles/{role_id}` — `retrieve-project-group-role`.
    pub async fn retrieve_project_group_role(
        &self,
        request: RetrieveProjectGroupRoleRequest,
    ) -> PlatformResult<RetrieveProjectGroupRoleResponse> {
        let mut path = String::from("/projects/{project_id}/groups/{group_id}/roles/{role_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-group-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/roles` — `list-project-roles`.
    pub async fn list_project_roles(
        &self,
        request: ListProjectRolesRequest,
    ) -> PlatformResult<ListProjectRolesResponse> {
        let mut path = String::from("/projects/{project_id}/roles");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-roles",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /projects/{project_id}/roles` — `create-project-role`.
    pub async fn create_project_role(
        &self,
        request: CreateProjectRoleRequest,
    ) -> PlatformResult<CreateProjectRoleResponse> {
        let mut path = String::from("/projects/{project_id}/roles");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-project-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /projects/{project_id}/roles/{role_id}` — `delete-project-role`.
    pub async fn delete_project_role(
        &self,
        request: DeleteProjectRoleRequest,
    ) -> PlatformResult<DeleteProjectRoleResponse> {
        let mut path = String::from("/projects/{project_id}/roles/{role_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-project-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/roles/{role_id}` — `retrieve-project-role`.
    pub async fn retrieve_project_role(
        &self,
        request: RetrieveProjectRoleRequest,
    ) -> PlatformResult<RetrieveProjectRoleResponse> {
        let mut path = String::from("/projects/{project_id}/roles/{role_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /projects/{project_id}/roles/{role_id}` — `update-project-role`.
    pub async fn update_project_role(
        &self,
        request: UpdateProjectRoleRequest,
    ) -> PlatformResult<UpdateProjectRoleResponse> {
        let mut path = String::from("/projects/{project_id}/roles/{role_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-project-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/users/{user_id}/roles` — `list-project-user-role-assignments`.
    pub async fn list_project_user_role_assignments(
        &self,
        request: ListProjectUserRoleAssignmentsRequest,
    ) -> PlatformResult<ListProjectUserRoleAssignmentsResponse> {
        let mut path = String::from("/projects/{project_id}/users/{user_id}/roles");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-user-role-assignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /projects/{project_id}/users/{user_id}/roles` — `assign-project-user-role`.
    pub async fn assign_project_user_role(
        &self,
        request: AssignProjectUserRoleRequest,
    ) -> PlatformResult<AssignProjectUserRoleResponse> {
        let mut path = String::from("/projects/{project_id}/users/{user_id}/roles");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "assign-project-user-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /projects/{project_id}/users/{user_id}/roles/{role_id}` — `unassign-project-user-role`.
    pub async fn unassign_project_user_role(
        &self,
        request: UnassignProjectUserRoleRequest,
    ) -> PlatformResult<UnassignProjectUserRoleResponse> {
        let mut path = String::from("/projects/{project_id}/users/{user_id}/roles/{role_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "unassign-project-user-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/users/{user_id}/roles/{role_id}` — `retrieve-project-user-role`.
    pub async fn retrieve_project_user_role(
        &self,
        request: RetrieveProjectUserRoleRequest,
    ) -> PlatformResult<RetrieveProjectUserRoleResponse> {
        let mut path = String::from("/projects/{project_id}/users/{user_id}/roles/{role_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-user-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls` — `create-realtime-call`.
    pub async fn create_realtime_call(
        &self,
        request: CreateRealtimeCallRequest,
    ) -> PlatformResult<CreateRealtimeCallResponse> {
        let mut path = String::from("/realtime/calls");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "create-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls/{call_id}/accept` — `accept-realtime-call`.
    pub async fn accept_realtime_call(
        &self,
        request: AcceptRealtimeCallRequest,
    ) -> PlatformResult<AcceptRealtimeCallResponse> {
        let mut path = String::from("/realtime/calls/{call_id}/accept");
        path = path.replace("{call_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.call_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "accept-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls/{call_id}/hangup` — `hangup-realtime-call`.
    pub async fn hangup_realtime_call(
        &self,
        request: HangupRealtimeCallRequest,
    ) -> PlatformResult<HangupRealtimeCallResponse> {
        let mut path = String::from("/realtime/calls/{call_id}/hangup");
        path = path.replace("{call_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.call_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "hangup-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls/{call_id}/refer` — `refer-realtime-call`.
    pub async fn refer_realtime_call(
        &self,
        request: ReferRealtimeCallRequest,
    ) -> PlatformResult<ReferRealtimeCallResponse> {
        let mut path = String::from("/realtime/calls/{call_id}/refer");
        path = path.replace("{call_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.call_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "refer-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls/{call_id}/reject` — `reject-realtime-call`.
    pub async fn reject_realtime_call(
        &self,
        request: RejectRealtimeCallRequest,
    ) -> PlatformResult<RejectRealtimeCallResponse> {
        let mut path = String::from("/realtime/calls/{call_id}/reject");
        path = path.replace("{call_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.call_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "reject-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/client_secrets` — `create-realtime-client-secret`.
    pub async fn create_realtime_client_secret(
        &self,
        request: CreateRealtimeClientSecretRequest,
    ) -> PlatformResult<CreateRealtimeClientSecretResponse> {
        let mut path = String::from("/realtime/client_secrets");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-realtime-client-secret",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/sessions` — `create-realtime-session`.
    pub async fn create_realtime_session(
        &self,
        request: CreateRealtimeSessionRequest,
    ) -> PlatformResult<CreateRealtimeSessionResponse> {
        let mut path = String::from("/realtime/sessions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-realtime-session",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/transcription_sessions` — `create-realtime-transcription-session`.
    pub async fn create_realtime_transcription_session(
        &self,
        request: CreateRealtimeTranscriptionSessionRequest,
    ) -> PlatformResult<CreateRealtimeTranscriptionSessionResponse> {
        let mut path = String::from("/realtime/transcription_sessions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-realtime-transcription-session",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/translations/client_secrets` — `create-realtime-translation-client-secret`.
    pub async fn create_realtime_translation_client_secret(
        &self,
        request: CreateRealtimeTranslationClientSecretRequest,
    ) -> PlatformResult<CreateRealtimeTranslationClientSecretResponse> {
        let mut path = String::from("/realtime/translations/client_secrets");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-realtime-translation-client-secret",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses` — `createResponse`.
    pub async fn create_response(
        &self,
        request: CreateResponseRequest,
    ) -> PlatformResult<CreateResponseResponse> {
        let mut path = String::from("/responses");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/compact` — `Compactconversation`.
    pub async fn compactconversation(
        &self,
        request: CompactconversationRequest,
    ) -> PlatformResult<CompactconversationResponse> {
        let mut path = String::from("/responses/compact");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "Compactconversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/compact?beta=true` — `beta_Compactconversation`.
    pub async fn beta_compactconversation(
        &self,
        request: BetaCompactconversationRequest,
    ) -> PlatformResult<BetaCompactconversationResponse> {
        let mut path = String::from("/responses/compact?beta=true");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "beta_Compactconversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/input_tokens` — `Getinputtokencounts`.
    pub async fn getinputtokencounts(
        &self,
        request: GetinputtokencountsRequest,
    ) -> PlatformResult<GetinputtokencountsResponse> {
        let mut path = String::from("/responses/input_tokens");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "Getinputtokencounts",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/input_tokens?beta=true` — `beta_Getinputtokencounts`.
    pub async fn beta_getinputtokencounts(
        &self,
        request: BetaGetinputtokencountsRequest,
    ) -> PlatformResult<BetaGetinputtokencountsResponse> {
        let mut path = String::from("/responses/input_tokens?beta=true");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "beta_Getinputtokencounts",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /responses/{response_id}` — `deleteResponse`.
    pub async fn delete_response(
        &self,
        request: DeleteResponseRequest,
    ) -> PlatformResult<DeleteResponseResponse> {
        let mut path = String::from("/responses/{response_id}");
        path = path.replace("{response_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.response_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /responses/{response_id}` — `getResponse`.
    pub async fn get_response(
        &self,
        request: GetResponseRequest,
    ) -> PlatformResult<GetResponseResponse> {
        let mut path = String::from("/responses/{response_id}");
        path = path.replace("{response_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.response_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getResponse",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/{response_id}/cancel` — `cancelResponse`.
    pub async fn cancel_response(
        &self,
        request: CancelResponseRequest,
    ) -> PlatformResult<CancelResponseResponse> {
        let mut path = String::from("/responses/{response_id}/cancel");
        path = path.replace("{response_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.response_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "cancelResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/{response_id}/cancel?beta=true` — `beta_cancelResponse`.
    pub async fn beta_cancel_response(
        &self,
        request: BetaCancelResponseRequest,
    ) -> PlatformResult<BetaCancelResponseResponse> {
        let mut path = String::from("/responses/{response_id}/cancel?beta=true");
        path = path.replace("{response_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.response_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "beta_cancelResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /responses/{response_id}/input_items` — `listInputItems`.
    pub async fn list_input_items(
        &self,
        request: ListInputItemsRequest,
    ) -> PlatformResult<ListInputItemsResponse> {
        let mut path = String::from("/responses/{response_id}/input_items");
        path = path.replace("{response_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.response_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listInputItems",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /responses/{response_id}/input_items?beta=true` — `beta_listInputItems`.
    pub async fn beta_list_input_items(
        &self,
        request: BetaListInputItemsRequest,
    ) -> PlatformResult<BetaListInputItemsResponse> {
        let mut path = String::from("/responses/{response_id}/input_items?beta=true");
        path = path.replace("{response_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.response_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "beta_listInputItems",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /responses/{response_id}?beta=true` — `beta_deleteResponse`.
    pub async fn beta_delete_response(
        &self,
        request: BetaDeleteResponseRequest,
    ) -> PlatformResult<BetaDeleteResponseResponse> {
        let mut path = String::from("/responses/{response_id}?beta=true");
        path = path.replace("{response_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.response_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "beta_deleteResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /responses/{response_id}?beta=true` — `beta_getResponse`.
    pub async fn beta_get_response(
        &self,
        request: BetaGetResponseRequest,
    ) -> PlatformResult<BetaGetResponseResponse> {
        let mut path = String::from("/responses/{response_id}?beta=true");
        path = path.replace("{response_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.response_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "beta_getResponse",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses?beta=true` — `beta_createResponse`.
    pub async fn beta_create_response(
        &self,
        request: BetaCreateResponseRequest,
    ) -> PlatformResult<BetaCreateResponseResponse> {
        let mut path = String::from("/responses?beta=true");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "beta_createResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills` — `ListSkills`.
    pub async fn list_skills(
        &self,
        request: ListSkillsRequest,
    ) -> PlatformResult<ListSkillsResponse> {
        let mut path = String::from("/skills");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "ListSkills",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /skills` — `CreateSkill`.
    pub async fn create_skill(
        &self,
        request: CreateSkillRequest,
    ) -> PlatformResult<CreateSkillResponse> {
        let mut path = String::from("/skills");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "CreateSkill",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /skills/{skill_id}` — `DeleteSkill`.
    pub async fn delete_skill(
        &self,
        request: DeleteSkillRequest,
    ) -> PlatformResult<DeleteSkillResponse> {
        let mut path = String::from("/skills/{skill_id}");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "DeleteSkill",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}` — `GetSkill`.
    pub async fn get_skill(
        &self,
        request: GetSkillRequest,
    ) -> PlatformResult<GetSkillResponse> {
        let mut path = String::from("/skills/{skill_id}");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "GetSkill",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /skills/{skill_id}` — `UpdateSkillDefaultVersion`.
    pub async fn update_skill_default_version(
        &self,
        request: UpdateSkillDefaultVersionRequest,
    ) -> PlatformResult<UpdateSkillDefaultVersionResponse> {
        let mut path = String::from("/skills/{skill_id}");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "UpdateSkillDefaultVersion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}/content` — `GetSkillContent`.
    pub async fn get_skill_content(
        &self,
        request: GetSkillContentRequest,
    ) -> PlatformResult<GetSkillContentResponse> {
        let mut path = String::from("/skills/{skill_id}/content");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: true,
            multipart: false,
            operation_id: "GetSkillContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}/versions` — `ListSkillVersions`.
    pub async fn list_skill_versions(
        &self,
        request: ListSkillVersionsRequest,
    ) -> PlatformResult<ListSkillVersionsResponse> {
        let mut path = String::from("/skills/{skill_id}/versions");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "ListSkillVersions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /skills/{skill_id}/versions` — `CreateSkillVersion`.
    pub async fn create_skill_version(
        &self,
        request: CreateSkillVersionRequest,
    ) -> PlatformResult<CreateSkillVersionResponse> {
        let mut path = String::from("/skills/{skill_id}/versions");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "CreateSkillVersion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /skills/{skill_id}/versions/{version}` — `DeleteSkillVersion`.
    pub async fn delete_skill_version(
        &self,
        request: DeleteSkillVersionRequest,
    ) -> PlatformResult<DeleteSkillVersionResponse> {
        let mut path = String::from("/skills/{skill_id}/versions/{version}");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        path = path.replace("{version}", &crate::openai_platform::url_policy::encode_path_segment(&request.version));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "DeleteSkillVersion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}/versions/{version}` — `GetSkillVersion`.
    pub async fn get_skill_version(
        &self,
        request: GetSkillVersionRequest,
    ) -> PlatformResult<GetSkillVersionResponse> {
        let mut path = String::from("/skills/{skill_id}/versions/{version}");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        path = path.replace("{version}", &crate::openai_platform::url_policy::encode_path_segment(&request.version));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "GetSkillVersion",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}/versions/{version}/content` — `GetSkillVersionContent`.
    pub async fn get_skill_version_content(
        &self,
        request: GetSkillVersionContentRequest,
    ) -> PlatformResult<GetSkillVersionContentResponse> {
        let mut path = String::from("/skills/{skill_id}/versions/{version}/content");
        path = path.replace("{skill_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id));
        path = path.replace("{version}", &crate::openai_platform::url_policy::encode_path_segment(&request.version));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: true,
            multipart: false,
            operation_id: "GetSkillVersionContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads` — `createThread`.
    pub async fn create_thread(
        &self,
        request: CreateThreadRequest,
    ) -> PlatformResult<CreateThreadResponse> {
        let mut path = String::from("/threads");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createThread",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/runs` — `createThreadAndRun`.
    pub async fn create_thread_and_run(
        &self,
        request: CreateThreadAndRunRequest,
    ) -> PlatformResult<CreateThreadAndRunResponse> {
        let mut path = String::from("/threads/runs");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createThreadAndRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /threads/{thread_id}` — `deleteThread`.
    pub async fn delete_thread(
        &self,
        request: DeleteThreadRequest,
    ) -> PlatformResult<DeleteThreadResponse> {
        let mut path = String::from("/threads/{thread_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteThread",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}` — `getThread`.
    pub async fn get_thread(
        &self,
        request: GetThreadRequest,
    ) -> PlatformResult<GetThreadResponse> {
        let mut path = String::from("/threads/{thread_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getThread",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}` — `modifyThread`.
    pub async fn modify_thread(
        &self,
        request: ModifyThreadRequest,
    ) -> PlatformResult<ModifyThreadResponse> {
        let mut path = String::from("/threads/{thread_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modifyThread",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/messages` — `listMessages`.
    pub async fn list_messages(
        &self,
        request: ListMessagesRequest,
    ) -> PlatformResult<ListMessagesResponse> {
        let mut path = String::from("/threads/{thread_id}/messages");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listMessages",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/messages` — `createMessage`.
    pub async fn create_message(
        &self,
        request: CreateMessageRequest,
    ) -> PlatformResult<CreateMessageResponse> {
        let mut path = String::from("/threads/{thread_id}/messages");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createMessage",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /threads/{thread_id}/messages/{message_id}` — `deleteMessage`.
    pub async fn delete_message(
        &self,
        request: DeleteMessageRequest,
    ) -> PlatformResult<DeleteMessageResponse> {
        let mut path = String::from("/threads/{thread_id}/messages/{message_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{message_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.message_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteMessage",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/messages/{message_id}` — `getMessage`.
    pub async fn get_message(
        &self,
        request: GetMessageRequest,
    ) -> PlatformResult<GetMessageResponse> {
        let mut path = String::from("/threads/{thread_id}/messages/{message_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{message_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.message_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getMessage",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/messages/{message_id}` — `modifyMessage`.
    pub async fn modify_message(
        &self,
        request: ModifyMessageRequest,
    ) -> PlatformResult<ModifyMessageResponse> {
        let mut path = String::from("/threads/{thread_id}/messages/{message_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{message_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.message_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modifyMessage",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/runs` — `listRuns`.
    pub async fn list_runs(
        &self,
        request: ListRunsRequest,
    ) -> PlatformResult<ListRunsResponse> {
        let mut path = String::from("/threads/{thread_id}/runs");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listRuns",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs` — `createRun`.
    pub async fn create_run(
        &self,
        request: CreateRunRequest,
    ) -> PlatformResult<CreateRunResponse> {
        let mut path = String::from("/threads/{thread_id}/runs");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/runs/{run_id}` — `getRun`.
    pub async fn get_run(
        &self,
        request: GetRunRequest,
    ) -> PlatformResult<GetRunResponse> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getRun",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs/{run_id}` — `modifyRun`.
    pub async fn modify_run(
        &self,
        request: ModifyRunRequest,
    ) -> PlatformResult<ModifyRunResponse> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modifyRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs/{run_id}/cancel` — `cancelRun`.
    pub async fn cancel_run(
        &self,
        request: CancelRunRequest,
    ) -> PlatformResult<CancelRunResponse> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/cancel");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "cancelRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/runs/{run_id}/steps` — `listRunSteps`.
    pub async fn list_run_steps(
        &self,
        request: ListRunStepsRequest,
    ) -> PlatformResult<ListRunStepsResponse> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/steps");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listRunSteps",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/runs/{run_id}/steps/{step_id}` — `getRunStep`.
    pub async fn get_run_step(
        &self,
        request: GetRunStepRequest,
    ) -> PlatformResult<GetRunStepResponse> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/steps/{step_id}");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        path = path.replace("{step_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.step_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getRunStep",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs/{run_id}/submit_tool_outputs` — `submitToolOuputsToRun`.
    pub async fn submit_tool_ouputs_to_run(
        &self,
        request: SubmitToolOuputsToRunRequest,
    ) -> PlatformResult<SubmitToolOuputsToRunResponse> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/submit_tool_outputs");
        path = path.replace("{thread_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id));
        path = path.replace("{run_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.run_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "submitToolOuputsToRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /uploads` — `createUpload`.
    pub async fn create_upload(
        &self,
        request: CreateUploadRequest,
    ) -> PlatformResult<CreateUploadResponse> {
        let mut path = String::from("/uploads");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createUpload",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /uploads/{upload_id}/cancel` — `cancelUpload`.
    pub async fn cancel_upload(
        &self,
        request: CancelUploadRequest,
    ) -> PlatformResult<CancelUploadResponse> {
        let mut path = String::from("/uploads/{upload_id}/cancel");
        path = path.replace("{upload_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.upload_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "cancelUpload",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /uploads/{upload_id}/complete` — `completeUpload`.
    pub async fn complete_upload(
        &self,
        request: CompleteUploadRequest,
    ) -> PlatformResult<CompleteUploadResponse> {
        let mut path = String::from("/uploads/{upload_id}/complete");
        path = path.replace("{upload_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.upload_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "completeUpload",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /uploads/{upload_id}/parts` — `addUploadPart`.
    pub async fn add_upload_part(
        &self,
        request: AddUploadPartRequest,
    ) -> PlatformResult<AddUploadPartResponse> {
        let mut path = String::from("/uploads/{upload_id}/parts");
        path = path.replace("{upload_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.upload_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "addUploadPart",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores` — `listVectorStores`.
    pub async fn list_vector_stores(
        &self,
        request: ListVectorStoresRequest,
    ) -> PlatformResult<ListVectorStoresResponse> {
        let mut path = String::from("/vector_stores");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listVectorStores",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores` — `createVectorStore`.
    pub async fn create_vector_store(
        &self,
        request: CreateVectorStoreRequest,
    ) -> PlatformResult<CreateVectorStoreResponse> {
        let mut path = String::from("/vector_stores");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createVectorStore",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /vector_stores/{vector_store_id}` — `deleteVectorStore`.
    pub async fn delete_vector_store(
        &self,
        request: DeleteVectorStoreRequest,
    ) -> PlatformResult<DeleteVectorStoreResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteVectorStore",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}` — `getVectorStore`.
    pub async fn get_vector_store(
        &self,
        request: GetVectorStoreRequest,
    ) -> PlatformResult<GetVectorStoreResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getVectorStore",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}` — `modifyVectorStore`.
    pub async fn modify_vector_store(
        &self,
        request: ModifyVectorStoreRequest,
    ) -> PlatformResult<ModifyVectorStoreResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modifyVectorStore",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/file_batches` — `createVectorStoreFileBatch`.
    pub async fn create_vector_store_file_batch(
        &self,
        request: CreateVectorStoreFileBatchRequest,
    ) -> PlatformResult<CreateVectorStoreFileBatchResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/file_batches");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createVectorStoreFileBatch",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/file_batches/{batch_id}` — `getVectorStoreFileBatch`.
    pub async fn get_vector_store_file_batch(
        &self,
        request: GetVectorStoreFileBatchRequest,
    ) -> PlatformResult<GetVectorStoreFileBatchResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/file_batches/{batch_id}");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        path = path.replace("{batch_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getVectorStoreFileBatch",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/file_batches/{batch_id}/cancel` — `cancelVectorStoreFileBatch`.
    pub async fn cancel_vector_store_file_batch(
        &self,
        request: CancelVectorStoreFileBatchRequest,
    ) -> PlatformResult<CancelVectorStoreFileBatchResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/file_batches/{batch_id}/cancel");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        path = path.replace("{batch_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "cancelVectorStoreFileBatch",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/file_batches/{batch_id}/files` — `listFilesInVectorStoreBatch`.
    pub async fn list_files_in_vector_store_batch(
        &self,
        request: ListFilesInVectorStoreBatchRequest,
    ) -> PlatformResult<ListFilesInVectorStoreBatchResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/file_batches/{batch_id}/files");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        path = path.replace("{batch_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listFilesInVectorStoreBatch",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/files` — `listVectorStoreFiles`.
    pub async fn list_vector_store_files(
        &self,
        request: ListVectorStoreFilesRequest,
    ) -> PlatformResult<ListVectorStoreFilesResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listVectorStoreFiles",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/files` — `createVectorStoreFile`.
    pub async fn create_vector_store_file(
        &self,
        request: CreateVectorStoreFileRequest,
    ) -> PlatformResult<CreateVectorStoreFileResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createVectorStoreFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /vector_stores/{vector_store_id}/files/{file_id}` — `deleteVectorStoreFile`.
    pub async fn delete_vector_store_file(
        &self,
        request: DeleteVectorStoreFileRequest,
    ) -> PlatformResult<DeleteVectorStoreFileResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files/{file_id}");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteVectorStoreFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/files/{file_id}` — `getVectorStoreFile`.
    pub async fn get_vector_store_file(
        &self,
        request: GetVectorStoreFileRequest,
    ) -> PlatformResult<GetVectorStoreFileResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files/{file_id}");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getVectorStoreFile",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/files/{file_id}` — `updateVectorStoreFileAttributes`.
    pub async fn update_vector_store_file_attributes(
        &self,
        request: UpdateVectorStoreFileAttributesRequest,
    ) -> PlatformResult<UpdateVectorStoreFileAttributesResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files/{file_id}");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateVectorStoreFileAttributes",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/files/{file_id}/content` — `retrieveVectorStoreFileContent`.
    pub async fn retrieve_vector_store_file_content(
        &self,
        request: RetrieveVectorStoreFileContentRequest,
    ) -> PlatformResult<RetrieveVectorStoreFileContentResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files/{file_id}/content");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        path = path.replace("{file_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.file_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieveVectorStoreFileContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/search` — `searchVectorStore`.
    pub async fn search_vector_store(
        &self,
        request: SearchVectorStoreRequest,
    ) -> PlatformResult<SearchVectorStoreResponse> {
        let mut path = String::from("/vector_stores/{vector_store_id}/search");
        path = path.replace("{vector_store_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "searchVectorStore",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos` — `ListVideos`.
    pub async fn list_videos(
        &self,
        request: ListVideosRequest,
    ) -> PlatformResult<ListVideosResponse> {
        let mut path = String::from("/videos");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "ListVideos",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos` — `createVideo`.
    pub async fn create_video(
        &self,
        request: CreateVideoRequest,
    ) -> PlatformResult<CreateVideoResponse> {
        let mut path = String::from("/videos");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "createVideo",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos/characters` — `CreateVideoCharacter`.
    pub async fn create_video_character(
        &self,
        request: CreateVideoCharacterRequest,
    ) -> PlatformResult<CreateVideoCharacterResponse> {
        let mut path = String::from("/videos/characters");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "CreateVideoCharacter",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/characters/{character_id}` — `GetVideoCharacter`.
    pub async fn get_video_character(
        &self,
        request: GetVideoCharacterRequest,
    ) -> PlatformResult<GetVideoCharacterResponse> {
        let mut path = String::from("/videos/characters/{character_id}");
        path = path.replace("{character_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.character_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "GetVideoCharacter",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos/edits` — `CreateVideoEdit`.
    pub async fn create_video_edit(
        &self,
        request: CreateVideoEditRequest,
    ) -> PlatformResult<CreateVideoEditResponse> {
        let mut path = String::from("/videos/edits");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "CreateVideoEdit",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos/extensions` — `CreateVideoExtend`.
    pub async fn create_video_extend(
        &self,
        request: CreateVideoExtendRequest,
    ) -> PlatformResult<CreateVideoExtendResponse> {
        let mut path = String::from("/videos/extensions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "CreateVideoExtend",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /videos/{video_id}` — `DeleteVideo`.
    pub async fn delete_video(
        &self,
        request: DeleteVideoRequest,
    ) -> PlatformResult<DeleteVideoResponse> {
        let mut path = String::from("/videos/{video_id}");
        path = path.replace("{video_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.video_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "DeleteVideo",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/{video_id}` — `GetVideo`.
    pub async fn get_video(
        &self,
        request: GetVideoRequest,
    ) -> PlatformResult<GetVideoResponse> {
        let mut path = String::from("/videos/{video_id}");
        path = path.replace("{video_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.video_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "GetVideo",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/{video_id}/content` — `RetrieveVideoContent`.
    pub async fn retrieve_video_content(
        &self,
        request: RetrieveVideoContentRequest,
    ) -> PlatformResult<RetrieveVideoContentResponse> {
        let mut path = String::from("/videos/{video_id}/content");
        path = path.replace("{video_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.video_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: true,
            multipart: false,
            operation_id: "RetrieveVideoContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos/{video_id}/remix` — `CreateVideoRemix`.
    pub async fn create_video_remix(
        &self,
        request: CreateVideoRemixRequest,
    ) -> PlatformResult<CreateVideoRemixResponse> {
        let mut path = String::from("/videos/{video_id}/remix");
        path = path.replace("{video_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.video_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: true,
            operation_id: "CreateVideoRemix",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

}
