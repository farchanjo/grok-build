//! Generated typed operations for openrouter.
//! DO NOT EDIT BY HAND.

use super::super::error::{PlatformError, PlatformResult};
use super::super::transport::{CredentialKind, HttpRequestSpec, MultipartFiles};
use super::openrouter_types::*;
use std::collections::BTreeMap;

fn query_value<T: serde::Serialize + ?Sized>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) if !other.is_null() => other.to_string(),
        _ => String::new(),
    }
}

impl crate::openai_platform::client::OpenRouterClient {
    /// `GET /activity` — `getUserActivity` (json).
    /// Transports: http_json.
    pub async fn get_user_activity(
        &self,
        request: GetUserActivityParams,
    ) -> PlatformResult<GetUserActivityResult> {
        let path = String::from("/activity");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.date.as_ref() {
            query.insert("date".into(), query_value(v));
        }
        if let Some(v) = request.api_key_hash.as_ref() {
            query.insert("api_key_hash".into(), query_value(v));
        }
        if let Some(v) = request.user_id.as_ref() {
            query.insert("user_id".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /analytics/meta` — `getAnalyticsMeta` (json).
    /// Transports: http_json.
    pub async fn get_analytics_meta(
        &self,
        _request: GetAnalyticsMetaParams,
    ) -> PlatformResult<GetAnalyticsMetaResult> {
        let path = String::from("/analytics/meta");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /analytics/query` — `queryAnalytics` (json).
    /// Transports: http_json.
    pub async fn query_analytics(
        &self,
        request: QueryAnalyticsParams,
    ) -> PlatformResult<QueryAnalyticsResult> {
        let path = String::from("/analytics/query");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /audio/speech` — `createAudioSpeech` (binary).
    /// Transports: http_binary, http_json.
    pub async fn create_audio_speech(
        &self,
        request: CreateAudioSpeechParams,
        sink: Option<&std::path::Path>,
    ) -> PlatformResult<CreateAudioSpeechResult> {
        let path = String::from("/audio/speech");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;
        Ok(CreateAudioSpeechResult {
            bytes,
            content_type,
        })
    }

    /// `POST /audio/transcriptions` — `createAudioTranscriptions` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_audio_transcriptions(
        &self,
        request: CreateAudioTranscriptionsParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateAudioTranscriptionsResult> {
        let path = String::from("/audio/transcriptions");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /auth/keys` — `exchangeAuthCodeForAPIKey` (json).
    /// Transports: http_json.
    pub async fn exchange_auth_code_for_api_key(
        &self,
        request: ExchangeAuthCodeForAPIKeyParams,
    ) -> PlatformResult<ExchangeAuthCodeForAPIKeyResult> {
        let path = String::from("/auth/keys");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /auth/keys/code` — `createAuthKeysCode` (json).
    /// Transports: http_json.
    pub async fn create_auth_keys_code(
        &self,
        request: CreateAuthKeysCodeParams,
    ) -> PlatformResult<CreateAuthKeysCodeResult> {
        let path = String::from("/auth/keys/code");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /benchmarks` — `getBenchmarks` (json).
    /// Transports: http_json.
    pub async fn get_benchmarks(
        &self,
        request: GetBenchmarksParams,
    ) -> PlatformResult<GetBenchmarksResult> {
        let path = String::from("/benchmarks");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.source.as_ref() {
            query.insert("source".into(), query_value(v));
        }
        if let Some(v) = request.task_type.as_ref() {
            query.insert("task_type".into(), query_value(v));
        }
        if let Some(v) = request.arena.as_ref() {
            query.insert("arena".into(), query_value(v));
        }
        if let Some(v) = request.category.as_ref() {
            query.insert("category".into(), query_value(v));
        }
        if let Some(v) = request.max_results.as_ref() {
            query.insert("max_results".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /byok` — `listBYOKKeys` (json).
    /// Transports: http_json.
    pub async fn list_byok_keys(
        &self,
        request: ListBYOKKeysParams,
    ) -> PlatformResult<ListBYOKKeysResult> {
        let path = String::from("/byok");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        if let Some(v) = request.provider.as_ref() {
            query.insert("provider".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /byok` — `createBYOKKey` (json).
    /// Transports: http_json.
    pub async fn create_byok_key(
        &self,
        request: CreateBYOKKeyParams,
    ) -> PlatformResult<CreateBYOKKeyResult> {
        let path = String::from("/byok");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /byok/{id}` — `deleteBYOKKey` (json).
    /// Transports: http_json.
    pub async fn delete_byok_key(
        &self,
        request: DeleteBYOKKeyParams,
    ) -> PlatformResult<DeleteBYOKKeyResult> {
        let mut path = String::from("/byok/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /byok/{id}` — `getBYOKKey` (json).
    /// Transports: http_json.
    pub async fn get_byok_key(
        &self,
        request: GetBYOKKeyParams,
    ) -> PlatformResult<GetBYOKKeyResult> {
        let mut path = String::from("/byok/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `PATCH /byok/{id}` — `updateBYOKKey` (json).
    /// Transports: http_json.
    pub async fn update_byok_key(
        &self,
        request: UpdateBYOKKeyParams,
    ) -> PlatformResult<UpdateBYOKKeyResult> {
        let mut path = String::from("/byok/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /chat/completions` — `sendChatCompletionRequest` (json).
    /// Transports: http_json, http_sse.
    pub async fn send_chat_completion_request(
        &self,
        request: SendChatCompletionRequestParams,
    ) -> PlatformResult<SendChatCompletionRequestResult> {
        let path = String::from("/chat/completions");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "sendChatCompletionRequest",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chat/completions` — `sendChatCompletionRequest` (sse).
    /// Transports: http_json, http_sse.
    pub async fn send_chat_completion_request_stream(
        &self,
        request: SendChatCompletionRequestParams,
    ) -> PlatformResult<SendChatCompletionRequestSseResult> {
        let path = String::from("/chat/completions");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let events = self.transport.execute_sse(spec).await?;
        Ok(SendChatCompletionRequestSseResult { events })
    }

    /// `GET /classifications/task` — `getTaskClassifications` (json).
    /// Transports: http_json.
    pub async fn get_task_classifications(
        &self,
        request: GetTaskClassificationsParams,
    ) -> PlatformResult<GetTaskClassificationsResult> {
        let path = String::from("/classifications/task");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.window.as_ref() {
            query.insert("window".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /credits` — `getCredits` (json).
    /// Transports: http_json.
    pub async fn get_credits(
        &self,
        _request: GetCreditsParams,
    ) -> PlatformResult<GetCreditsResult> {
        let path = String::from("/credits");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /credits/coinbase` — `createCoinbaseCharge` (json).
    /// Transports: http_json.
    pub async fn create_coinbase_charge(
        &self,
        _request: CreateCoinbaseChargeParams,
    ) -> PlatformResult<CreateCoinbaseChargeResult> {
        let path = String::from("/credits/coinbase");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /datasets/app-rankings` — `getAppRankings` (json).
    /// Transports: http_json.
    pub async fn get_app_rankings(
        &self,
        request: GetAppRankingsParams,
    ) -> PlatformResult<GetAppRankingsResult> {
        let path = String::from("/datasets/app-rankings");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.category.as_ref() {
            query.insert("category".into(), query_value(v));
        }
        if let Some(v) = request.subcategory.as_ref() {
            query.insert("subcategory".into(), query_value(v));
        }
        if let Some(v) = request.sort.as_ref() {
            query.insert("sort".into(), query_value(v));
        }
        if let Some(v) = request.start_date.as_ref() {
            query.insert("start_date".into(), query_value(v));
        }
        if let Some(v) = request.end_date.as_ref() {
            query.insert("end_date".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /datasets/rankings-daily` — `getRankingsDaily` (json).
    /// Transports: http_json.
    pub async fn get_rankings_daily(
        &self,
        request: GetRankingsDailyParams,
    ) -> PlatformResult<GetRankingsDailyResult> {
        let path = String::from("/datasets/rankings-daily");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.start_date.as_ref() {
            query.insert("start_date".into(), query_value(v));
        }
        if let Some(v) = request.end_date.as_ref() {
            query.insert("end_date".into(), query_value(v));
        }
        if let Some(v) = request.period.as_ref() {
            query.insert("period".into(), query_value(v));
        }
        if let Some(v) = request.modality.as_ref() {
            query.insert("modality".into(), query_value(v));
        }
        if let Some(v) = request.context_bucket.as_ref() {
            query.insert("context_bucket".into(), query_value(v));
        }
        if let Some(v) = request.category.as_ref() {
            query.insert("category".into(), query_value(v));
        }
        if let Some(v) = request.language_type.as_ref() {
            query.insert("language_type".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /embeddings` — `createEmbeddings` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_embeddings(
        &self,
        request: CreateEmbeddingsParams,
    ) -> PlatformResult<CreateEmbeddingsResult> {
        let path = String::from("/embeddings");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createEmbeddings",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /embeddings` — `createEmbeddings` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_embeddings_stream(
        &self,
        request: CreateEmbeddingsParams,
    ) -> PlatformResult<CreateEmbeddingsSseResult> {
        let path = String::from("/embeddings");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateEmbeddingsSseResult { events })
    }

    /// `GET /embeddings/models` — `listEmbeddingsModels` (json).
    /// Transports: http_json.
    pub async fn list_embeddings_models(
        &self,
        request: ListEmbeddingsModelsParams,
    ) -> PlatformResult<ListEmbeddingsModelsResult> {
        let path = String::from("/embeddings/models");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /endpoints/zdr` — `listEndpointsZdr` (json).
    /// Transports: http_json.
    pub async fn list_endpoints_zdr(
        &self,
        _request: ListEndpointsZdrParams,
    ) -> PlatformResult<ListEndpointsZdrResult> {
        let path = String::from("/endpoints/zdr");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /files` — `listFiles` (json).
    /// Transports: http_json.
    pub async fn list_files(&self, request: ListFilesParams) -> PlatformResult<ListFilesResult> {
        let path = String::from("/files");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.cursor.as_ref() {
            query.insert("cursor".into(), query_value(v));
        }
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /files` — `uploadFile` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn upload_file(
        &self,
        request: UploadFileParams,
        files: MultipartFiles,
    ) -> PlatformResult<UploadFileResult> {
        let path = String::from("/files");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /files/{file_id}` — `deleteFile` (json).
    /// Transports: http_json.
    pub async fn delete_file(&self, request: DeleteFileParams) -> PlatformResult<DeleteFileResult> {
        let mut path = String::from("/files/{file_id}");
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /files/{file_id}` — `getFileMetadata` (json).
    /// Transports: http_json.
    pub async fn get_file_metadata(
        &self,
        request: GetFileMetadataParams,
    ) -> PlatformResult<GetFileMetadataResult> {
        let mut path = String::from("/files/{file_id}");
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /files/{file_id}/content` — `downloadFileContent` (binary).
    /// Transports: http_binary, http_json.
    pub async fn download_file_content(
        &self,
        request: DownloadFileContentParams,
        sink: Option<&std::path::Path>,
    ) -> PlatformResult<DownloadFileContentResult> {
        let mut path = String::from("/files/{file_id}/content");
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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
        let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;
        Ok(DownloadFileContentResult {
            bytes,
            content_type,
        })
    }

    /// `GET /generation` — `getGeneration` (json).
    /// Transports: http_json.
    pub async fn get_generation(
        &self,
        request: GetGenerationParams,
    ) -> PlatformResult<GetGenerationResult> {
        let path = String::from("/generation");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("id".into(), query_value(&request.id));
        let body: Option<serde_json::Value> = None;
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

    /// `GET /generation/content` — `listGenerationContent` (json).
    /// Transports: http_json.
    pub async fn list_generation_content(
        &self,
        request: ListGenerationContentParams,
    ) -> PlatformResult<ListGenerationContentResult> {
        let path = String::from("/generation/content");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("id".into(), query_value(&request.id));
        let body: Option<serde_json::Value> = None;
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

    /// `POST /generation/feedback` — `submitGenerationFeedback` (json).
    /// Transports: http_json.
    pub async fn submit_generation_feedback(
        &self,
        request: SubmitGenerationFeedbackParams,
    ) -> PlatformResult<SubmitGenerationFeedbackResult> {
        let path = String::from("/generation/feedback");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /guardrails` — `listGuardrails` (json).
    /// Transports: http_json.
    pub async fn list_guardrails(
        &self,
        request: ListGuardrailsParams,
    ) -> PlatformResult<ListGuardrailsResult> {
        let path = String::from("/guardrails");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /guardrails` — `createGuardrail` (json).
    /// Transports: http_json.
    pub async fn create_guardrail(
        &self,
        request: CreateGuardrailParams,
    ) -> PlatformResult<CreateGuardrailResult> {
        let path = String::from("/guardrails");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /guardrails/assignments/keys` — `listKeyAssignments` (json).
    /// Transports: http_json.
    pub async fn list_key_assignments(
        &self,
        request: ListKeyAssignmentsParams,
    ) -> PlatformResult<ListKeyAssignmentsResult> {
        let path = String::from("/guardrails/assignments/keys");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /guardrails/assignments/members` — `listMemberAssignments` (json).
    /// Transports: http_json.
    pub async fn list_member_assignments(
        &self,
        request: ListMemberAssignmentsParams,
    ) -> PlatformResult<ListMemberAssignmentsResult> {
        let path = String::from("/guardrails/assignments/members");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /guardrails/{id}` — `deleteGuardrail` (json).
    /// Transports: http_json.
    pub async fn delete_guardrail(
        &self,
        request: DeleteGuardrailParams,
    ) -> PlatformResult<DeleteGuardrailResult> {
        let mut path = String::from("/guardrails/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /guardrails/{id}` — `getGuardrail` (json).
    /// Transports: http_json.
    pub async fn get_guardrail(
        &self,
        request: GetGuardrailParams,
    ) -> PlatformResult<GetGuardrailResult> {
        let mut path = String::from("/guardrails/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `PATCH /guardrails/{id}` — `updateGuardrail` (json).
    /// Transports: http_json.
    pub async fn update_guardrail(
        &self,
        request: UpdateGuardrailParams,
    ) -> PlatformResult<UpdateGuardrailResult> {
        let mut path = String::from("/guardrails/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /guardrails/{id}/assignments/keys` — `listGuardrailKeyAssignments` (json).
    /// Transports: http_json.
    pub async fn list_guardrail_key_assignments(
        &self,
        request: ListGuardrailKeyAssignmentsParams,
    ) -> PlatformResult<ListGuardrailKeyAssignmentsResult> {
        let mut path = String::from("/guardrails/{id}/assignments/keys");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /guardrails/{id}/assignments/keys` — `bulkAssignKeysToGuardrail` (json).
    /// Transports: http_json.
    pub async fn bulk_assign_keys_to_guardrail(
        &self,
        request: BulkAssignKeysToGuardrailParams,
    ) -> PlatformResult<BulkAssignKeysToGuardrailResult> {
        let mut path = String::from("/guardrails/{id}/assignments/keys");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /guardrails/{id}/assignments/keys/remove` — `bulkUnassignKeysFromGuardrail` (json).
    /// Transports: http_json.
    pub async fn bulk_unassign_keys_from_guardrail(
        &self,
        request: BulkUnassignKeysFromGuardrailParams,
    ) -> PlatformResult<BulkUnassignKeysFromGuardrailResult> {
        let mut path = String::from("/guardrails/{id}/assignments/keys/remove");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /guardrails/{id}/assignments/members` — `listGuardrailMemberAssignments` (json).
    /// Transports: http_json.
    pub async fn list_guardrail_member_assignments(
        &self,
        request: ListGuardrailMemberAssignmentsParams,
    ) -> PlatformResult<ListGuardrailMemberAssignmentsResult> {
        let mut path = String::from("/guardrails/{id}/assignments/members");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /guardrails/{id}/assignments/members` — `bulkAssignMembersToGuardrail` (json).
    /// Transports: http_json.
    pub async fn bulk_assign_members_to_guardrail(
        &self,
        request: BulkAssignMembersToGuardrailParams,
    ) -> PlatformResult<BulkAssignMembersToGuardrailResult> {
        let mut path = String::from("/guardrails/{id}/assignments/members");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /guardrails/{id}/assignments/members/remove` — `bulkUnassignMembersFromGuardrail` (json).
    /// Transports: http_json.
    pub async fn bulk_unassign_members_from_guardrail(
        &self,
        request: BulkUnassignMembersFromGuardrailParams,
    ) -> PlatformResult<BulkUnassignMembersFromGuardrailResult> {
        let mut path = String::from("/guardrails/{id}/assignments/members/remove");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /images` — `createImages` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_images(
        &self,
        request: CreateImagesParams,
    ) -> PlatformResult<CreateImagesResult> {
        let path = String::from("/images");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createImages",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /images` — `createImages` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_images_stream(
        &self,
        request: CreateImagesParams,
    ) -> PlatformResult<CreateImagesSseResult> {
        let path = String::from("/images");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateImagesSseResult { events })
    }

    /// `GET /images/models` — `listImageModels` (json).
    /// Transports: http_json.
    pub async fn list_image_models(
        &self,
        _request: ListImageModelsParams,
    ) -> PlatformResult<ListImageModelsResult> {
        let path = String::from("/images/models");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /images/models/{author}/{slug}/endpoints` — `listImageModelEndpoints` (json).
    /// Transports: http_json.
    pub async fn list_image_model_endpoints(
        &self,
        request: ListImageModelEndpointsParams,
    ) -> PlatformResult<ListImageModelEndpointsResult> {
        let mut path = String::from("/images/models/{author}/{slug}/endpoints");
        path = path.replace(
            "{author}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.author),
        );
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /key` — `getCurrentKey` (json).
    /// Transports: http_json.
    pub async fn get_current_key(
        &self,
        _request: GetCurrentKeyParams,
    ) -> PlatformResult<GetCurrentKeyResult> {
        let path = String::from("/key");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /keys` — `list` (json).
    /// Transports: http_json.
    pub async fn list(&self, request: ListParams) -> PlatformResult<ListResult> {
        let path = String::from("/keys");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include_disabled.as_ref() {
            query.insert("include_disabled".into(), query_value(v));
        }
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /keys` — `createKeys` (json).
    /// Transports: http_json.
    pub async fn create_keys(&self, request: CreateKeysParams) -> PlatformResult<CreateKeysResult> {
        let path = String::from("/keys");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /keys/{hash}` — `deleteKeys` (json).
    /// Transports: http_json.
    pub async fn delete_keys(&self, request: DeleteKeysParams) -> PlatformResult<DeleteKeysResult> {
        let mut path = String::from("/keys/{hash}");
        path = path.replace(
            "{hash}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.hash),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /keys/{hash}` — `getKey` (json).
    /// Transports: http_json.
    pub async fn get_key(&self, request: GetKeyParams) -> PlatformResult<GetKeyResult> {
        let mut path = String::from("/keys/{hash}");
        path = path.replace(
            "{hash}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.hash),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `PATCH /keys/{hash}` — `updateKeys` (json).
    /// Transports: http_json.
    pub async fn update_keys(&self, request: UpdateKeysParams) -> PlatformResult<UpdateKeysResult> {
        let mut path = String::from("/keys/{hash}");
        path = path.replace(
            "{hash}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.hash),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /messages` — `createMessages` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_messages(
        &self,
        request: CreateMessagesParams,
    ) -> PlatformResult<CreateMessagesResult> {
        let path = String::from("/messages");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createMessages",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /messages` — `createMessages` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_messages_stream(
        &self,
        request: CreateMessagesParams,
    ) -> PlatformResult<CreateMessagesSseResult> {
        let path = String::from("/messages");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateMessagesSseResult { events })
    }

    /// `GET /model/{author}/{slug}` — `getModel` (json).
    /// Transports: http_json.
    pub async fn get_model(&self, request: GetModelParams) -> PlatformResult<GetModelResult> {
        let mut path = String::from("/model/{author}/{slug}");
        path = path.replace(
            "{author}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.author),
        );
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /models` — `getModels` (json).
    /// Transports: http_json.
    pub async fn get_models(&self, request: GetModelsParams) -> PlatformResult<GetModelsResult> {
        let path = String::from("/models");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.category.as_ref() {
            query.insert("category".into(), query_value(v));
        }
        if let Some(v) = request.supported_parameters.as_ref() {
            query.insert("supported_parameters".into(), query_value(v));
        }
        if let Some(v) = request.output_modalities.as_ref() {
            query.insert("output_modalities".into(), query_value(v));
        }
        if let Some(v) = request.sort.as_ref() {
            query.insert("sort".into(), query_value(v));
        }
        if let Some(v) = request.q.as_ref() {
            query.insert("q".into(), query_value(v));
        }
        if let Some(v) = request.input_modalities.as_ref() {
            query.insert("input_modalities".into(), query_value(v));
        }
        if let Some(v) = request.context.as_ref() {
            query.insert("context".into(), query_value(v));
        }
        if let Some(v) = request.min_price.as_ref() {
            query.insert("min_price".into(), query_value(v));
        }
        if let Some(v) = request.max_price.as_ref() {
            query.insert("max_price".into(), query_value(v));
        }
        if let Some(v) = request.arch.as_ref() {
            query.insert("arch".into(), query_value(v));
        }
        if let Some(v) = request.model_authors.as_ref() {
            query.insert("model_authors".into(), query_value(v));
        }
        if let Some(v) = request.providers.as_ref() {
            query.insert("providers".into(), query_value(v));
        }
        if let Some(v) = request.distillable.as_ref() {
            query.insert("distillable".into(), query_value(v));
        }
        if let Some(v) = request.zdr.as_ref() {
            query.insert("zdr".into(), query_value(v));
        }
        if let Some(v) = request.region.as_ref() {
            query.insert("region".into(), query_value(v));
        }
        if let Some(v) = request.min_output_price.as_ref() {
            query.insert("min_output_price".into(), query_value(v));
        }
        if let Some(v) = request.max_output_price.as_ref() {
            query.insert("max_output_price".into(), query_value(v));
        }
        if let Some(v) = request.min_age_days.as_ref() {
            query.insert("min_age_days".into(), query_value(v));
        }
        if let Some(v) = request.max_age_days.as_ref() {
            query.insert("max_age_days".into(), query_value(v));
        }
        if let Some(v) = request.min_intelligence_index.as_ref() {
            query.insert("min_intelligence_index".into(), query_value(v));
        }
        if let Some(v) = request.max_intelligence_index.as_ref() {
            query.insert("max_intelligence_index".into(), query_value(v));
        }
        if let Some(v) = request.min_coding_index.as_ref() {
            query.insert("min_coding_index".into(), query_value(v));
        }
        if let Some(v) = request.max_coding_index.as_ref() {
            query.insert("max_coding_index".into(), query_value(v));
        }
        if let Some(v) = request.min_agentic_index.as_ref() {
            query.insert("min_agentic_index".into(), query_value(v));
        }
        if let Some(v) = request.max_agentic_index.as_ref() {
            query.insert("max_agentic_index".into(), query_value(v));
        }
        if let Some(v) = request.min_tool_success_rate.as_ref() {
            query.insert("min_tool_success_rate".into(), query_value(v));
        }
        if let Some(v) = request.max_tool_success_rate.as_ref() {
            query.insert("max_tool_success_rate".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /models/count` — `listModelsCount` (json).
    /// Transports: http_json.
    pub async fn list_models_count(
        &self,
        request: ListModelsCountParams,
    ) -> PlatformResult<ListModelsCountResult> {
        let path = String::from("/models/count");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.output_modalities.as_ref() {
            query.insert("output_modalities".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /models/user` — `listModelsUser` (json).
    /// Transports: http_json.
    pub async fn list_models_user(
        &self,
        request: ListModelsUserParams,
    ) -> PlatformResult<ListModelsUserResult> {
        let path = String::from("/models/user");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /models/{author}/{slug}/endpoints` — `listEndpoints` (json).
    /// Transports: http_json.
    pub async fn list_endpoints(
        &self,
        request: ListEndpointsParams,
    ) -> PlatformResult<ListEndpointsResult> {
        let mut path = String::from("/models/{author}/{slug}/endpoints");
        path = path.replace(
            "{author}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.author),
        );
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /observability/destinations` — `listObservabilityDestinations` (json).
    /// Transports: http_json.
    pub async fn list_observability_destinations(
        &self,
        request: ListObservabilityDestinationsParams,
    ) -> PlatformResult<ListObservabilityDestinationsResult> {
        let path = String::from("/observability/destinations");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.workspace_id.as_ref() {
            query.insert("workspace_id".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /observability/destinations` — `createObservabilityDestination` (json).
    /// Transports: http_json.
    pub async fn create_observability_destination(
        &self,
        request: CreateObservabilityDestinationParams,
    ) -> PlatformResult<CreateObservabilityDestinationResult> {
        let path = String::from("/observability/destinations");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /observability/destinations/{id}` — `deleteObservabilityDestination` (json).
    /// Transports: http_json.
    pub async fn delete_observability_destination(
        &self,
        request: DeleteObservabilityDestinationParams,
    ) -> PlatformResult<DeleteObservabilityDestinationResult> {
        let mut path = String::from("/observability/destinations/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /observability/destinations/{id}` — `getObservabilityDestination` (json).
    /// Transports: http_json.
    pub async fn get_observability_destination(
        &self,
        request: GetObservabilityDestinationParams,
    ) -> PlatformResult<GetObservabilityDestinationResult> {
        let mut path = String::from("/observability/destinations/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `PATCH /observability/destinations/{id}` — `updateObservabilityDestination` (json).
    /// Transports: http_json.
    pub async fn update_observability_destination(
        &self,
        request: UpdateObservabilityDestinationParams,
    ) -> PlatformResult<UpdateObservabilityDestinationResult> {
        let mut path = String::from("/observability/destinations/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/members` — `listOrganizationMembers` (json).
    /// Transports: http_json.
    pub async fn list_organization_members(
        &self,
        request: ListOrganizationMembersParams,
    ) -> PlatformResult<ListOrganizationMembersResult> {
        let path = String::from("/organization/members");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /presets` — `listPresets` (json).
    /// Transports: http_json.
    pub async fn list_presets(
        &self,
        request: ListPresetsParams,
    ) -> PlatformResult<ListPresetsResult> {
        let path = String::from("/presets");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /presets/{slug}` — `getPreset` (json).
    /// Transports: http_json.
    pub async fn get_preset(&self, request: GetPresetParams) -> PlatformResult<GetPresetResult> {
        let mut path = String::from("/presets/{slug}");
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /presets/{slug}/chat/completions` — `createPresetsChatCompletions` (json).
    /// Transports: http_json.
    pub async fn create_presets_chat_completions(
        &self,
        request: CreatePresetsChatCompletionsParams,
    ) -> PlatformResult<CreatePresetsChatCompletionsResult> {
        let mut path = String::from("/presets/{slug}/chat/completions");
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createPresetsChatCompletions",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /presets/{slug}/messages` — `createPresetsMessages` (json).
    /// Transports: http_json.
    pub async fn create_presets_messages(
        &self,
        request: CreatePresetsMessagesParams,
    ) -> PlatformResult<CreatePresetsMessagesResult> {
        let mut path = String::from("/presets/{slug}/messages");
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createPresetsMessages",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /presets/{slug}/responses` — `createPresetsResponses` (json).
    /// Transports: http_json.
    pub async fn create_presets_responses(
        &self,
        request: CreatePresetsResponsesParams,
    ) -> PlatformResult<CreatePresetsResponsesResult> {
        let mut path = String::from("/presets/{slug}/responses");
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createPresetsResponses",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /presets/{slug}/versions` — `listPresetVersions` (json).
    /// Transports: http_json.
    pub async fn list_preset_versions(
        &self,
        request: ListPresetVersionsParams,
    ) -> PlatformResult<ListPresetVersionsResult> {
        let mut path = String::from("/presets/{slug}/versions");
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /presets/{slug}/versions/{version}` — `getPresetVersion` (json).
    /// Transports: http_json.
    pub async fn get_preset_version(
        &self,
        request: GetPresetVersionParams,
    ) -> PlatformResult<GetPresetVersionResult> {
        let mut path = String::from("/presets/{slug}/versions/{version}");
        path = path.replace(
            "{slug}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.slug),
        );
        path = path.replace(
            "{version}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.version),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /providers` — `listProviders` (json).
    /// Transports: http_json.
    pub async fn list_providers(
        &self,
        _request: ListProvidersParams,
    ) -> PlatformResult<ListProvidersResult> {
        let path = String::from("/providers");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /rerank` — `createRerank` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_rerank(
        &self,
        request: CreateRerankParams,
    ) -> PlatformResult<CreateRerankResult> {
        let path = String::from("/rerank");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createRerank",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /rerank` — `createRerank` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_rerank_stream(
        &self,
        request: CreateRerankParams,
    ) -> PlatformResult<CreateRerankSseResult> {
        let path = String::from("/rerank");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateRerankSseResult { events })
    }

    /// `POST /responses` — `createResponses` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_responses(
        &self,
        request: CreateResponsesParams,
    ) -> PlatformResult<CreateResponsesResult> {
        let path = String::from("/responses");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "createResponses",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses` — `createResponses` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_responses_stream(
        &self,
        request: CreateResponsesParams,
    ) -> PlatformResult<CreateResponsesSseResult> {
        let path = String::from("/responses");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateResponsesSseResult { events })
    }

    /// `POST /videos` — `createVideos` (json).
    /// Transports: http_json.
    pub async fn create_videos(
        &self,
        request: CreateVideosParams,
    ) -> PlatformResult<CreateVideosResult> {
        let path = String::from("/videos");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /videos/models` — `listVideosModels` (json).
    /// Transports: http_json.
    pub async fn list_videos_models(
        &self,
        _request: ListVideosModelsParams,
    ) -> PlatformResult<ListVideosModelsResult> {
        let path = String::from("/videos/models");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /videos/{jobId}` — `getVideos` (json).
    /// Transports: http_json.
    pub async fn get_videos(&self, request: GetVideosParams) -> PlatformResult<GetVideosResult> {
        let mut path = String::from("/videos/{jobId}");
        path = path.replace(
            "{jobId}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.job_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /videos/{jobId}/content` — `listVideosContent` (binary).
    /// Transports: http_binary, http_json.
    pub async fn list_videos_content(
        &self,
        request: ListVideosContentParams,
        sink: Option<&std::path::Path>,
    ) -> PlatformResult<ListVideosContentResult> {
        let mut path = String::from("/videos/{jobId}/content");
        path = path.replace(
            "{jobId}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.job_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.index.as_ref() {
            query.insert("index".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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
        let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;
        Ok(ListVideosContentResult {
            bytes,
            content_type,
        })
    }

    /// `GET /workspaces` — `listWorkspaces` (json).
    /// Transports: http_json.
    pub async fn list_workspaces(
        &self,
        request: ListWorkspacesParams,
    ) -> PlatformResult<ListWorkspacesResult> {
        let path = String::from("/workspaces");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /workspaces` — `createWorkspace` (json).
    /// Transports: http_json.
    pub async fn create_workspace(
        &self,
        request: CreateWorkspaceParams,
    ) -> PlatformResult<CreateWorkspaceResult> {
        let path = String::from("/workspaces");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /workspaces/{id}` — `deleteWorkspace` (json).
    /// Transports: http_json.
    pub async fn delete_workspace(
        &self,
        request: DeleteWorkspaceParams,
    ) -> PlatformResult<DeleteWorkspaceResult> {
        let mut path = String::from("/workspaces/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /workspaces/{id}` — `getWorkspace` (json).
    /// Transports: http_json.
    pub async fn get_workspace(
        &self,
        request: GetWorkspaceParams,
    ) -> PlatformResult<GetWorkspaceResult> {
        let mut path = String::from("/workspaces/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `PATCH /workspaces/{id}` — `updateWorkspace` (json).
    /// Transports: http_json.
    pub async fn update_workspace(
        &self,
        request: UpdateWorkspaceParams,
    ) -> PlatformResult<UpdateWorkspaceResult> {
        let mut path = String::from("/workspaces/{id}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /workspaces/{id}/budgets` — `listWorkspaceBudgets` (json).
    /// Transports: http_json.
    pub async fn list_workspace_budgets(
        &self,
        request: ListWorkspaceBudgetsParams,
    ) -> PlatformResult<ListWorkspaceBudgetsResult> {
        let mut path = String::from("/workspaces/{id}/budgets");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /workspaces/{id}/budgets/{interval}` — `deleteWorkspaceBudget` (json).
    /// Transports: http_json.
    pub async fn delete_workspace_budget(
        &self,
        request: DeleteWorkspaceBudgetParams,
    ) -> PlatformResult<DeleteWorkspaceBudgetResult> {
        let mut path = String::from("/workspaces/{id}/budgets/{interval}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        path = path.replace(
            "{interval}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.interval),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `PUT /workspaces/{id}/budgets/{interval}` — `upsertWorkspaceBudget` (json).
    /// Transports: http_json.
    pub async fn upsert_workspace_budget(
        &self,
        request: UpsertWorkspaceBudgetParams,
    ) -> PlatformResult<UpsertWorkspaceBudgetResult> {
        let mut path = String::from("/workspaces/{id}/budgets/{interval}");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        path = path.replace(
            "{interval}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.interval),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /workspaces/{id}/members` — `listWorkspaceMembers` (json).
    /// Transports: http_json.
    pub async fn list_workspace_members(
        &self,
        request: ListWorkspaceMembersParams,
    ) -> PlatformResult<ListWorkspaceMembersResult> {
        let mut path = String::from("/workspaces/{id}/members");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.offset.as_ref() {
            query.insert("offset".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /workspaces/{id}/members/add` — `bulkAddWorkspaceMembers` (json).
    /// Transports: http_json.
    pub async fn bulk_add_workspace_members(
        &self,
        request: BulkAddWorkspaceMembersParams,
    ) -> PlatformResult<BulkAddWorkspaceMembersResult> {
        let mut path = String::from("/workspaces/{id}/members/add");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /workspaces/{id}/members/remove` — `bulkRemoveWorkspaceMembers` (json).
    /// Transports: http_json.
    pub async fn bulk_remove_workspace_members(
        &self,
        request: BulkRemoveWorkspaceMembersParams,
    ) -> PlatformResult<BulkRemoveWorkspaceMembersResult> {
        let mut path = String::from("/workspaces/{id}/members/remove");
        path = path.replace(
            "{id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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
