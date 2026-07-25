//! Generated typed operations for openrouter platform baseline.
//! DO NOT EDIT BY HAND — regenerate via baselines/scripts/generate_platform_client.py

use super::super::error::{PlatformError, PlatformResult};
use super::super::transport::{CredentialKind, HttpRequestSpec, PlatformTransport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Request for `GET /activity` (`getUserActivity`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetUserActivityRequest {
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

/// Response for `GET /activity` (`getUserActivity`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetUserActivityResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /analytics/meta` (`getAnalyticsMeta`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetAnalyticsMetaRequest {
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

/// Response for `GET /analytics/meta` (`getAnalyticsMeta`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetAnalyticsMetaResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /analytics/query` (`queryAnalytics`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct QueryAnalyticsRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: QueryAnalyticsBody,
}

/// Body for `queryAnalytics`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct QueryAnalyticsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl QueryAnalyticsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /analytics/query` (`queryAnalytics`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct QueryAnalyticsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /audio/speech` (`createAudioSpeech`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAudioSpeechRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateAudioSpeechBody,
}

/// Body for `createAudioSpeech`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAudioSpeechBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateAudioSpeechBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /audio/speech` (`createAudioSpeech`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAudioSpeechResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /audio/transcriptions` (`createAudioTranscriptions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAudioTranscriptionsRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateAudioTranscriptionsBody,
}

/// Body for `createAudioTranscriptions`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAudioTranscriptionsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateAudioTranscriptionsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /audio/transcriptions` (`createAudioTranscriptions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAudioTranscriptionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /auth/keys` (`exchangeAuthCodeForAPIKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExchangeAuthCodeForAPIKeyRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ExchangeAuthCodeForAPIKeyBody,
}

/// Body for `exchangeAuthCodeForAPIKey`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExchangeAuthCodeForAPIKeyBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ExchangeAuthCodeForAPIKeyBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /auth/keys` (`exchangeAuthCodeForAPIKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExchangeAuthCodeForAPIKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /auth/keys/code` (`createAuthKeysCode`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAuthKeysCodeRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateAuthKeysCodeBody,
}

/// Body for `createAuthKeysCode`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAuthKeysCodeBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateAuthKeysCodeBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /auth/keys/code` (`createAuthKeysCode`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateAuthKeysCodeResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /benchmarks` (`getBenchmarks`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetBenchmarksRequest {
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

/// Response for `GET /benchmarks` (`getBenchmarks`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetBenchmarksResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /byok` (`listBYOKKeys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListBYOKKeysRequest {
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

/// Response for `GET /byok` (`listBYOKKeys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListBYOKKeysResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /byok` (`createBYOKKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateBYOKKeyRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateBYOKKeyBody,
}

/// Body for `createBYOKKey`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateBYOKKeyBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateBYOKKeyBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /byok` (`createBYOKKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateBYOKKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /byok/{id}` (`deleteBYOKKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteBYOKKeyRequest {
    pub id: String,
}

/// Response for `DELETE /byok/{id}` (`deleteBYOKKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteBYOKKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /byok/{id}` (`getBYOKKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetBYOKKeyRequest {
    pub id: String,
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

/// Response for `GET /byok/{id}` (`getBYOKKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetBYOKKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `PATCH /byok/{id}` (`updateBYOKKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateBYOKKeyRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateBYOKKeyBody,
}

/// Body for `updateBYOKKey`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateBYOKKeyBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateBYOKKeyBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `PATCH /byok/{id}` (`updateBYOKKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateBYOKKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /chat/completions` (`sendChatCompletionRequest`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SendChatCompletionRequestRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: SendChatCompletionRequestBody,
}

/// Body for `sendChatCompletionRequest`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SendChatCompletionRequestBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl SendChatCompletionRequestBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /chat/completions` (`sendChatCompletionRequest`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SendChatCompletionRequestResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /classifications/task` (`getTaskClassifications`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetTaskClassificationsRequest {
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

/// Response for `GET /classifications/task` (`getTaskClassifications`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetTaskClassificationsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /credits` (`getCredits`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetCreditsRequest {
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

/// Response for `GET /credits` (`getCredits`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetCreditsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /credits/coinbase` (`createCoinbaseCharge`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateCoinbaseChargeRequest {
}

/// Response for `POST /credits/coinbase` (`createCoinbaseCharge`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateCoinbaseChargeResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /datasets/app-rankings` (`getAppRankings`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetAppRankingsRequest {
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

/// Response for `GET /datasets/app-rankings` (`getAppRankings`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetAppRankingsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /datasets/rankings-daily` (`getRankingsDaily`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetRankingsDailyRequest {
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

/// Response for `GET /datasets/rankings-daily` (`getRankingsDaily`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetRankingsDailyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /embeddings` (`createEmbeddings`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEmbeddingsRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateEmbeddingsBody,
}

/// Body for `createEmbeddings`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEmbeddingsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateEmbeddingsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /embeddings` (`createEmbeddings`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateEmbeddingsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /embeddings/models` (`listEmbeddingsModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListEmbeddingsModelsRequest {
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

/// Response for `GET /embeddings/models` (`listEmbeddingsModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListEmbeddingsModelsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /endpoints/zdr` (`listEndpointsZdr`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListEndpointsZdrRequest {
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

/// Response for `GET /endpoints/zdr` (`listEndpointsZdr`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListEndpointsZdrResponse {
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

/// Request for `POST /files` (`uploadFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UploadFileRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UploadFileBody,
}

/// Body for `uploadFile`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UploadFileBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UploadFileBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /files` (`uploadFile`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UploadFileResponse {
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

/// Request for `GET /files/{file_id}` (`getFileMetadata`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetFileMetadataRequest {
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

/// Response for `GET /files/{file_id}` (`getFileMetadata`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetFileMetadataResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /files/{file_id}/content` (`downloadFileContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DownloadFileContentRequest {
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

/// Response for `GET /files/{file_id}/content` (`downloadFileContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DownloadFileContentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /generation` (`getGeneration`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetGenerationRequest {
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

/// Response for `GET /generation` (`getGeneration`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetGenerationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /generation/content` (`listGenerationContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGenerationContentRequest {
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

/// Response for `GET /generation/content` (`listGenerationContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGenerationContentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /generation/feedback` (`submitGenerationFeedback`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SubmitGenerationFeedbackRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: SubmitGenerationFeedbackBody,
}

/// Body for `submitGenerationFeedback`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SubmitGenerationFeedbackBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl SubmitGenerationFeedbackBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /generation/feedback` (`submitGenerationFeedback`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SubmitGenerationFeedbackResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /guardrails` (`listGuardrails`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGuardrailsRequest {
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

/// Response for `GET /guardrails` (`listGuardrails`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGuardrailsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /guardrails` (`createGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateGuardrailRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateGuardrailBody,
}

/// Body for `createGuardrail`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateGuardrailBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateGuardrailBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /guardrails` (`createGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateGuardrailResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /guardrails/assignments/keys` (`listKeyAssignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListKeyAssignmentsRequest {
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

/// Response for `GET /guardrails/assignments/keys` (`listKeyAssignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListKeyAssignmentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /guardrails/assignments/members` (`listMemberAssignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListMemberAssignmentsRequest {
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

/// Response for `GET /guardrails/assignments/members` (`listMemberAssignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListMemberAssignmentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /guardrails/{id}` (`deleteGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteGuardrailRequest {
    pub id: String,
}

/// Response for `DELETE /guardrails/{id}` (`deleteGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteGuardrailResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /guardrails/{id}` (`getGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetGuardrailRequest {
    pub id: String,
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

/// Response for `GET /guardrails/{id}` (`getGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetGuardrailResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `PATCH /guardrails/{id}` (`updateGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateGuardrailRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateGuardrailBody,
}

/// Body for `updateGuardrail`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateGuardrailBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateGuardrailBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `PATCH /guardrails/{id}` (`updateGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateGuardrailResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /guardrails/{id}/assignments/keys` (`listGuardrailKeyAssignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGuardrailKeyAssignmentsRequest {
    pub id: String,
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

/// Response for `GET /guardrails/{id}/assignments/keys` (`listGuardrailKeyAssignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGuardrailKeyAssignmentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /guardrails/{id}/assignments/keys` (`bulkAssignKeysToGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAssignKeysToGuardrailRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BulkAssignKeysToGuardrailBody,
}

/// Body for `bulkAssignKeysToGuardrail`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAssignKeysToGuardrailBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BulkAssignKeysToGuardrailBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /guardrails/{id}/assignments/keys` (`bulkAssignKeysToGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAssignKeysToGuardrailResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /guardrails/{id}/assignments/keys/remove` (`bulkUnassignKeysFromGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkUnassignKeysFromGuardrailRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BulkUnassignKeysFromGuardrailBody,
}

/// Body for `bulkUnassignKeysFromGuardrail`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkUnassignKeysFromGuardrailBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BulkUnassignKeysFromGuardrailBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /guardrails/{id}/assignments/keys/remove` (`bulkUnassignKeysFromGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkUnassignKeysFromGuardrailResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /guardrails/{id}/assignments/members` (`listGuardrailMemberAssignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGuardrailMemberAssignmentsRequest {
    pub id: String,
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

/// Response for `GET /guardrails/{id}/assignments/members` (`listGuardrailMemberAssignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGuardrailMemberAssignmentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /guardrails/{id}/assignments/members` (`bulkAssignMembersToGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAssignMembersToGuardrailRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BulkAssignMembersToGuardrailBody,
}

/// Body for `bulkAssignMembersToGuardrail`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAssignMembersToGuardrailBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BulkAssignMembersToGuardrailBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /guardrails/{id}/assignments/members` (`bulkAssignMembersToGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAssignMembersToGuardrailResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /guardrails/{id}/assignments/members/remove` (`bulkUnassignMembersFromGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkUnassignMembersFromGuardrailRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BulkUnassignMembersFromGuardrailBody,
}

/// Body for `bulkUnassignMembersFromGuardrail`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkUnassignMembersFromGuardrailBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BulkUnassignMembersFromGuardrailBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /guardrails/{id}/assignments/members/remove` (`bulkUnassignMembersFromGuardrail`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkUnassignMembersFromGuardrailResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /images` (`createImages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImagesRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateImagesBody,
}

/// Body for `createImages`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImagesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateImagesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /images` (`createImages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateImagesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /images/models` (`listImageModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListImageModelsRequest {
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

/// Response for `GET /images/models` (`listImageModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListImageModelsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /images/models/{author}/{slug}/endpoints` (`listImageModelEndpoints`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListImageModelEndpointsRequest {
    pub author: String,
    pub slug: String,
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

/// Response for `GET /images/models/{author}/{slug}/endpoints` (`listImageModelEndpoints`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListImageModelEndpointsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /key` (`getCurrentKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetCurrentKeyRequest {
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

/// Response for `GET /key` (`getCurrentKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetCurrentKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /keys` (`list`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListRequest {
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

/// Response for `GET /keys` (`list`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /keys` (`createKeys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateKeysRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateKeysBody,
}

/// Body for `createKeys`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateKeysBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateKeysBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /keys` (`createKeys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateKeysResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /keys/{hash}` (`deleteKeys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteKeysRequest {
    pub hash: String,
}

/// Response for `DELETE /keys/{hash}` (`deleteKeys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteKeysResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /keys/{hash}` (`getKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetKeyRequest {
    pub hash: String,
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

/// Response for `GET /keys/{hash}` (`getKey`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `PATCH /keys/{hash}` (`updateKeys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateKeysRequest {
    pub hash: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateKeysBody,
}

/// Body for `updateKeys`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateKeysBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateKeysBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `PATCH /keys/{hash}` (`updateKeys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateKeysResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /messages` (`createMessages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateMessagesRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateMessagesBody,
}

/// Body for `createMessages`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateMessagesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateMessagesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /messages` (`createMessages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateMessagesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /model/{author}/{slug}` (`getModel`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetModelRequest {
    pub author: String,
    pub slug: String,
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

/// Response for `GET /model/{author}/{slug}` (`getModel`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetModelResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /models` (`getModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetModelsRequest {
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

/// Response for `GET /models` (`getModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetModelsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /models/count` (`listModelsCount`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListModelsCountRequest {
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

/// Response for `GET /models/count` (`listModelsCount`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListModelsCountResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /models/user` (`listModelsUser`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListModelsUserRequest {
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

/// Response for `GET /models/user` (`listModelsUser`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListModelsUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /models/{author}/{slug}/endpoints` (`listEndpoints`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListEndpointsRequest {
    pub author: String,
    pub slug: String,
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

/// Response for `GET /models/{author}/{slug}/endpoints` (`listEndpoints`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListEndpointsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /observability/destinations` (`listObservabilityDestinations`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListObservabilityDestinationsRequest {
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

/// Response for `GET /observability/destinations` (`listObservabilityDestinations`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListObservabilityDestinationsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /observability/destinations` (`createObservabilityDestination`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateObservabilityDestinationRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateObservabilityDestinationBody,
}

/// Body for `createObservabilityDestination`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateObservabilityDestinationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateObservabilityDestinationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /observability/destinations` (`createObservabilityDestination`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateObservabilityDestinationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /observability/destinations/{id}` (`deleteObservabilityDestination`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteObservabilityDestinationRequest {
    pub id: String,
}

/// Response for `DELETE /observability/destinations/{id}` (`deleteObservabilityDestination`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteObservabilityDestinationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /observability/destinations/{id}` (`getObservabilityDestination`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetObservabilityDestinationRequest {
    pub id: String,
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

/// Response for `GET /observability/destinations/{id}` (`getObservabilityDestination`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetObservabilityDestinationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `PATCH /observability/destinations/{id}` (`updateObservabilityDestination`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateObservabilityDestinationRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateObservabilityDestinationBody,
}

/// Body for `updateObservabilityDestination`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateObservabilityDestinationBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateObservabilityDestinationBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `PATCH /observability/destinations/{id}` (`updateObservabilityDestination`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateObservabilityDestinationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/members` (`listOrganizationMembers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListOrganizationMembersRequest {
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

/// Response for `GET /organization/members` (`listOrganizationMembers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListOrganizationMembersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /presets` (`listPresets`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListPresetsRequest {
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

/// Response for `GET /presets` (`listPresets`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListPresetsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /presets/{slug}` (`getPreset`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetPresetRequest {
    pub slug: String,
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

/// Response for `GET /presets/{slug}` (`getPreset`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetPresetResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /presets/{slug}/chat/completions` (`createPresetsChatCompletions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsChatCompletionsRequest {
    pub slug: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreatePresetsChatCompletionsBody,
}

/// Body for `createPresetsChatCompletions`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsChatCompletionsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreatePresetsChatCompletionsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /presets/{slug}/chat/completions` (`createPresetsChatCompletions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsChatCompletionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /presets/{slug}/messages` (`createPresetsMessages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsMessagesRequest {
    pub slug: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreatePresetsMessagesBody,
}

/// Body for `createPresetsMessages`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsMessagesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreatePresetsMessagesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /presets/{slug}/messages` (`createPresetsMessages`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsMessagesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /presets/{slug}/responses` (`createPresetsResponses`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsResponsesRequest {
    pub slug: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreatePresetsResponsesBody,
}

/// Body for `createPresetsResponses`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsResponsesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreatePresetsResponsesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /presets/{slug}/responses` (`createPresetsResponses`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreatePresetsResponsesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /presets/{slug}/versions` (`listPresetVersions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListPresetVersionsRequest {
    pub slug: String,
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

/// Response for `GET /presets/{slug}/versions` (`listPresetVersions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListPresetVersionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /presets/{slug}/versions/{version}` (`getPresetVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetPresetVersionRequest {
    pub slug: String,
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

/// Response for `GET /presets/{slug}/versions/{version}` (`getPresetVersion`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetPresetVersionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /providers` (`listProviders`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProvidersRequest {
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

/// Response for `GET /providers` (`listProviders`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProvidersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /rerank` (`createRerank`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRerankRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateRerankBody,
}

/// Body for `createRerank`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRerankBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateRerankBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /rerank` (`createRerank`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRerankResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /responses` (`createResponses`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateResponsesRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateResponsesBody,
}

/// Body for `createResponses`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateResponsesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateResponsesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /responses` (`createResponses`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateResponsesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /videos` (`createVideos`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideosRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateVideosBody,
}

/// Body for `createVideos`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideosBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateVideosBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /videos` (`createVideos`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateVideosResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /videos/models` (`listVideosModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVideosModelsRequest {
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

/// Response for `GET /videos/models` (`listVideosModels`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVideosModelsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /videos/{jobId}` (`getVideos`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVideosRequest {
    pub job_id: String,
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

/// Response for `GET /videos/{jobId}` (`getVideos`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetVideosResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /videos/{jobId}/content` (`listVideosContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVideosContentRequest {
    pub job_id: String,
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

/// Response for `GET /videos/{jobId}/content` (`listVideosContent`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListVideosContentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /workspaces` (`listWorkspaces`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListWorkspacesRequest {
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

/// Response for `GET /workspaces` (`listWorkspaces`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListWorkspacesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /workspaces` (`createWorkspace`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateWorkspaceRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateWorkspaceBody,
}

/// Body for `createWorkspace`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateWorkspaceBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateWorkspaceBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /workspaces` (`createWorkspace`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateWorkspaceResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /workspaces/{id}` (`deleteWorkspace`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteWorkspaceRequest {
    pub id: String,
}

/// Response for `DELETE /workspaces/{id}` (`deleteWorkspace`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteWorkspaceResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /workspaces/{id}` (`getWorkspace`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetWorkspaceRequest {
    pub id: String,
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

/// Response for `GET /workspaces/{id}` (`getWorkspace`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetWorkspaceResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `PATCH /workspaces/{id}` (`updateWorkspace`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateWorkspaceRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateWorkspaceBody,
}

/// Body for `updateWorkspace`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateWorkspaceBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateWorkspaceBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `PATCH /workspaces/{id}` (`updateWorkspace`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateWorkspaceResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /workspaces/{id}/budgets` (`listWorkspaceBudgets`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListWorkspaceBudgetsRequest {
    pub id: String,
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

/// Response for `GET /workspaces/{id}/budgets` (`listWorkspaceBudgets`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListWorkspaceBudgetsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /workspaces/{id}/budgets/{interval}` (`deleteWorkspaceBudget`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteWorkspaceBudgetRequest {
    pub id: String,
    pub interval: String,
}

/// Response for `DELETE /workspaces/{id}/budgets/{interval}` (`deleteWorkspaceBudget`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteWorkspaceBudgetResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `PUT /workspaces/{id}/budgets/{interval}` (`upsertWorkspaceBudget`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpsertWorkspaceBudgetRequest {
    pub id: String,
    pub interval: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpsertWorkspaceBudgetBody,
}

/// Body for `upsertWorkspaceBudget`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpsertWorkspaceBudgetBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpsertWorkspaceBudgetBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `PUT /workspaces/{id}/budgets/{interval}` (`upsertWorkspaceBudget`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpsertWorkspaceBudgetResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /workspaces/{id}/members` (`listWorkspaceMembers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListWorkspaceMembersRequest {
    pub id: String,
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

/// Response for `GET /workspaces/{id}/members` (`listWorkspaceMembers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListWorkspaceMembersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /workspaces/{id}/members/add` (`bulkAddWorkspaceMembers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAddWorkspaceMembersRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BulkAddWorkspaceMembersBody,
}

/// Body for `bulkAddWorkspaceMembers`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAddWorkspaceMembersBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BulkAddWorkspaceMembersBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /workspaces/{id}/members/add` (`bulkAddWorkspaceMembers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkAddWorkspaceMembersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /workspaces/{id}/members/remove` (`bulkRemoveWorkspaceMembers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkRemoveWorkspaceMembersRequest {
    pub id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: BulkRemoveWorkspaceMembersBody,
}

/// Body for `bulkRemoveWorkspaceMembers`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkRemoveWorkspaceMembersBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl BulkRemoveWorkspaceMembersBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /workspaces/{id}/members/remove` (`bulkRemoveWorkspaceMembers`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BulkRemoveWorkspaceMembersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl crate::openai_platform::client::OpenRouterClient {
    /// `GET /activity` — `getUserActivity`.
    pub async fn get_user_activity(
        &self,
        request: GetUserActivityRequest,
    ) -> PlatformResult<GetUserActivityResponse> {
        let mut path = String::from("/activity");
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
            operation_id: "getUserActivity",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /analytics/meta` — `getAnalyticsMeta`.
    pub async fn get_analytics_meta(
        &self,
        request: GetAnalyticsMetaRequest,
    ) -> PlatformResult<GetAnalyticsMetaResponse> {
        let mut path = String::from("/analytics/meta");
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
            operation_id: "getAnalyticsMeta",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /analytics/query` — `queryAnalytics`.
    pub async fn query_analytics(
        &self,
        request: QueryAnalyticsRequest,
    ) -> PlatformResult<QueryAnalyticsResponse> {
        let mut path = String::from("/analytics/query");
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
            operation_id: "queryAnalytics",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/speech` — `createAudioSpeech`.
    pub async fn create_audio_speech(
        &self,
        request: CreateAudioSpeechRequest,
    ) -> PlatformResult<CreateAudioSpeechResponse> {
        let mut path = String::from("/audio/speech");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: true,
            multipart: false,
            operation_id: "createAudioSpeech",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/transcriptions` — `createAudioTranscriptions`.
    pub async fn create_audio_transcriptions(
        &self,
        request: CreateAudioTranscriptionsRequest,
    ) -> PlatformResult<CreateAudioTranscriptionsResponse> {
        let mut path = String::from("/audio/transcriptions");
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
            operation_id: "createAudioTranscriptions",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /auth/keys` — `exchangeAuthCodeForAPIKey`.
    pub async fn exchange_auth_code_for_api_key(
        &self,
        request: ExchangeAuthCodeForAPIKeyRequest,
    ) -> PlatformResult<ExchangeAuthCodeForAPIKeyResponse> {
        let mut path = String::from("/auth/keys");
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
            operation_id: "exchangeAuthCodeForAPIKey",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /auth/keys/code` — `createAuthKeysCode`.
    pub async fn create_auth_keys_code(
        &self,
        request: CreateAuthKeysCodeRequest,
    ) -> PlatformResult<CreateAuthKeysCodeResponse> {
        let mut path = String::from("/auth/keys/code");
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
            operation_id: "createAuthKeysCode",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /benchmarks` — `getBenchmarks`.
    pub async fn get_benchmarks(
        &self,
        request: GetBenchmarksRequest,
    ) -> PlatformResult<GetBenchmarksResponse> {
        let mut path = String::from("/benchmarks");
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
            operation_id: "getBenchmarks",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /byok` — `listBYOKKeys`.
    pub async fn list_byok_keys(
        &self,
        request: ListBYOKKeysRequest,
    ) -> PlatformResult<ListBYOKKeysResponse> {
        let mut path = String::from("/byok");
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
            operation_id: "listBYOKKeys",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /byok` — `createBYOKKey`.
    pub async fn create_byok_key(
        &self,
        request: CreateBYOKKeyRequest,
    ) -> PlatformResult<CreateBYOKKeyResponse> {
        let mut path = String::from("/byok");
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
            operation_id: "createBYOKKey",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /byok/{id}` — `deleteBYOKKey`.
    pub async fn delete_byok_key(
        &self,
        request: DeleteBYOKKeyRequest,
    ) -> PlatformResult<DeleteBYOKKeyResponse> {
        let mut path = String::from("/byok/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "deleteBYOKKey",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /byok/{id}` — `getBYOKKey`.
    pub async fn get_byok_key(
        &self,
        request: GetBYOKKeyRequest,
    ) -> PlatformResult<GetBYOKKeyResponse> {
        let mut path = String::from("/byok/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "getBYOKKey",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `PATCH /byok/{id}` — `updateBYOKKey`.
    pub async fn update_byok_key(
        &self,
        request: UpdateBYOKKeyRequest,
    ) -> PlatformResult<UpdateBYOKKeyResponse> {
        let mut path = String::from("/byok/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "PATCH",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateBYOKKey",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chat/completions` — `sendChatCompletionRequest`.
    pub async fn send_chat_completion_request(
        &self,
        request: SendChatCompletionRequestRequest,
    ) -> PlatformResult<SendChatCompletionRequestResponse> {
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
            operation_id: "sendChatCompletionRequest",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /classifications/task` — `getTaskClassifications`.
    pub async fn get_task_classifications(
        &self,
        request: GetTaskClassificationsRequest,
    ) -> PlatformResult<GetTaskClassificationsResponse> {
        let mut path = String::from("/classifications/task");
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
            operation_id: "getTaskClassifications",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /credits` — `getCredits`.
    pub async fn get_credits(
        &self,
        request: GetCreditsRequest,
    ) -> PlatformResult<GetCreditsResponse> {
        let mut path = String::from("/credits");
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
            operation_id: "getCredits",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /credits/coinbase` — `createCoinbaseCharge`.
    pub async fn create_coinbase_charge(
        &self,
        request: CreateCoinbaseChargeRequest,
    ) -> PlatformResult<CreateCoinbaseChargeResponse> {
        let mut path = String::from("/credits/coinbase");
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
            operation_id: "createCoinbaseCharge",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /datasets/app-rankings` — `getAppRankings`.
    pub async fn get_app_rankings(
        &self,
        request: GetAppRankingsRequest,
    ) -> PlatformResult<GetAppRankingsResponse> {
        let mut path = String::from("/datasets/app-rankings");
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
            operation_id: "getAppRankings",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /datasets/rankings-daily` — `getRankingsDaily`.
    pub async fn get_rankings_daily(
        &self,
        request: GetRankingsDailyRequest,
    ) -> PlatformResult<GetRankingsDailyResponse> {
        let mut path = String::from("/datasets/rankings-daily");
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
            operation_id: "getRankingsDaily",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /embeddings` — `createEmbeddings`.
    pub async fn create_embeddings(
        &self,
        request: CreateEmbeddingsRequest,
    ) -> PlatformResult<CreateEmbeddingsResponse> {
        let mut path = String::from("/embeddings");
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
            operation_id: "createEmbeddings",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /embeddings/models` — `listEmbeddingsModels`.
    pub async fn list_embeddings_models(
        &self,
        request: ListEmbeddingsModelsRequest,
    ) -> PlatformResult<ListEmbeddingsModelsResponse> {
        let mut path = String::from("/embeddings/models");
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
            operation_id: "listEmbeddingsModels",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /endpoints/zdr` — `listEndpointsZdr`.
    pub async fn list_endpoints_zdr(
        &self,
        request: ListEndpointsZdrRequest,
    ) -> PlatformResult<ListEndpointsZdrResponse> {
        let mut path = String::from("/endpoints/zdr");
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
            operation_id: "listEndpointsZdr",
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

    /// `POST /files` — `uploadFile`.
    pub async fn upload_file(
        &self,
        request: UploadFileRequest,
    ) -> PlatformResult<UploadFileResponse> {
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
            operation_id: "uploadFile",
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

    /// `GET /files/{file_id}` — `getFileMetadata`.
    pub async fn get_file_metadata(
        &self,
        request: GetFileMetadataRequest,
    ) -> PlatformResult<GetFileMetadataResponse> {
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
            operation_id: "getFileMetadata",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /files/{file_id}/content` — `downloadFileContent`.
    pub async fn download_file_content(
        &self,
        request: DownloadFileContentRequest,
    ) -> PlatformResult<DownloadFileContentResponse> {
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
            expect_binary: true,
            multipart: false,
            operation_id: "downloadFileContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /generation` — `getGeneration`.
    pub async fn get_generation(
        &self,
        request: GetGenerationRequest,
    ) -> PlatformResult<GetGenerationResponse> {
        let mut path = String::from("/generation");
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
            operation_id: "getGeneration",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /generation/content` — `listGenerationContent`.
    pub async fn list_generation_content(
        &self,
        request: ListGenerationContentRequest,
    ) -> PlatformResult<ListGenerationContentResponse> {
        let mut path = String::from("/generation/content");
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
            operation_id: "listGenerationContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /generation/feedback` — `submitGenerationFeedback`.
    pub async fn submit_generation_feedback(
        &self,
        request: SubmitGenerationFeedbackRequest,
    ) -> PlatformResult<SubmitGenerationFeedbackResponse> {
        let mut path = String::from("/generation/feedback");
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
            operation_id: "submitGenerationFeedback",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /guardrails` — `listGuardrails`.
    pub async fn list_guardrails(
        &self,
        request: ListGuardrailsRequest,
    ) -> PlatformResult<ListGuardrailsResponse> {
        let mut path = String::from("/guardrails");
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
            operation_id: "listGuardrails",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /guardrails` — `createGuardrail`.
    pub async fn create_guardrail(
        &self,
        request: CreateGuardrailRequest,
    ) -> PlatformResult<CreateGuardrailResponse> {
        let mut path = String::from("/guardrails");
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
            operation_id: "createGuardrail",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /guardrails/assignments/keys` — `listKeyAssignments`.
    pub async fn list_key_assignments(
        &self,
        request: ListKeyAssignmentsRequest,
    ) -> PlatformResult<ListKeyAssignmentsResponse> {
        let mut path = String::from("/guardrails/assignments/keys");
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
            operation_id: "listKeyAssignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /guardrails/assignments/members` — `listMemberAssignments`.
    pub async fn list_member_assignments(
        &self,
        request: ListMemberAssignmentsRequest,
    ) -> PlatformResult<ListMemberAssignmentsResponse> {
        let mut path = String::from("/guardrails/assignments/members");
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
            operation_id: "listMemberAssignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /guardrails/{id}` — `deleteGuardrail`.
    pub async fn delete_guardrail(
        &self,
        request: DeleteGuardrailRequest,
    ) -> PlatformResult<DeleteGuardrailResponse> {
        let mut path = String::from("/guardrails/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "deleteGuardrail",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /guardrails/{id}` — `getGuardrail`.
    pub async fn get_guardrail(
        &self,
        request: GetGuardrailRequest,
    ) -> PlatformResult<GetGuardrailResponse> {
        let mut path = String::from("/guardrails/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "getGuardrail",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `PATCH /guardrails/{id}` — `updateGuardrail`.
    pub async fn update_guardrail(
        &self,
        request: UpdateGuardrailRequest,
    ) -> PlatformResult<UpdateGuardrailResponse> {
        let mut path = String::from("/guardrails/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "PATCH",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateGuardrail",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /guardrails/{id}/assignments/keys` — `listGuardrailKeyAssignments`.
    pub async fn list_guardrail_key_assignments(
        &self,
        request: ListGuardrailKeyAssignmentsRequest,
    ) -> PlatformResult<ListGuardrailKeyAssignmentsResponse> {
        let mut path = String::from("/guardrails/{id}/assignments/keys");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "listGuardrailKeyAssignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /guardrails/{id}/assignments/keys` — `bulkAssignKeysToGuardrail`.
    pub async fn bulk_assign_keys_to_guardrail(
        &self,
        request: BulkAssignKeysToGuardrailRequest,
    ) -> PlatformResult<BulkAssignKeysToGuardrailResponse> {
        let mut path = String::from("/guardrails/{id}/assignments/keys");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "bulkAssignKeysToGuardrail",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /guardrails/{id}/assignments/keys/remove` — `bulkUnassignKeysFromGuardrail`.
    pub async fn bulk_unassign_keys_from_guardrail(
        &self,
        request: BulkUnassignKeysFromGuardrailRequest,
    ) -> PlatformResult<BulkUnassignKeysFromGuardrailResponse> {
        let mut path = String::from("/guardrails/{id}/assignments/keys/remove");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "bulkUnassignKeysFromGuardrail",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /guardrails/{id}/assignments/members` — `listGuardrailMemberAssignments`.
    pub async fn list_guardrail_member_assignments(
        &self,
        request: ListGuardrailMemberAssignmentsRequest,
    ) -> PlatformResult<ListGuardrailMemberAssignmentsResponse> {
        let mut path = String::from("/guardrails/{id}/assignments/members");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "listGuardrailMemberAssignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /guardrails/{id}/assignments/members` — `bulkAssignMembersToGuardrail`.
    pub async fn bulk_assign_members_to_guardrail(
        &self,
        request: BulkAssignMembersToGuardrailRequest,
    ) -> PlatformResult<BulkAssignMembersToGuardrailResponse> {
        let mut path = String::from("/guardrails/{id}/assignments/members");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "bulkAssignMembersToGuardrail",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /guardrails/{id}/assignments/members/remove` — `bulkUnassignMembersFromGuardrail`.
    pub async fn bulk_unassign_members_from_guardrail(
        &self,
        request: BulkUnassignMembersFromGuardrailRequest,
    ) -> PlatformResult<BulkUnassignMembersFromGuardrailResponse> {
        let mut path = String::from("/guardrails/{id}/assignments/members/remove");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "bulkUnassignMembersFromGuardrail",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /images` — `createImages`.
    pub async fn create_images(
        &self,
        request: CreateImagesRequest,
    ) -> PlatformResult<CreateImagesResponse> {
        let mut path = String::from("/images");
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
            operation_id: "createImages",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /images/models` — `listImageModels`.
    pub async fn list_image_models(
        &self,
        request: ListImageModelsRequest,
    ) -> PlatformResult<ListImageModelsResponse> {
        let mut path = String::from("/images/models");
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
            operation_id: "listImageModels",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /images/models/{author}/{slug}/endpoints` — `listImageModelEndpoints`.
    pub async fn list_image_model_endpoints(
        &self,
        request: ListImageModelEndpointsRequest,
    ) -> PlatformResult<ListImageModelEndpointsResponse> {
        let mut path = String::from("/images/models/{author}/{slug}/endpoints");
        path = path.replace("{author}", &crate::openai_platform::url_policy::encode_path_segment(&request.author));
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "listImageModelEndpoints",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /key` — `getCurrentKey`.
    pub async fn get_current_key(
        &self,
        request: GetCurrentKeyRequest,
    ) -> PlatformResult<GetCurrentKeyResponse> {
        let mut path = String::from("/key");
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
            operation_id: "getCurrentKey",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /keys` — `list`.
    pub async fn list(
        &self,
        request: ListRequest,
    ) -> PlatformResult<ListResponse> {
        let mut path = String::from("/keys");
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
            operation_id: "list",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /keys` — `createKeys`.
    pub async fn create_keys(
        &self,
        request: CreateKeysRequest,
    ) -> PlatformResult<CreateKeysResponse> {
        let mut path = String::from("/keys");
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
            operation_id: "createKeys",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /keys/{hash}` — `deleteKeys`.
    pub async fn delete_keys(
        &self,
        request: DeleteKeysRequest,
    ) -> PlatformResult<DeleteKeysResponse> {
        let mut path = String::from("/keys/{hash}");
        path = path.replace("{hash}", &crate::openai_platform::url_policy::encode_path_segment(&request.hash));
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
            operation_id: "deleteKeys",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /keys/{hash}` — `getKey`.
    pub async fn get_key(
        &self,
        request: GetKeyRequest,
    ) -> PlatformResult<GetKeyResponse> {
        let mut path = String::from("/keys/{hash}");
        path = path.replace("{hash}", &crate::openai_platform::url_policy::encode_path_segment(&request.hash));
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
            operation_id: "getKey",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `PATCH /keys/{hash}` — `updateKeys`.
    pub async fn update_keys(
        &self,
        request: UpdateKeysRequest,
    ) -> PlatformResult<UpdateKeysResponse> {
        let mut path = String::from("/keys/{hash}");
        path = path.replace("{hash}", &crate::openai_platform::url_policy::encode_path_segment(&request.hash));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "PATCH",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateKeys",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /messages` — `createMessages`.
    pub async fn create_messages(
        &self,
        request: CreateMessagesRequest,
    ) -> PlatformResult<CreateMessagesResponse> {
        let mut path = String::from("/messages");
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
            operation_id: "createMessages",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /model/{author}/{slug}` — `getModel`.
    pub async fn get_model(
        &self,
        request: GetModelRequest,
    ) -> PlatformResult<GetModelResponse> {
        let mut path = String::from("/model/{author}/{slug}");
        path = path.replace("{author}", &crate::openai_platform::url_policy::encode_path_segment(&request.author));
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "getModel",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /models` — `getModels`.
    pub async fn get_models(
        &self,
        request: GetModelsRequest,
    ) -> PlatformResult<GetModelsResponse> {
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
            operation_id: "getModels",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /models/count` — `listModelsCount`.
    pub async fn list_models_count(
        &self,
        request: ListModelsCountRequest,
    ) -> PlatformResult<ListModelsCountResponse> {
        let mut path = String::from("/models/count");
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
            operation_id: "listModelsCount",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /models/user` — `listModelsUser`.
    pub async fn list_models_user(
        &self,
        request: ListModelsUserRequest,
    ) -> PlatformResult<ListModelsUserResponse> {
        let mut path = String::from("/models/user");
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
            operation_id: "listModelsUser",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /models/{author}/{slug}/endpoints` — `listEndpoints`.
    pub async fn list_endpoints(
        &self,
        request: ListEndpointsRequest,
    ) -> PlatformResult<ListEndpointsResponse> {
        let mut path = String::from("/models/{author}/{slug}/endpoints");
        path = path.replace("{author}", &crate::openai_platform::url_policy::encode_path_segment(&request.author));
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "listEndpoints",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /observability/destinations` — `listObservabilityDestinations`.
    pub async fn list_observability_destinations(
        &self,
        request: ListObservabilityDestinationsRequest,
    ) -> PlatformResult<ListObservabilityDestinationsResponse> {
        let mut path = String::from("/observability/destinations");
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
            operation_id: "listObservabilityDestinations",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /observability/destinations` — `createObservabilityDestination`.
    pub async fn create_observability_destination(
        &self,
        request: CreateObservabilityDestinationRequest,
    ) -> PlatformResult<CreateObservabilityDestinationResponse> {
        let mut path = String::from("/observability/destinations");
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
            operation_id: "createObservabilityDestination",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /observability/destinations/{id}` — `deleteObservabilityDestination`.
    pub async fn delete_observability_destination(
        &self,
        request: DeleteObservabilityDestinationRequest,
    ) -> PlatformResult<DeleteObservabilityDestinationResponse> {
        let mut path = String::from("/observability/destinations/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "deleteObservabilityDestination",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /observability/destinations/{id}` — `getObservabilityDestination`.
    pub async fn get_observability_destination(
        &self,
        request: GetObservabilityDestinationRequest,
    ) -> PlatformResult<GetObservabilityDestinationResponse> {
        let mut path = String::from("/observability/destinations/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "getObservabilityDestination",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `PATCH /observability/destinations/{id}` — `updateObservabilityDestination`.
    pub async fn update_observability_destination(
        &self,
        request: UpdateObservabilityDestinationRequest,
    ) -> PlatformResult<UpdateObservabilityDestinationResponse> {
        let mut path = String::from("/observability/destinations/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "PATCH",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateObservabilityDestination",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/members` — `listOrganizationMembers`.
    pub async fn list_organization_members(
        &self,
        request: ListOrganizationMembersRequest,
    ) -> PlatformResult<ListOrganizationMembersResponse> {
        let mut path = String::from("/organization/members");
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
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listOrganizationMembers",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /presets` — `listPresets`.
    pub async fn list_presets(
        &self,
        request: ListPresetsRequest,
    ) -> PlatformResult<ListPresetsResponse> {
        let mut path = String::from("/presets");
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
            operation_id: "listPresets",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /presets/{slug}` — `getPreset`.
    pub async fn get_preset(
        &self,
        request: GetPresetRequest,
    ) -> PlatformResult<GetPresetResponse> {
        let mut path = String::from("/presets/{slug}");
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "getPreset",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /presets/{slug}/chat/completions` — `createPresetsChatCompletions`.
    pub async fn create_presets_chat_completions(
        &self,
        request: CreatePresetsChatCompletionsRequest,
    ) -> PlatformResult<CreatePresetsChatCompletionsResponse> {
        let mut path = String::from("/presets/{slug}/chat/completions");
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "createPresetsChatCompletions",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /presets/{slug}/messages` — `createPresetsMessages`.
    pub async fn create_presets_messages(
        &self,
        request: CreatePresetsMessagesRequest,
    ) -> PlatformResult<CreatePresetsMessagesResponse> {
        let mut path = String::from("/presets/{slug}/messages");
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "createPresetsMessages",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /presets/{slug}/responses` — `createPresetsResponses`.
    pub async fn create_presets_responses(
        &self,
        request: CreatePresetsResponsesRequest,
    ) -> PlatformResult<CreatePresetsResponsesResponse> {
        let mut path = String::from("/presets/{slug}/responses");
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "createPresetsResponses",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /presets/{slug}/versions` — `listPresetVersions`.
    pub async fn list_preset_versions(
        &self,
        request: ListPresetVersionsRequest,
    ) -> PlatformResult<ListPresetVersionsResponse> {
        let mut path = String::from("/presets/{slug}/versions");
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "listPresetVersions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /presets/{slug}/versions/{version}` — `getPresetVersion`.
    pub async fn get_preset_version(
        &self,
        request: GetPresetVersionRequest,
    ) -> PlatformResult<GetPresetVersionResponse> {
        let mut path = String::from("/presets/{slug}/versions/{version}");
        path = path.replace("{slug}", &crate::openai_platform::url_policy::encode_path_segment(&request.slug));
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
            operation_id: "getPresetVersion",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /providers` — `listProviders`.
    pub async fn list_providers(
        &self,
        request: ListProvidersRequest,
    ) -> PlatformResult<ListProvidersResponse> {
        let mut path = String::from("/providers");
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
            operation_id: "listProviders",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /rerank` — `createRerank`.
    pub async fn create_rerank(
        &self,
        request: CreateRerankRequest,
    ) -> PlatformResult<CreateRerankResponse> {
        let mut path = String::from("/rerank");
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
            operation_id: "createRerank",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses` — `createResponses`.
    pub async fn create_responses(
        &self,
        request: CreateResponsesRequest,
    ) -> PlatformResult<CreateResponsesResponse> {
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
            operation_id: "createResponses",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos` — `createVideos`.
    pub async fn create_videos(
        &self,
        request: CreateVideosRequest,
    ) -> PlatformResult<CreateVideosResponse> {
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
            multipart: false,
            operation_id: "createVideos",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/models` — `listVideosModels`.
    pub async fn list_videos_models(
        &self,
        request: ListVideosModelsRequest,
    ) -> PlatformResult<ListVideosModelsResponse> {
        let mut path = String::from("/videos/models");
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
            operation_id: "listVideosModels",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/{jobId}` — `getVideos`.
    pub async fn get_videos(
        &self,
        request: GetVideosRequest,
    ) -> PlatformResult<GetVideosResponse> {
        let mut path = String::from("/videos/{jobId}");
        path = path.replace("{jobId}", &crate::openai_platform::url_policy::encode_path_segment(&request.job_id));
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
            operation_id: "getVideos",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/{jobId}/content` — `listVideosContent`.
    pub async fn list_videos_content(
        &self,
        request: ListVideosContentRequest,
    ) -> PlatformResult<ListVideosContentResponse> {
        let mut path = String::from("/videos/{jobId}/content");
        path = path.replace("{jobId}", &crate::openai_platform::url_policy::encode_path_segment(&request.job_id));
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
            operation_id: "listVideosContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /workspaces` — `listWorkspaces`.
    pub async fn list_workspaces(
        &self,
        request: ListWorkspacesRequest,
    ) -> PlatformResult<ListWorkspacesResponse> {
        let mut path = String::from("/workspaces");
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
            operation_id: "listWorkspaces",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /workspaces` — `createWorkspace`.
    pub async fn create_workspace(
        &self,
        request: CreateWorkspaceRequest,
    ) -> PlatformResult<CreateWorkspaceResponse> {
        let mut path = String::from("/workspaces");
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
            operation_id: "createWorkspace",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /workspaces/{id}` — `deleteWorkspace`.
    pub async fn delete_workspace(
        &self,
        request: DeleteWorkspaceRequest,
    ) -> PlatformResult<DeleteWorkspaceResponse> {
        let mut path = String::from("/workspaces/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "deleteWorkspace",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /workspaces/{id}` — `getWorkspace`.
    pub async fn get_workspace(
        &self,
        request: GetWorkspaceRequest,
    ) -> PlatformResult<GetWorkspaceResponse> {
        let mut path = String::from("/workspaces/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "getWorkspace",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `PATCH /workspaces/{id}` — `updateWorkspace`.
    pub async fn update_workspace(
        &self,
        request: UpdateWorkspaceRequest,
    ) -> PlatformResult<UpdateWorkspaceResponse> {
        let mut path = String::from("/workspaces/{id}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "PATCH",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "updateWorkspace",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /workspaces/{id}/budgets` — `listWorkspaceBudgets`.
    pub async fn list_workspace_budgets(
        &self,
        request: ListWorkspaceBudgetsRequest,
    ) -> PlatformResult<ListWorkspaceBudgetsResponse> {
        let mut path = String::from("/workspaces/{id}/budgets");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "listWorkspaceBudgets",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /workspaces/{id}/budgets/{interval}` — `deleteWorkspaceBudget`.
    pub async fn delete_workspace_budget(
        &self,
        request: DeleteWorkspaceBudgetRequest,
    ) -> PlatformResult<DeleteWorkspaceBudgetResponse> {
        let mut path = String::from("/workspaces/{id}/budgets/{interval}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
        path = path.replace("{interval}", &crate::openai_platform::url_policy::encode_path_segment(&request.interval));
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
            operation_id: "deleteWorkspaceBudget",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `PUT /workspaces/{id}/budgets/{interval}` — `upsertWorkspaceBudget`.
    pub async fn upsert_workspace_budget(
        &self,
        request: UpsertWorkspaceBudgetRequest,
    ) -> PlatformResult<UpsertWorkspaceBudgetResponse> {
        let mut path = String::from("/workspaces/{id}/budgets/{interval}");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
        path = path.replace("{interval}", &crate::openai_platform::url_policy::encode_path_segment(&request.interval));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "PUT",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "upsertWorkspaceBudget",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /workspaces/{id}/members` — `listWorkspaceMembers`.
    pub async fn list_workspace_members(
        &self,
        request: ListWorkspaceMembersRequest,
    ) -> PlatformResult<ListWorkspaceMembersResponse> {
        let mut path = String::from("/workspaces/{id}/members");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "listWorkspaceMembers",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /workspaces/{id}/members/add` — `bulkAddWorkspaceMembers`.
    pub async fn bulk_add_workspace_members(
        &self,
        request: BulkAddWorkspaceMembersRequest,
    ) -> PlatformResult<BulkAddWorkspaceMembersResponse> {
        let mut path = String::from("/workspaces/{id}/members/add");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "bulkAddWorkspaceMembers",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /workspaces/{id}/members/remove` — `bulkRemoveWorkspaceMembers`.
    pub async fn bulk_remove_workspace_members(
        &self,
        request: BulkRemoveWorkspaceMembersRequest,
    ) -> PlatformResult<BulkRemoveWorkspaceMembersResponse> {
        let mut path = String::from("/workspaces/{id}/members/remove");
        path = path.replace("{id}", &crate::openai_platform::url_policy::encode_path_segment(&request.id));
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
            operation_id: "bulkRemoveWorkspaceMembers",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

}
