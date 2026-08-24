//! Generated typed operations for openai.
//! DO NOT EDIT BY HAND.

use super::super::error::{PlatformError, PlatformResult};
use super::super::transport::{CredentialKind, HttpRequestSpec, MultipartFiles};
use super::openai_types::*;
use std::collections::BTreeMap;

fn query_value<T: serde::Serialize + ?Sized>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) if !other.is_null() => other.to_string(),
        _ => String::new(),
    }
}

impl crate::openai_platform::client::OpenAiClient {
    /// `GET /assistants` — `listAssistants` (json).
    /// Transports: http_json.
    pub async fn list_assistants(
        &self,
        request: ListAssistantsParams,
    ) -> PlatformResult<ListAssistantsResult> {
        let path = String::from("/assistants");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
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
            operation_id: "listAssistants",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /assistants` — `createAssistant` (json).
    /// Transports: http_json.
    pub async fn create_assistant(
        &self,
        request: CreateAssistantParams,
    ) -> PlatformResult<CreateAssistantResult> {
        let path = String::from("/assistants");
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
            operation_id: "createAssistant",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /assistants/{assistant_id}` — `getAssistant` (json).
    /// Transports: http_json.
    pub async fn get_assistant(
        &self,
        request: GetAssistantParams,
    ) -> PlatformResult<GetAssistantResult> {
        let mut path = String::from("/assistants/{assistant_id}");
        path = path.replace(
            "{assistant_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.assistant_id),
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
            operation_id: "getAssistant",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /assistants/{assistant_id}` — `modifyAssistant` (json).
    /// Transports: http_json.
    pub async fn modify_assistant(
        &self,
        request: ModifyAssistantParams,
    ) -> PlatformResult<ModifyAssistantResult> {
        let mut path = String::from("/assistants/{assistant_id}");
        path = path.replace(
            "{assistant_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.assistant_id),
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
            operation_id: "modifyAssistant",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /assistants/{assistant_id}` — `deleteAssistant` (json).
    /// Transports: http_json.
    pub async fn delete_assistant(
        &self,
        request: DeleteAssistantParams,
    ) -> PlatformResult<DeleteAssistantResult> {
        let mut path = String::from("/assistants/{assistant_id}");
        path = path.replace(
            "{assistant_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.assistant_id),
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
            operation_id: "deleteAssistant",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/speech` — `createSpeech` (binary).
    /// Transports: http_binary, http_json, http_sse.
    pub async fn create_speech(
        &self,
        request: CreateSpeechParams,
        sink: Option<&std::path::Path>,
    ) -> PlatformResult<CreateSpeechResult> {
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
            operation_id: "createSpeech",
            idempotent: false,
        };
        let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;
        Ok(CreateSpeechResult {
            bytes,
            content_type,
        })
    }

    /// `POST /audio/speech` — `createSpeech` (sse).
    /// Transports: http_binary, http_json, http_sse.
    pub async fn create_speech_stream(
        &self,
        request: CreateSpeechParams,
    ) -> PlatformResult<CreateSpeechSseResult> {
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
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createSpeech",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateSpeechSseResult { events })
    }

    /// `POST /audio/transcriptions` — `createTranscription` (multipart).
    /// Transports: http_json, http_multipart, http_sse.
    pub async fn create_transcription(
        &self,
        request: CreateTranscriptionParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateTranscriptionResult> {
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
            operation_id: "createTranscription",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/transcriptions` — `createTranscription` (sse).
    /// Transports: http_json, http_multipart, http_sse.
    pub async fn create_transcription_stream(
        &self,
        request: CreateTranscriptionParams,
    ) -> PlatformResult<CreateTranscriptionSseResult> {
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
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createTranscription",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateTranscriptionSseResult { events })
    }

    /// `POST /audio/translations` — `createTranslation` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_translation(
        &self,
        request: CreateTranslationParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateTranslationResult> {
        let path = String::from("/audio/translations");
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
            operation_id: "createTranslation",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/voice_consents` — `createVoiceConsent` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_voice_consent(
        &self,
        request: CreateVoiceConsentParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateVoiceConsentResult> {
        let path = String::from("/audio/voice_consents");
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
            operation_id: "createVoiceConsent",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /audio/voice_consents` — `listVoiceConsents` (json).
    /// Transports: http_json.
    pub async fn list_voice_consents(
        &self,
        request: ListVoiceConsentsParams,
    ) -> PlatformResult<ListVoiceConsentsResult> {
        let path = String::from("/audio/voice_consents");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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
            operation_id: "listVoiceConsents",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /audio/voice_consents/{consent_id}` — `getVoiceConsent` (json).
    /// Transports: http_json.
    pub async fn get_voice_consent(
        &self,
        request: GetVoiceConsentParams,
    ) -> PlatformResult<GetVoiceConsentResult> {
        let mut path = String::from("/audio/voice_consents/{consent_id}");
        path = path.replace(
            "{consent_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.consent_id),
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
            operation_id: "getVoiceConsent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/voice_consents/{consent_id}` — `updateVoiceConsent` (json).
    /// Transports: http_json.
    pub async fn update_voice_consent(
        &self,
        request: UpdateVoiceConsentParams,
    ) -> PlatformResult<UpdateVoiceConsentResult> {
        let mut path = String::from("/audio/voice_consents/{consent_id}");
        path = path.replace(
            "{consent_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.consent_id),
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
            operation_id: "updateVoiceConsent",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /audio/voice_consents/{consent_id}` — `deleteVoiceConsent` (json).
    /// Transports: http_json.
    pub async fn delete_voice_consent(
        &self,
        request: DeleteVoiceConsentParams,
    ) -> PlatformResult<DeleteVoiceConsentResult> {
        let mut path = String::from("/audio/voice_consents/{consent_id}");
        path = path.replace(
            "{consent_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.consent_id),
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
            operation_id: "deleteVoiceConsent",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /audio/voices` — `createVoice` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_voice(
        &self,
        request: CreateVoiceParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateVoiceResult> {
        let path = String::from("/audio/voices");
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
            operation_id: "createVoice",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /batches` — `createBatch` (json).
    /// Transports: http_json.
    pub async fn create_batch(
        &self,
        request: CreateBatchParams,
    ) -> PlatformResult<CreateBatchResult> {
        let path = String::from("/batches");
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
            operation_id: "createBatch",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /batches` — `listBatches` (json).
    /// Transports: http_json.
    pub async fn list_batches(
        &self,
        request: ListBatchesParams,
    ) -> PlatformResult<ListBatchesResult> {
        let path = String::from("/batches");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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
            operation_id: "listBatches",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /batches/{batch_id}` — `retrieveBatch` (json).
    /// Transports: http_json.
    pub async fn retrieve_batch(
        &self,
        request: RetrieveBatchParams,
    ) -> PlatformResult<RetrieveBatchResult> {
        let mut path = String::from("/batches/{batch_id}");
        path = path.replace(
            "{batch_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id),
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
            operation_id: "retrieveBatch",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /batches/{batch_id}/cancel` — `cancelBatch` (json).
    /// Transports: http_json, unknown.
    pub async fn cancel_batch(
        &self,
        request: CancelBatchParams,
    ) -> PlatformResult<CancelBatchResult> {
        let mut path = String::from("/batches/{batch_id}/cancel");
        path = path.replace(
            "{batch_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id),
        );
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
            operation_id: "cancelBatch",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chat/completions` — `listChatCompletions` (json).
    /// Transports: http_json.
    pub async fn list_chat_completions(
        &self,
        request: ListChatCompletionsParams,
    ) -> PlatformResult<ListChatCompletionsResult> {
        let path = String::from("/chat/completions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.model.as_ref() {
            query.insert("model".into(), query_value(v));
        }
        if let Some(v) = request.metadata.as_ref() {
            query.insert("metadata".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
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
            operation_id: "listChatCompletions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chat/completions` — `createChatCompletion` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_chat_completion(
        &self,
        request: CreateChatCompletionParams,
    ) -> PlatformResult<CreateChatCompletionResult> {
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
            operation_id: "createChatCompletion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chat/completions` — `createChatCompletion` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_chat_completion_stream(
        &self,
        request: CreateChatCompletionParams,
    ) -> PlatformResult<CreateChatCompletionSseResult> {
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
            operation_id: "createChatCompletion",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateChatCompletionSseResult { events })
    }

    /// `GET /chat/completions/{completion_id}` — `getChatCompletion` (json).
    /// Transports: http_json.
    pub async fn get_chat_completion(
        &self,
        request: GetChatCompletionParams,
    ) -> PlatformResult<GetChatCompletionResult> {
        let mut path = String::from("/chat/completions/{completion_id}");
        path = path.replace(
            "{completion_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.completion_id),
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
            operation_id: "getChatCompletion",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chat/completions/{completion_id}` — `updateChatCompletion` (json).
    /// Transports: http_json.
    pub async fn update_chat_completion(
        &self,
        request: UpdateChatCompletionParams,
    ) -> PlatformResult<UpdateChatCompletionResult> {
        let mut path = String::from("/chat/completions/{completion_id}");
        path = path.replace(
            "{completion_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.completion_id),
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
            operation_id: "updateChatCompletion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /chat/completions/{completion_id}` — `deleteChatCompletion` (json).
    /// Transports: http_json.
    pub async fn delete_chat_completion(
        &self,
        request: DeleteChatCompletionParams,
    ) -> PlatformResult<DeleteChatCompletionResult> {
        let mut path = String::from("/chat/completions/{completion_id}");
        path = path.replace(
            "{completion_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.completion_id),
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
            operation_id: "deleteChatCompletion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chat/completions/{completion_id}/messages` — `getChatCompletionMessages` (json).
    /// Transports: http_json.
    pub async fn get_chat_completion_messages(
        &self,
        request: GetChatCompletionMessagesParams,
    ) -> PlatformResult<GetChatCompletionMessagesResult> {
        let mut path = String::from("/chat/completions/{completion_id}/messages");
        path = path.replace(
            "{completion_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.completion_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
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
            operation_id: "getChatCompletionMessages",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chatkit/sessions` — `CreateChatSessionMethod` (json).
    /// Transports: http_json.
    pub async fn create_chat_session_method(
        &self,
        request: CreateChatSessionMethodParams,
    ) -> PlatformResult<CreateChatSessionMethodResult> {
        let path = String::from("/chatkit/sessions");
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
            operation_id: "CreateChatSessionMethod",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /chatkit/sessions/{session_id}/cancel` — `CancelChatSessionMethod` (json).
    /// Transports: http_json, unknown.
    pub async fn cancel_chat_session_method(
        &self,
        request: CancelChatSessionMethodParams,
    ) -> PlatformResult<CancelChatSessionMethodResult> {
        let mut path = String::from("/chatkit/sessions/{session_id}/cancel");
        path = path.replace(
            "{session_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.session_id),
        );
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
            operation_id: "CancelChatSessionMethod",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chatkit/threads` — `ListThreadsMethod` (json).
    /// Transports: http_json.
    pub async fn list_threads_method(
        &self,
        request: ListThreadsMethodParams,
    ) -> PlatformResult<ListThreadsMethodResult> {
        let path = String::from("/chatkit/threads");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        if let Some(v) = request.user.as_ref() {
            query.insert("user".into(), query_value(v));
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
            operation_id: "ListThreadsMethod",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chatkit/threads/{thread_id}` — `GetThreadMethod` (json).
    /// Transports: http_json.
    pub async fn get_thread_method(
        &self,
        request: GetThreadMethodParams,
    ) -> PlatformResult<GetThreadMethodResult> {
        let mut path = String::from("/chatkit/threads/{thread_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
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
            operation_id: "GetThreadMethod",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /chatkit/threads/{thread_id}` — `DeleteThreadMethod` (json).
    /// Transports: http_json.
    pub async fn delete_thread_method(
        &self,
        request: DeleteThreadMethodParams,
    ) -> PlatformResult<DeleteThreadMethodResult> {
        let mut path = String::from("/chatkit/threads/{thread_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
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
            operation_id: "DeleteThreadMethod",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /chatkit/threads/{thread_id}/items` — `ListThreadItemsMethod` (json).
    /// Transports: http_json.
    pub async fn list_thread_items_method(
        &self,
        request: ListThreadItemsMethodParams,
    ) -> PlatformResult<ListThreadItemsMethodResult> {
        let mut path = String::from("/chatkit/threads/{thread_id}/items");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
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
            operation_id: "ListThreadItemsMethod",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /completions` — `createCompletion` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_completion(
        &self,
        request: CreateCompletionParams,
    ) -> PlatformResult<CreateCompletionResult> {
        let path = String::from("/completions");
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
            operation_id: "createCompletion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /completions` — `createCompletion` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_completion_stream(
        &self,
        request: CreateCompletionParams,
    ) -> PlatformResult<CreateCompletionSseResult> {
        let path = String::from("/completions");
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
            operation_id: "createCompletion",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateCompletionSseResult { events })
    }

    /// `GET /containers` — `ListContainers` (json).
    /// Transports: http_json.
    pub async fn list_containers(
        &self,
        request: ListContainersParams,
    ) -> PlatformResult<ListContainersResult> {
        let path = String::from("/containers");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.name.as_ref() {
            query.insert("name".into(), query_value(v));
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
            operation_id: "ListContainers",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /containers` — `CreateContainer` (json).
    /// Transports: http_json.
    pub async fn create_container(
        &self,
        request: CreateContainerParams,
    ) -> PlatformResult<CreateContainerResult> {
        let path = String::from("/containers");
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
            operation_id: "CreateContainer",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers/{container_id}` — `RetrieveContainer` (json).
    /// Transports: http_json.
    pub async fn retrieve_container(
        &self,
        request: RetrieveContainerParams,
    ) -> PlatformResult<RetrieveContainerResult> {
        let mut path = String::from("/containers/{container_id}");
        path = path.replace(
            "{container_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.container_id),
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
            operation_id: "RetrieveContainer",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /containers/{container_id}` — `DeleteContainer` (json).
    /// Transports: http_json.
    pub async fn delete_container(
        &self,
        request: DeleteContainerParams,
    ) -> PlatformResult<DeleteContainerResult> {
        let mut path = String::from("/containers/{container_id}");
        path = path.replace(
            "{container_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.container_id),
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
            operation_id: "DeleteContainer",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /containers/{container_id}/files` — `CreateContainerFile` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_container_file(
        &self,
        request: CreateContainerFileParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateContainerFileResult> {
        let mut path = String::from("/containers/{container_id}/files");
        path = path.replace(
            "{container_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.container_id),
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
            multipart: true,
            operation_id: "CreateContainerFile",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers/{container_id}/files` — `ListContainerFiles` (json).
    /// Transports: http_json.
    pub async fn list_container_files(
        &self,
        request: ListContainerFilesParams,
    ) -> PlatformResult<ListContainerFilesResult> {
        let mut path = String::from("/containers/{container_id}/files");
        path = path.replace(
            "{container_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.container_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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
            operation_id: "ListContainerFiles",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers/{container_id}/files/{file_id}` — `RetrieveContainerFile` (json).
    /// Transports: http_json.
    pub async fn retrieve_container_file(
        &self,
        request: RetrieveContainerFileParams,
    ) -> PlatformResult<RetrieveContainerFileResult> {
        let mut path = String::from("/containers/{container_id}/files/{file_id}");
        path = path.replace(
            "{container_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.container_id),
        );
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "RetrieveContainerFile",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /containers/{container_id}/files/{file_id}` — `DeleteContainerFile` (json).
    /// Transports: http_json.
    pub async fn delete_container_file(
        &self,
        request: DeleteContainerFileParams,
    ) -> PlatformResult<DeleteContainerFileResult> {
        let mut path = String::from("/containers/{container_id}/files/{file_id}");
        path = path.replace(
            "{container_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.container_id),
        );
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "DeleteContainerFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /containers/{container_id}/files/{file_id}/content` — `RetrieveContainerFileContent` (json).
    /// Transports: http_json.
    pub async fn retrieve_container_file_content(
        &self,
        request: RetrieveContainerFileContentParams,
    ) -> PlatformResult<RetrieveContainerFileContentResult> {
        let mut path = String::from("/containers/{container_id}/files/{file_id}/content");
        path = path.replace(
            "{container_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.container_id),
        );
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "RetrieveContainerFileContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /conversations` — `createConversation` (json).
    /// Transports: http_json.
    pub async fn create_conversation(
        &self,
        request: CreateConversationParams,
    ) -> PlatformResult<CreateConversationResult> {
        let path = String::from("/conversations");
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
            operation_id: "createConversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /conversations/{conversation_id}` — `getConversation` (json).
    /// Transports: http_json.
    pub async fn get_conversation(
        &self,
        request: GetConversationParams,
    ) -> PlatformResult<GetConversationResult> {
        let mut path = String::from("/conversations/{conversation_id}");
        path = path.replace(
            "{conversation_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id),
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
            operation_id: "getConversation",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /conversations/{conversation_id}` — `deleteConversation` (json).
    /// Transports: http_json.
    pub async fn delete_conversation(
        &self,
        request: DeleteConversationParams,
    ) -> PlatformResult<DeleteConversationResult> {
        let mut path = String::from("/conversations/{conversation_id}");
        path = path.replace(
            "{conversation_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id),
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
            operation_id: "deleteConversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /conversations/{conversation_id}` — `updateConversation` (json).
    /// Transports: http_json.
    pub async fn update_conversation(
        &self,
        request: UpdateConversationParams,
    ) -> PlatformResult<UpdateConversationResult> {
        let mut path = String::from("/conversations/{conversation_id}");
        path = path.replace(
            "{conversation_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id),
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
            operation_id: "updateConversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /conversations/{conversation_id}/items` — `createConversationItems` (json).
    /// Transports: http_json.
    pub async fn create_conversation_items(
        &self,
        request: CreateConversationItemsParams,
    ) -> PlatformResult<CreateConversationItemsResult> {
        let mut path = String::from("/conversations/{conversation_id}/items");
        path = path.replace(
            "{conversation_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include.as_ref() {
            query.insert("include".into(), query_value(v));
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
            multipart: false,
            operation_id: "createConversationItems",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /conversations/{conversation_id}/items` — `listConversationItems` (json).
    /// Transports: http_json.
    pub async fn list_conversation_items(
        &self,
        request: ListConversationItemsParams,
    ) -> PlatformResult<ListConversationItemsResult> {
        let mut path = String::from("/conversations/{conversation_id}/items");
        path = path.replace(
            "{conversation_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.include.as_ref() {
            query.insert("include".into(), query_value(v));
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
            operation_id: "listConversationItems",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /conversations/{conversation_id}/items/{item_id}` — `getConversationItem` (json).
    /// Transports: http_json.
    pub async fn get_conversation_item(
        &self,
        request: GetConversationItemParams,
    ) -> PlatformResult<GetConversationItemResult> {
        let mut path = String::from("/conversations/{conversation_id}/items/{item_id}");
        path = path.replace(
            "{conversation_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id),
        );
        path = path.replace(
            "{item_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.item_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include.as_ref() {
            query.insert("include".into(), query_value(v));
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
            operation_id: "getConversationItem",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /conversations/{conversation_id}/items/{item_id}` — `deleteConversationItem` (json).
    /// Transports: http_json.
    pub async fn delete_conversation_item(
        &self,
        request: DeleteConversationItemParams,
    ) -> PlatformResult<DeleteConversationItemResult> {
        let mut path = String::from("/conversations/{conversation_id}/items/{item_id}");
        path = path.replace(
            "{conversation_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.conversation_id),
        );
        path = path.replace(
            "{item_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.item_id),
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
            operation_id: "deleteConversationItem",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /embeddings` — `createEmbedding` (json).
    /// Transports: http_json.
    pub async fn create_embedding(
        &self,
        request: CreateEmbeddingParams,
    ) -> PlatformResult<CreateEmbeddingResult> {
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
            operation_id: "createEmbedding",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals` — `listEvals` (json).
    /// Transports: http_json.
    pub async fn list_evals(&self, request: ListEvalsParams) -> PlatformResult<ListEvalsResult> {
        let path = String::from("/evals");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.order_by.as_ref() {
            query.insert("order_by".into(), query_value(v));
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
            operation_id: "listEvals",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /evals` — `createEval` (json).
    /// Transports: http_json.
    pub async fn create_eval(&self, request: CreateEvalParams) -> PlatformResult<CreateEvalResult> {
        let path = String::from("/evals");
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
            operation_id: "createEval",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}` — `getEval` (json).
    /// Transports: http_json.
    pub async fn get_eval(&self, request: GetEvalParams) -> PlatformResult<GetEvalResult> {
        let mut path = String::from("/evals/{eval_id}");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
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
            operation_id: "getEval",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /evals/{eval_id}` — `updateEval` (json).
    /// Transports: http_json.
    pub async fn update_eval(&self, request: UpdateEvalParams) -> PlatformResult<UpdateEvalResult> {
        let mut path = String::from("/evals/{eval_id}");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
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
            operation_id: "updateEval",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /evals/{eval_id}` — `deleteEval` (json).
    /// Transports: http_json.
    pub async fn delete_eval(&self, request: DeleteEvalParams) -> PlatformResult<DeleteEvalResult> {
        let mut path = String::from("/evals/{eval_id}");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
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
            operation_id: "deleteEval",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}/runs` — `getEvalRuns` (json).
    /// Transports: http_json.
    pub async fn get_eval_runs(
        &self,
        request: GetEvalRunsParams,
    ) -> PlatformResult<GetEvalRunsResult> {
        let mut path = String::from("/evals/{eval_id}/runs");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.status.as_ref() {
            query.insert("status".into(), query_value(v));
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
            operation_id: "getEvalRuns",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /evals/{eval_id}/runs` — `createEvalRun` (json).
    /// Transports: http_json.
    pub async fn create_eval_run(
        &self,
        request: CreateEvalRunParams,
    ) -> PlatformResult<CreateEvalRunResult> {
        let mut path = String::from("/evals/{eval_id}/runs");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
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
            operation_id: "createEvalRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}/runs/{run_id}` — `getEvalRun` (json).
    /// Transports: http_json.
    pub async fn get_eval_run(
        &self,
        request: GetEvalRunParams,
    ) -> PlatformResult<GetEvalRunResult> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
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
            operation_id: "getEvalRun",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /evals/{eval_id}/runs/{run_id}` — `cancelEvalRun` (json).
    /// Transports: http_json, unknown.
    pub async fn cancel_eval_run(
        &self,
        request: CancelEvalRunParams,
    ) -> PlatformResult<CancelEvalRunResult> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
        );
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
            operation_id: "cancelEvalRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /evals/{eval_id}/runs/{run_id}` — `deleteEvalRun` (json).
    /// Transports: http_json.
    pub async fn delete_eval_run(
        &self,
        request: DeleteEvalRunParams,
    ) -> PlatformResult<DeleteEvalRunResult> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
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
            operation_id: "deleteEvalRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}/runs/{run_id}/output_items` — `getEvalRunOutputItems` (json).
    /// Transports: http_json.
    pub async fn get_eval_run_output_items(
        &self,
        request: GetEvalRunOutputItemsParams,
    ) -> PlatformResult<GetEvalRunOutputItemsResult> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}/output_items");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.status.as_ref() {
            query.insert("status".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
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
            operation_id: "getEvalRunOutputItems",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /evals/{eval_id}/runs/{run_id}/output_items/{output_item_id}` — `getEvalRunOutputItem` (json).
    /// Transports: http_json.
    pub async fn get_eval_run_output_item(
        &self,
        request: GetEvalRunOutputItemParams,
    ) -> PlatformResult<GetEvalRunOutputItemResult> {
        let mut path = String::from("/evals/{eval_id}/runs/{run_id}/output_items/{output_item_id}");
        path = path.replace(
            "{eval_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.eval_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
        );
        path = path.replace(
            "{output_item_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.output_item_id),
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
            operation_id: "getEvalRunOutputItem",
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
        if let Some(v) = request.purpose.as_ref() {
            query.insert("purpose".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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

    /// `POST /files` — `createFile` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_file(
        &self,
        request: CreateFileParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateFileResult> {
        let path = String::from("/files");
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
            operation_id: "createFile",
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
            operation_id: "deleteFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /files/{file_id}` — `retrieveFile` (json).
    /// Transports: http_json.
    pub async fn retrieve_file(
        &self,
        request: RetrieveFileParams,
    ) -> PlatformResult<RetrieveFileResult> {
        let mut path = String::from("/files/{file_id}");
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "retrieveFile",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /files/{file_id}/content` — `downloadFile` (json).
    /// Transports: http_json.
    pub async fn download_file(
        &self,
        request: DownloadFileParams,
    ) -> PlatformResult<DownloadFileResult> {
        let mut path = String::from("/files/{file_id}/content");
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "downloadFile",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/alpha/graders/run` — `runGrader` (json).
    /// Transports: http_json.
    pub async fn run_grader(&self, request: RunGraderParams) -> PlatformResult<RunGraderResult> {
        let path = String::from("/fine_tuning/alpha/graders/run");
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
            operation_id: "runGrader",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/alpha/graders/validate` — `validateGrader` (json).
    /// Transports: http_json.
    pub async fn validate_grader(
        &self,
        request: ValidateGraderParams,
    ) -> PlatformResult<ValidateGraderResult> {
        let path = String::from("/fine_tuning/alpha/graders/validate");
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
            operation_id: "validateGrader",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions` — `listFineTuningCheckpointPermissions` (json).
    /// Transports: http_json.
    pub async fn list_fine_tuning_checkpoint_permissions(
        &self,
        request: ListFineTuningCheckpointPermissionsParams,
    ) -> PlatformResult<ListFineTuningCheckpointPermissionsResult> {
        let mut path =
            String::from("/fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions");
        path = path.replace(
            "{fine_tuned_model_checkpoint}",
            &crate::openai_platform::url_policy::encode_path_segment(
                &request.fine_tuned_model_checkpoint,
            ),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.project_id.as_ref() {
            query.insert("project_id".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
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
            operation_id: "listFineTuningCheckpointPermissions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions` — `createFineTuningCheckpointPermission` (json).
    /// Transports: http_json.
    pub async fn create_fine_tuning_checkpoint_permission(
        &self,
        request: CreateFineTuningCheckpointPermissionParams,
    ) -> PlatformResult<CreateFineTuningCheckpointPermissionResult> {
        let mut path =
            String::from("/fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions");
        path = path.replace(
            "{fine_tuned_model_checkpoint}",
            &crate::openai_platform::url_policy::encode_path_segment(
                &request.fine_tuned_model_checkpoint,
            ),
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
            operation_id: "createFineTuningCheckpointPermission",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions/{permission_id}` — `deleteFineTuningCheckpointPermission` (json).
    /// Transports: http_json.
    pub async fn delete_fine_tuning_checkpoint_permission(
        &self,
        request: DeleteFineTuningCheckpointPermissionParams,
    ) -> PlatformResult<DeleteFineTuningCheckpointPermissionResult> {
        let mut path = String::from(
            "/fine_tuning/checkpoints/{fine_tuned_model_checkpoint}/permissions/{permission_id}",
        );
        path = path.replace(
            "{fine_tuned_model_checkpoint}",
            &crate::openai_platform::url_policy::encode_path_segment(
                &request.fine_tuned_model_checkpoint,
            ),
        );
        path = path.replace(
            "{permission_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.permission_id),
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
            operation_id: "deleteFineTuningCheckpointPermission",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/jobs` — `createFineTuningJob` (json).
    /// Transports: http_json.
    pub async fn create_fine_tuning_job(
        &self,
        request: CreateFineTuningJobParams,
    ) -> PlatformResult<CreateFineTuningJobResult> {
        let path = String::from("/fine_tuning/jobs");
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
            operation_id: "createFineTuningJob",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/jobs` — `listPaginatedFineTuningJobs` (json).
    /// Transports: http_json.
    pub async fn list_paginated_fine_tuning_jobs(
        &self,
        request: ListPaginatedFineTuningJobsParams,
    ) -> PlatformResult<ListPaginatedFineTuningJobsResult> {
        let path = String::from("/fine_tuning/jobs");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.metadata.as_ref() {
            query.insert("metadata".into(), query_value(v));
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
            operation_id: "listPaginatedFineTuningJobs",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/jobs/{fine_tuning_job_id}` — `retrieveFineTuningJob` (json).
    /// Transports: http_json.
    pub async fn retrieve_fine_tuning_job(
        &self,
        request: RetrieveFineTuningJobParams,
    ) -> PlatformResult<RetrieveFineTuningJobResult> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}");
        path = path.replace(
            "{fine_tuning_job_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id),
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
            operation_id: "retrieveFineTuningJob",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/jobs/{fine_tuning_job_id}/cancel` — `cancelFineTuningJob` (json).
    /// Transports: http_json, unknown.
    pub async fn cancel_fine_tuning_job(
        &self,
        request: CancelFineTuningJobParams,
    ) -> PlatformResult<CancelFineTuningJobResult> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/cancel");
        path = path.replace(
            "{fine_tuning_job_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id),
        );
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
            operation_id: "cancelFineTuningJob",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/jobs/{fine_tuning_job_id}/checkpoints` — `listFineTuningJobCheckpoints` (json).
    /// Transports: http_json.
    pub async fn list_fine_tuning_job_checkpoints(
        &self,
        request: ListFineTuningJobCheckpointsParams,
    ) -> PlatformResult<ListFineTuningJobCheckpointsResult> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/checkpoints");
        path = path.replace(
            "{fine_tuning_job_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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
            operation_id: "listFineTuningJobCheckpoints",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /fine_tuning/jobs/{fine_tuning_job_id}/events` — `listFineTuningEvents` (json).
    /// Transports: http_json.
    pub async fn list_fine_tuning_events(
        &self,
        request: ListFineTuningEventsParams,
    ) -> PlatformResult<ListFineTuningEventsResult> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/events");
        path = path.replace(
            "{fine_tuning_job_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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
            operation_id: "listFineTuningEvents",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/jobs/{fine_tuning_job_id}/pause` — `pauseFineTuningJob` (json).
    /// Transports: http_json, unknown.
    pub async fn pause_fine_tuning_job(
        &self,
        request: PauseFineTuningJobParams,
    ) -> PlatformResult<PauseFineTuningJobResult> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/pause");
        path = path.replace(
            "{fine_tuning_job_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id),
        );
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
            operation_id: "pauseFineTuningJob",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /fine_tuning/jobs/{fine_tuning_job_id}/resume` — `resumeFineTuningJob` (json).
    /// Transports: http_json, unknown.
    pub async fn resume_fine_tuning_job(
        &self,
        request: ResumeFineTuningJobParams,
    ) -> PlatformResult<ResumeFineTuningJobResult> {
        let mut path = String::from("/fine_tuning/jobs/{fine_tuning_job_id}/resume");
        path = path.replace(
            "{fine_tuning_job_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.fine_tuning_job_id),
        );
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
            operation_id: "resumeFineTuningJob",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /images/edits` — `createImageEdit` (multipart).
    /// Transports: http_json, http_multipart, http_sse.
    pub async fn create_image_edit(
        &self,
        request: CreateImageEditParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateImageEditResult> {
        let path = String::from("/images/edits");
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
            operation_id: "createImageEdit",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /images/edits` — `createImageEdit` (sse).
    /// Transports: http_json, http_multipart, http_sse.
    pub async fn create_image_edit_stream(
        &self,
        request: CreateImageEditParams,
    ) -> PlatformResult<CreateImageEditSseResult> {
        let path = String::from("/images/edits");
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
            operation_id: "createImageEdit",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateImageEditSseResult { events })
    }

    /// `POST /images/generations` — `createImage` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_image(
        &self,
        request: CreateImageParams,
    ) -> PlatformResult<CreateImageResult> {
        let path = String::from("/images/generations");
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
            operation_id: "createImage",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /images/generations` — `createImage` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_image_stream(
        &self,
        request: CreateImageParams,
    ) -> PlatformResult<CreateImageSseResult> {
        let path = String::from("/images/generations");
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
            operation_id: "createImage",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateImageSseResult { events })
    }

    /// `POST /images/variations` — `createImageVariation` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_image_variation(
        &self,
        request: CreateImageVariationParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateImageVariationResult> {
        let path = String::from("/images/variations");
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
            operation_id: "createImageVariation",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /models` — `listModels` (json).
    /// Transports: http_json.
    pub async fn list_models(
        &self,
        _request: ListModelsParams,
    ) -> PlatformResult<ListModelsResult> {
        let path = String::from("/models");
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
            operation_id: "listModels",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /models/{model}` — `retrieveModel` (json).
    /// Transports: http_json.
    pub async fn retrieve_model(
        &self,
        request: RetrieveModelParams,
    ) -> PlatformResult<RetrieveModelResult> {
        let mut path = String::from("/models/{model}");
        path = path.replace(
            "{model}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.model),
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
            operation_id: "retrieveModel",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /models/{model}` — `deleteModel` (json).
    /// Transports: http_json.
    pub async fn delete_model(
        &self,
        request: DeleteModelParams,
    ) -> PlatformResult<DeleteModelResult> {
        let mut path = String::from("/models/{model}");
        path = path.replace(
            "{model}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.model),
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
            operation_id: "deleteModel",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /moderations` — `createModeration` (json).
    /// Transports: http_json.
    pub async fn create_moderation(
        &self,
        request: CreateModerationParams,
    ) -> PlatformResult<CreateModerationResult> {
        let path = String::from("/moderations");
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
            operation_id: "createModeration",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/groups/{group_id}/roles` — `list-project-group-role-assignments` (json).
    /// Transports: http_json.
    pub async fn list_project_group_role_assignments(
        &self,
        request: ListProjectGroupRoleAssignmentsParams,
    ) -> PlatformResult<ListProjectGroupRoleAssignmentsResult> {
        let mut path = String::from("/projects/{project_id}/groups/{group_id}/roles");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
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
            operation_id: "list-project-group-role-assignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /projects/{project_id}/groups/{group_id}/roles` — `assign-project-group-role` (json).
    /// Transports: http_json.
    pub async fn assign_project_group_role(
        &self,
        request: AssignProjectGroupRoleParams,
    ) -> PlatformResult<AssignProjectGroupRoleResult> {
        let mut path = String::from("/projects/{project_id}/groups/{group_id}/roles");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
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
            operation_id: "assign-project-group-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/groups/{group_id}/roles/{role_id}` — `retrieve-project-group-role` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_group_role(
        &self,
        request: RetrieveProjectGroupRoleParams,
    ) -> PlatformResult<RetrieveProjectGroupRoleResult> {
        let mut path = String::from("/projects/{project_id}/groups/{group_id}/roles/{role_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
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
            operation_id: "retrieve-project-group-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /projects/{project_id}/groups/{group_id}/roles/{role_id}` — `unassign-project-group-role` (json).
    /// Transports: http_json.
    pub async fn unassign_project_group_role(
        &self,
        request: UnassignProjectGroupRoleParams,
    ) -> PlatformResult<UnassignProjectGroupRoleResult> {
        let mut path = String::from("/projects/{project_id}/groups/{group_id}/roles/{role_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
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
            operation_id: "unassign-project-group-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/roles` — `list-project-roles` (json).
    /// Transports: http_json.
    pub async fn list_project_roles(
        &self,
        request: ListProjectRolesParams,
    ) -> PlatformResult<ListProjectRolesResult> {
        let mut path = String::from("/projects/{project_id}/roles");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
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
            operation_id: "list-project-roles",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /projects/{project_id}/roles` — `create-project-role` (json).
    /// Transports: http_json.
    pub async fn create_project_role(
        &self,
        request: CreateProjectRoleParams,
    ) -> PlatformResult<CreateProjectRoleResult> {
        let mut path = String::from("/projects/{project_id}/roles");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
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
            operation_id: "create-project-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/roles/{role_id}` — `retrieve-project-role` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_role(
        &self,
        request: RetrieveProjectRoleParams,
    ) -> PlatformResult<RetrieveProjectRoleResult> {
        let mut path = String::from("/projects/{project_id}/roles/{role_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
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
            operation_id: "retrieve-project-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /projects/{project_id}/roles/{role_id}` — `update-project-role` (json).
    /// Transports: http_json.
    pub async fn update_project_role(
        &self,
        request: UpdateProjectRoleParams,
    ) -> PlatformResult<UpdateProjectRoleResult> {
        let mut path = String::from("/projects/{project_id}/roles/{role_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
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
            operation_id: "update-project-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /projects/{project_id}/roles/{role_id}` — `delete-project-role` (json).
    /// Transports: http_json.
    pub async fn delete_project_role(
        &self,
        request: DeleteProjectRoleParams,
    ) -> PlatformResult<DeleteProjectRoleResult> {
        let mut path = String::from("/projects/{project_id}/roles/{role_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
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
            operation_id: "delete-project-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/users/{user_id}/roles` — `list-project-user-role-assignments` (json).
    /// Transports: http_json.
    pub async fn list_project_user_role_assignments(
        &self,
        request: ListProjectUserRoleAssignmentsParams,
    ) -> PlatformResult<ListProjectUserRoleAssignmentsResult> {
        let mut path = String::from("/projects/{project_id}/users/{user_id}/roles");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
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
            operation_id: "list-project-user-role-assignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /projects/{project_id}/users/{user_id}/roles` — `assign-project-user-role` (json).
    /// Transports: http_json.
    pub async fn assign_project_user_role(
        &self,
        request: AssignProjectUserRoleParams,
    ) -> PlatformResult<AssignProjectUserRoleResult> {
        let mut path = String::from("/projects/{project_id}/users/{user_id}/roles");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
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
            operation_id: "assign-project-user-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /projects/{project_id}/users/{user_id}/roles/{role_id}` — `retrieve-project-user-role` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_user_role(
        &self,
        request: RetrieveProjectUserRoleParams,
    ) -> PlatformResult<RetrieveProjectUserRoleResult> {
        let mut path = String::from("/projects/{project_id}/users/{user_id}/roles/{role_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
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
            operation_id: "retrieve-project-user-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /projects/{project_id}/users/{user_id}/roles/{role_id}` — `unassign-project-user-role` (json).
    /// Transports: http_json.
    pub async fn unassign_project_user_role(
        &self,
        request: UnassignProjectUserRoleParams,
    ) -> PlatformResult<UnassignProjectUserRoleResult> {
        let mut path = String::from("/projects/{project_id}/users/{user_id}/roles/{role_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
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
            operation_id: "unassign-project-user-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls` — `create-realtime-call` (multipart).
    /// Transports: http_multipart.
    pub async fn create_realtime_call(
        &self,
        request: CreateRealtimeCallParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateRealtimeCallResult> {
        let path = String::from("/realtime/calls");
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
            operation_id: "create-realtime-call",
            idempotent: false,
        };
        let files = files.content_type("sdp", "application/sdp");
        let files = files.content_type("session", "application/json");
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls/{call_id}/accept` — `accept-realtime-call` (json).
    /// Transports: http_json.
    pub async fn accept_realtime_call(
        &self,
        request: AcceptRealtimeCallParams,
    ) -> PlatformResult<AcceptRealtimeCallResult> {
        let mut path = String::from("/realtime/calls/{call_id}/accept");
        path = path.replace(
            "{call_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.call_id),
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
            operation_id: "accept-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls/{call_id}/hangup` — `hangup-realtime-call` (json).
    /// Transports: http_json, unknown.
    pub async fn hangup_realtime_call(
        &self,
        request: HangupRealtimeCallParams,
    ) -> PlatformResult<HangupRealtimeCallResult> {
        let mut path = String::from("/realtime/calls/{call_id}/hangup");
        path = path.replace(
            "{call_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.call_id),
        );
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
            operation_id: "hangup-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls/{call_id}/refer` — `refer-realtime-call` (json).
    /// Transports: http_json.
    pub async fn refer_realtime_call(
        &self,
        request: ReferRealtimeCallParams,
    ) -> PlatformResult<ReferRealtimeCallResult> {
        let mut path = String::from("/realtime/calls/{call_id}/refer");
        path = path.replace(
            "{call_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.call_id),
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
            operation_id: "refer-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/calls/{call_id}/reject` — `reject-realtime-call` (json).
    /// Transports: http_json.
    pub async fn reject_realtime_call(
        &self,
        request: RejectRealtimeCallParams,
    ) -> PlatformResult<RejectRealtimeCallResult> {
        let mut path = String::from("/realtime/calls/{call_id}/reject");
        path = path.replace(
            "{call_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.call_id),
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
            operation_id: "reject-realtime-call",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/client_secrets` — `create-realtime-client-secret` (json).
    /// Transports: http_json.
    pub async fn create_realtime_client_secret(
        &self,
        request: CreateRealtimeClientSecretParams,
    ) -> PlatformResult<CreateRealtimeClientSecretResult> {
        let path = String::from("/realtime/client_secrets");
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
            operation_id: "create-realtime-client-secret",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/sessions` — `create-realtime-session` (json).
    /// Transports: http_json.
    pub async fn create_realtime_session(
        &self,
        request: CreateRealtimeSessionParams,
    ) -> PlatformResult<CreateRealtimeSessionResult> {
        let path = String::from("/realtime/sessions");
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
            operation_id: "create-realtime-session",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/transcription_sessions` — `create-realtime-transcription-session` (json).
    /// Transports: http_json.
    pub async fn create_realtime_transcription_session(
        &self,
        request: CreateRealtimeTranscriptionSessionParams,
    ) -> PlatformResult<CreateRealtimeTranscriptionSessionResult> {
        let path = String::from("/realtime/transcription_sessions");
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
            operation_id: "create-realtime-transcription-session",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /realtime/translations/client_secrets` — `create-realtime-translation-client-secret` (json).
    /// Transports: http_json.
    pub async fn create_realtime_translation_client_secret(
        &self,
        request: CreateRealtimeTranslationClientSecretParams,
    ) -> PlatformResult<CreateRealtimeTranslationClientSecretResult> {
        let path = String::from("/realtime/translations/client_secrets");
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
            operation_id: "create-realtime-translation-client-secret",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses` — `createResponse` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_response(
        &self,
        request: CreateResponseParams,
    ) -> PlatformResult<CreateResponseResult> {
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
            operation_id: "createResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses` — `createResponse` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_response_stream(
        &self,
        request: CreateResponseParams,
    ) -> PlatformResult<CreateResponseSseResult> {
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
            operation_id: "createResponse",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateResponseSseResult { events })
    }

    /// `POST /responses/compact` — `Compactconversation` (json).
    /// Transports: http_json.
    pub async fn compactconversation(
        &self,
        request: CompactconversationParams,
    ) -> PlatformResult<CompactconversationResult> {
        let path = String::from("/responses/compact");
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
            operation_id: "Compactconversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/compact?beta=true` — `beta_Compactconversation` (json).
    /// Transports: http_json.
    pub async fn beta_compactconversation(
        &self,
        request: BetaCompactconversationParams,
    ) -> PlatformResult<BetaCompactconversationResult> {
        let path = String::from("/responses/compact?beta=true");
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
            operation_id: "beta_Compactconversation",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/input_tokens` — `Getinputtokencounts` (json).
    /// Transports: http_json.
    pub async fn getinputtokencounts(
        &self,
        request: GetinputtokencountsParams,
    ) -> PlatformResult<GetinputtokencountsResult> {
        let path = String::from("/responses/input_tokens");
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
            operation_id: "Getinputtokencounts",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/input_tokens?beta=true` — `beta_Getinputtokencounts` (json).
    /// Transports: http_json.
    pub async fn beta_getinputtokencounts(
        &self,
        request: BetaGetinputtokencountsParams,
    ) -> PlatformResult<BetaGetinputtokencountsResult> {
        let path = String::from("/responses/input_tokens?beta=true");
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
            operation_id: "beta_Getinputtokencounts",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /responses/{response_id}` — `getResponse` (json).
    /// Transports: http_json.
    pub async fn get_response(
        &self,
        request: GetResponseParams,
    ) -> PlatformResult<GetResponseResult> {
        let mut path = String::from("/responses/{response_id}");
        path = path.replace(
            "{response_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.response_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include.as_ref() {
            query.insert("include".into(), query_value(v));
        }
        if let Some(v) = request.stream.as_ref() {
            query.insert("stream".into(), query_value(v));
        }
        if let Some(v) = request.starting_after.as_ref() {
            query.insert("starting_after".into(), query_value(v));
        }
        if let Some(v) = request.include_obfuscation.as_ref() {
            query.insert("include_obfuscation".into(), query_value(v));
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
            operation_id: "getResponse",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /responses/{response_id}` — `deleteResponse` (json).
    /// Transports: http_json.
    pub async fn delete_response(
        &self,
        request: DeleteResponseParams,
    ) -> PlatformResult<DeleteResponseResult> {
        let mut path = String::from("/responses/{response_id}");
        path = path.replace(
            "{response_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.response_id),
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
            operation_id: "deleteResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/{response_id}/cancel` — `cancelResponse` (json).
    /// Transports: http_json, unknown.
    pub async fn cancel_response(
        &self,
        request: CancelResponseParams,
    ) -> PlatformResult<CancelResponseResult> {
        let mut path = String::from("/responses/{response_id}/cancel");
        path = path.replace(
            "{response_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.response_id),
        );
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
            operation_id: "cancelResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses/{response_id}/cancel?beta=true` — `beta_cancelResponse` (json).
    /// Transports: http_json, unknown.
    pub async fn beta_cancel_response(
        &self,
        request: BetaCancelResponseParams,
    ) -> PlatformResult<BetaCancelResponseResult> {
        let mut path = String::from("/responses/{response_id}/cancel?beta=true");
        path = path.replace(
            "{response_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.response_id),
        );
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
            operation_id: "beta_cancelResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /responses/{response_id}/input_items` — `listInputItems` (json).
    /// Transports: http_json.
    pub async fn list_input_items(
        &self,
        request: ListInputItemsParams,
    ) -> PlatformResult<ListInputItemsResult> {
        let mut path = String::from("/responses/{response_id}/input_items");
        path = path.replace(
            "{response_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.response_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.include.as_ref() {
            query.insert("include".into(), query_value(v));
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
            operation_id: "listInputItems",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /responses/{response_id}/input_items?beta=true` — `beta_listInputItems` (json).
    /// Transports: http_json.
    pub async fn beta_list_input_items(
        &self,
        request: BetaListInputItemsParams,
    ) -> PlatformResult<BetaListInputItemsResult> {
        let mut path = String::from("/responses/{response_id}/input_items?beta=true");
        path = path.replace(
            "{response_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.response_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.include.as_ref() {
            query.insert("include".into(), query_value(v));
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
            operation_id: "beta_listInputItems",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /responses/{response_id}?beta=true` — `beta_getResponse` (json).
    /// Transports: http_json.
    pub async fn beta_get_response(
        &self,
        request: BetaGetResponseParams,
    ) -> PlatformResult<BetaGetResponseResult> {
        let mut path = String::from("/responses/{response_id}?beta=true");
        path = path.replace(
            "{response_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.response_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include.as_ref() {
            query.insert("include".into(), query_value(v));
        }
        if let Some(v) = request.stream.as_ref() {
            query.insert("stream".into(), query_value(v));
        }
        if let Some(v) = request.starting_after.as_ref() {
            query.insert("starting_after".into(), query_value(v));
        }
        if let Some(v) = request.include_obfuscation.as_ref() {
            query.insert("include_obfuscation".into(), query_value(v));
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
            operation_id: "beta_getResponse",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /responses/{response_id}?beta=true` — `beta_deleteResponse` (json).
    /// Transports: http_json.
    pub async fn beta_delete_response(
        &self,
        request: BetaDeleteResponseParams,
    ) -> PlatformResult<BetaDeleteResponseResult> {
        let mut path = String::from("/responses/{response_id}?beta=true");
        path = path.replace(
            "{response_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.response_id),
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
            operation_id: "beta_deleteResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses?beta=true` — `beta_createResponse` (json).
    /// Transports: http_json, http_sse.
    pub async fn beta_create_response(
        &self,
        request: BetaCreateResponseParams,
    ) -> PlatformResult<BetaCreateResponseResult> {
        let path = String::from("/responses?beta=true");
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
            operation_id: "beta_createResponse",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /responses?beta=true` — `beta_createResponse` (sse).
    /// Transports: http_json, http_sse.
    pub async fn beta_create_response_stream(
        &self,
        request: BetaCreateResponseParams,
    ) -> PlatformResult<BetaCreateResponseSseResult> {
        let path = String::from("/responses?beta=true");
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
            operation_id: "beta_createResponse",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(BetaCreateResponseSseResult { events })
    }

    /// `POST /skills` — `CreateSkill` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_skill(
        &self,
        request: CreateSkillParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateSkillResult> {
        let path = String::from("/skills");
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
            operation_id: "CreateSkill",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills` — `ListSkills` (json).
    /// Transports: http_json.
    pub async fn list_skills(&self, request: ListSkillsParams) -> PlatformResult<ListSkillsResult> {
        let path = String::from("/skills");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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
            operation_id: "ListSkills",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /skills/{skill_id}` — `DeleteSkill` (json).
    /// Transports: http_json.
    pub async fn delete_skill(
        &self,
        request: DeleteSkillParams,
    ) -> PlatformResult<DeleteSkillResult> {
        let mut path = String::from("/skills/{skill_id}");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
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
            operation_id: "DeleteSkill",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}` — `GetSkill` (json).
    /// Transports: http_json.
    pub async fn get_skill(&self, request: GetSkillParams) -> PlatformResult<GetSkillResult> {
        let mut path = String::from("/skills/{skill_id}");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
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
            operation_id: "GetSkill",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /skills/{skill_id}` — `UpdateSkillDefaultVersion` (json).
    /// Transports: http_json.
    pub async fn update_skill_default_version(
        &self,
        request: UpdateSkillDefaultVersionParams,
    ) -> PlatformResult<UpdateSkillDefaultVersionResult> {
        let mut path = String::from("/skills/{skill_id}");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
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
            operation_id: "UpdateSkillDefaultVersion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}/content` — `GetSkillContent` (binary).
    /// Transports: http_binary.
    pub async fn get_skill_content(
        &self,
        request: GetSkillContentParams,
        sink: Option<&std::path::Path>,
    ) -> PlatformResult<GetSkillContentResult> {
        let mut path = String::from("/skills/{skill_id}/content");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
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
            expect_binary: true,
            multipart: false,
            operation_id: "GetSkillContent",
            idempotent: true,
        };
        let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;
        Ok(GetSkillContentResult {
            bytes,
            content_type,
        })
    }

    /// `POST /skills/{skill_id}/versions` — `CreateSkillVersion` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_skill_version(
        &self,
        request: CreateSkillVersionParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateSkillVersionResult> {
        let mut path = String::from("/skills/{skill_id}/versions");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
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
            multipart: true,
            operation_id: "CreateSkillVersion",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}/versions` — `ListSkillVersions` (json).
    /// Transports: http_json.
    pub async fn list_skill_versions(
        &self,
        request: ListSkillVersionsParams,
    ) -> PlatformResult<ListSkillVersionsResult> {
        let mut path = String::from("/skills/{skill_id}/versions");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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
            operation_id: "ListSkillVersions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}/versions/{version}` — `GetSkillVersion` (json).
    /// Transports: http_json.
    pub async fn get_skill_version(
        &self,
        request: GetSkillVersionParams,
    ) -> PlatformResult<GetSkillVersionResult> {
        let mut path = String::from("/skills/{skill_id}/versions/{version}");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
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
            operation_id: "GetSkillVersion",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /skills/{skill_id}/versions/{version}` — `DeleteSkillVersion` (json).
    /// Transports: http_json.
    pub async fn delete_skill_version(
        &self,
        request: DeleteSkillVersionParams,
    ) -> PlatformResult<DeleteSkillVersionResult> {
        let mut path = String::from("/skills/{skill_id}/versions/{version}");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
        );
        path = path.replace(
            "{version}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.version),
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
            operation_id: "DeleteSkillVersion",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /skills/{skill_id}/versions/{version}/content` — `GetSkillVersionContent` (binary).
    /// Transports: http_binary.
    pub async fn get_skill_version_content(
        &self,
        request: GetSkillVersionContentParams,
        sink: Option<&std::path::Path>,
    ) -> PlatformResult<GetSkillVersionContentResult> {
        let mut path = String::from("/skills/{skill_id}/versions/{version}/content");
        path = path.replace(
            "{skill_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.skill_id),
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
            expect_binary: true,
            multipart: false,
            operation_id: "GetSkillVersionContent",
            idempotent: true,
        };
        let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;
        Ok(GetSkillVersionContentResult {
            bytes,
            content_type,
        })
    }

    /// `POST /threads` — `createThread` (json).
    /// Transports: http_json.
    pub async fn create_thread(
        &self,
        request: CreateThreadParams,
    ) -> PlatformResult<CreateThreadResult> {
        let path = String::from("/threads");
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
            operation_id: "createThread",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/runs` — `createThreadAndRun` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_thread_and_run(
        &self,
        request: CreateThreadAndRunParams,
    ) -> PlatformResult<CreateThreadAndRunResult> {
        let path = String::from("/threads/runs");
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
            operation_id: "createThreadAndRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/runs` — `createThreadAndRun` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_thread_and_run_stream(
        &self,
        request: CreateThreadAndRunParams,
    ) -> PlatformResult<CreateThreadAndRunSseResult> {
        let path = String::from("/threads/runs");
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
            operation_id: "createThreadAndRun",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateThreadAndRunSseResult { events })
    }

    /// `GET /threads/{thread_id}` — `getThread` (json).
    /// Transports: http_json.
    pub async fn get_thread(&self, request: GetThreadParams) -> PlatformResult<GetThreadResult> {
        let mut path = String::from("/threads/{thread_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
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
            operation_id: "getThread",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}` — `modifyThread` (json).
    /// Transports: http_json.
    pub async fn modify_thread(
        &self,
        request: ModifyThreadParams,
    ) -> PlatformResult<ModifyThreadResult> {
        let mut path = String::from("/threads/{thread_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
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
            operation_id: "modifyThread",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /threads/{thread_id}` — `deleteThread` (json).
    /// Transports: http_json.
    pub async fn delete_thread(
        &self,
        request: DeleteThreadParams,
    ) -> PlatformResult<DeleteThreadResult> {
        let mut path = String::from("/threads/{thread_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
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
            operation_id: "deleteThread",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/messages` — `listMessages` (json).
    /// Transports: http_json.
    pub async fn list_messages(
        &self,
        request: ListMessagesParams,
    ) -> PlatformResult<ListMessagesResult> {
        let mut path = String::from("/threads/{thread_id}/messages");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        if let Some(v) = request.run_id.as_ref() {
            query.insert("run_id".into(), query_value(v));
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
            operation_id: "listMessages",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/messages` — `createMessage` (json).
    /// Transports: http_json.
    pub async fn create_message(
        &self,
        request: CreateMessageParams,
    ) -> PlatformResult<CreateMessageResult> {
        let mut path = String::from("/threads/{thread_id}/messages");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
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
            operation_id: "createMessage",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/messages/{message_id}` — `getMessage` (json).
    /// Transports: http_json.
    pub async fn get_message(&self, request: GetMessageParams) -> PlatformResult<GetMessageResult> {
        let mut path = String::from("/threads/{thread_id}/messages/{message_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{message_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.message_id),
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
            operation_id: "getMessage",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/messages/{message_id}` — `modifyMessage` (json).
    /// Transports: http_json.
    pub async fn modify_message(
        &self,
        request: ModifyMessageParams,
    ) -> PlatformResult<ModifyMessageResult> {
        let mut path = String::from("/threads/{thread_id}/messages/{message_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{message_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.message_id),
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
            operation_id: "modifyMessage",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /threads/{thread_id}/messages/{message_id}` — `deleteMessage` (json).
    /// Transports: http_json.
    pub async fn delete_message(
        &self,
        request: DeleteMessageParams,
    ) -> PlatformResult<DeleteMessageResult> {
        let mut path = String::from("/threads/{thread_id}/messages/{message_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{message_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.message_id),
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
            operation_id: "deleteMessage",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/runs` — `listRuns` (json).
    /// Transports: http_json.
    pub async fn list_runs(&self, request: ListRunsParams) -> PlatformResult<ListRunsResult> {
        let mut path = String::from("/threads/{thread_id}/runs");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
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
            operation_id: "listRuns",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs` — `createRun` (json).
    /// Transports: http_json.
    /// `POST /threads/{thread_id}/runs` — `createRun` (json).
    /// Transports: http_json, http_sse.
    pub async fn create_run(&self, request: CreateRunParams) -> PlatformResult<CreateRunResult> {
        let mut path = String::from("/threads/{thread_id}/runs");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include.as_ref() {
            query.insert("include[]".into(), query_value(v));
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
            multipart: false,
            operation_id: "createRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs` — `createRun` (sse).
    /// Transports: http_json, http_sse.
    pub async fn create_run_stream(
        &self,
        request: CreateRunParams,
    ) -> PlatformResult<CreateRunSseResult> {
        let mut path = String::from("/threads/{thread_id}/runs");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include.as_ref() {
            query.insert("include[]".into(), query_value(v));
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
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "createRun",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(CreateRunSseResult { events })
    }

    /// `GET /threads/{thread_id}/runs/{run_id}` — `getRun` (json).
    /// Transports: http_json.
    pub async fn get_run(&self, request: GetRunParams) -> PlatformResult<GetRunResult> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
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
            operation_id: "getRun",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs/{run_id}` — `modifyRun` (json).
    /// Transports: http_json.
    pub async fn modify_run(&self, request: ModifyRunParams) -> PlatformResult<ModifyRunResult> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
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
            operation_id: "modifyRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs/{run_id}/cancel` — `cancelRun` (json).
    /// Transports: http_json, unknown.
    pub async fn cancel_run(&self, request: CancelRunParams) -> PlatformResult<CancelRunResult> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/cancel");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
        );
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
            operation_id: "cancelRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/runs/{run_id}/steps` — `listRunSteps` (json).
    /// Transports: http_json.
    pub async fn list_run_steps(
        &self,
        request: ListRunStepsParams,
    ) -> PlatformResult<ListRunStepsResult> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/steps");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        if let Some(v) = request.include.as_ref() {
            query.insert("include[]".into(), query_value(v));
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
            operation_id: "listRunSteps",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /threads/{thread_id}/runs/{run_id}/steps/{step_id}` — `getRunStep` (json).
    /// Transports: http_json.
    pub async fn get_run_step(
        &self,
        request: GetRunStepParams,
    ) -> PlatformResult<GetRunStepResult> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/steps/{step_id}");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
        );
        path = path.replace(
            "{step_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.step_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include.as_ref() {
            query.insert("include[]".into(), query_value(v));
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
            operation_id: "getRunStep",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs/{run_id}/submit_tool_outputs` — `submitToolOuputsToRun` (json).
    /// Transports: http_json, http_sse.
    pub async fn submit_tool_ouputs_to_run(
        &self,
        request: SubmitToolOuputsToRunParams,
    ) -> PlatformResult<SubmitToolOuputsToRunResult> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/submit_tool_outputs");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
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
            operation_id: "submitToolOuputsToRun",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /threads/{thread_id}/runs/{run_id}/submit_tool_outputs` — `submitToolOuputsToRun` (sse).
    /// Transports: http_json, http_sse.
    pub async fn submit_tool_ouputs_to_run_stream(
        &self,
        request: SubmitToolOuputsToRunParams,
    ) -> PlatformResult<SubmitToolOuputsToRunSseResult> {
        let mut path = String::from("/threads/{thread_id}/runs/{run_id}/submit_tool_outputs");
        path = path.replace(
            "{thread_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.thread_id),
        );
        path = path.replace(
            "{run_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.run_id),
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
            expect_sse: true,
            expect_binary: false,
            multipart: false,
            operation_id: "submitToolOuputsToRun",
            idempotent: false,
        };
        let events = self.transport.execute_sse(spec).await?;
        Ok(SubmitToolOuputsToRunSseResult { events })
    }

    /// `POST /uploads` — `createUpload` (json).
    /// Transports: http_json.
    pub async fn create_upload(
        &self,
        request: CreateUploadParams,
    ) -> PlatformResult<CreateUploadResult> {
        let path = String::from("/uploads");
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
            operation_id: "createUpload",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /uploads/{upload_id}/cancel` — `cancelUpload` (json).
    /// Transports: http_json, unknown.
    pub async fn cancel_upload(
        &self,
        request: CancelUploadParams,
    ) -> PlatformResult<CancelUploadResult> {
        let mut path = String::from("/uploads/{upload_id}/cancel");
        path = path.replace(
            "{upload_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.upload_id),
        );
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
            operation_id: "cancelUpload",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /uploads/{upload_id}/complete` — `completeUpload` (json).
    /// Transports: http_json.
    pub async fn complete_upload(
        &self,
        request: CompleteUploadParams,
    ) -> PlatformResult<CompleteUploadResult> {
        let mut path = String::from("/uploads/{upload_id}/complete");
        path = path.replace(
            "{upload_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.upload_id),
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
            operation_id: "completeUpload",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /uploads/{upload_id}/parts` — `addUploadPart` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn add_upload_part(
        &self,
        request: AddUploadPartParams,
        files: MultipartFiles,
    ) -> PlatformResult<AddUploadPartResult> {
        let mut path = String::from("/uploads/{upload_id}/parts");
        path = path.replace(
            "{upload_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.upload_id),
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
            multipart: true,
            operation_id: "addUploadPart",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores` — `listVectorStores` (json).
    /// Transports: http_json.
    pub async fn list_vector_stores(
        &self,
        request: ListVectorStoresParams,
    ) -> PlatformResult<ListVectorStoresResult> {
        let path = String::from("/vector_stores");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
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
            operation_id: "listVectorStores",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores` — `createVectorStore` (json).
    /// Transports: http_json.
    pub async fn create_vector_store(
        &self,
        request: CreateVectorStoreParams,
    ) -> PlatformResult<CreateVectorStoreResult> {
        let path = String::from("/vector_stores");
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
            operation_id: "createVectorStore",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}` — `getVectorStore` (json).
    /// Transports: http_json.
    pub async fn get_vector_store(
        &self,
        request: GetVectorStoreParams,
    ) -> PlatformResult<GetVectorStoreResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
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
            operation_id: "getVectorStore",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}` — `modifyVectorStore` (json).
    /// Transports: http_json.
    pub async fn modify_vector_store(
        &self,
        request: ModifyVectorStoreParams,
    ) -> PlatformResult<ModifyVectorStoreResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
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
            operation_id: "modifyVectorStore",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /vector_stores/{vector_store_id}` — `deleteVectorStore` (json).
    /// Transports: http_json.
    pub async fn delete_vector_store(
        &self,
        request: DeleteVectorStoreParams,
    ) -> PlatformResult<DeleteVectorStoreResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
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
            operation_id: "deleteVectorStore",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/file_batches` — `createVectorStoreFileBatch` (json).
    /// Transports: http_json.
    pub async fn create_vector_store_file_batch(
        &self,
        request: CreateVectorStoreFileBatchParams,
    ) -> PlatformResult<CreateVectorStoreFileBatchResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/file_batches");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
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
            operation_id: "createVectorStoreFileBatch",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/file_batches/{batch_id}` — `getVectorStoreFileBatch` (json).
    /// Transports: http_json.
    pub async fn get_vector_store_file_batch(
        &self,
        request: GetVectorStoreFileBatchParams,
    ) -> PlatformResult<GetVectorStoreFileBatchResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/file_batches/{batch_id}");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
        );
        path = path.replace(
            "{batch_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id),
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
            operation_id: "getVectorStoreFileBatch",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/file_batches/{batch_id}/cancel` — `cancelVectorStoreFileBatch` (json).
    /// Transports: http_json, unknown.
    pub async fn cancel_vector_store_file_batch(
        &self,
        request: CancelVectorStoreFileBatchParams,
    ) -> PlatformResult<CancelVectorStoreFileBatchResult> {
        let mut path =
            String::from("/vector_stores/{vector_store_id}/file_batches/{batch_id}/cancel");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
        );
        path = path.replace(
            "{batch_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id),
        );
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
            operation_id: "cancelVectorStoreFileBatch",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/file_batches/{batch_id}/files` — `listFilesInVectorStoreBatch` (json).
    /// Transports: http_json.
    pub async fn list_files_in_vector_store_batch(
        &self,
        request: ListFilesInVectorStoreBatchParams,
    ) -> PlatformResult<ListFilesInVectorStoreBatchResult> {
        let mut path =
            String::from("/vector_stores/{vector_store_id}/file_batches/{batch_id}/files");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
        );
        path = path.replace(
            "{batch_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.batch_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        if let Some(v) = request.filter.as_ref() {
            query.insert("filter".into(), query_value(v));
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
            operation_id: "listFilesInVectorStoreBatch",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/files` — `listVectorStoreFiles` (json).
    /// Transports: http_json.
    pub async fn list_vector_store_files(
        &self,
        request: ListVectorStoreFilesParams,
    ) -> PlatformResult<ListVectorStoreFilesResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        if let Some(v) = request.filter.as_ref() {
            query.insert("filter".into(), query_value(v));
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
            operation_id: "listVectorStoreFiles",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/files` — `createVectorStoreFile` (json).
    /// Transports: http_json.
    pub async fn create_vector_store_file(
        &self,
        request: CreateVectorStoreFileParams,
    ) -> PlatformResult<CreateVectorStoreFileResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
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
            operation_id: "createVectorStoreFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/files/{file_id}` — `getVectorStoreFile` (json).
    /// Transports: http_json.
    pub async fn get_vector_store_file(
        &self,
        request: GetVectorStoreFileParams,
    ) -> PlatformResult<GetVectorStoreFileResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files/{file_id}");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
        );
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "getVectorStoreFile",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /vector_stores/{vector_store_id}/files/{file_id}` — `deleteVectorStoreFile` (json).
    /// Transports: http_json.
    pub async fn delete_vector_store_file(
        &self,
        request: DeleteVectorStoreFileParams,
    ) -> PlatformResult<DeleteVectorStoreFileResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files/{file_id}");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
        );
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "deleteVectorStoreFile",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/files/{file_id}` — `updateVectorStoreFileAttributes` (json).
    /// Transports: http_json.
    pub async fn update_vector_store_file_attributes(
        &self,
        request: UpdateVectorStoreFileAttributesParams,
    ) -> PlatformResult<UpdateVectorStoreFileAttributesResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files/{file_id}");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
        );
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "updateVectorStoreFileAttributes",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /vector_stores/{vector_store_id}/files/{file_id}/content` — `retrieveVectorStoreFileContent` (json).
    /// Transports: http_json.
    pub async fn retrieve_vector_store_file_content(
        &self,
        request: RetrieveVectorStoreFileContentParams,
    ) -> PlatformResult<RetrieveVectorStoreFileContentResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/files/{file_id}/content");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
        );
        path = path.replace(
            "{file_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.file_id),
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
            operation_id: "retrieveVectorStoreFileContent",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /vector_stores/{vector_store_id}/search` — `searchVectorStore` (json).
    /// Transports: http_json.
    pub async fn search_vector_store(
        &self,
        request: SearchVectorStoreParams,
    ) -> PlatformResult<SearchVectorStoreResult> {
        let mut path = String::from("/vector_stores/{vector_store_id}/search");
        path = path.replace(
            "{vector_store_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.vector_store_id),
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
            operation_id: "searchVectorStore",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos` — `createVideo` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_video(
        &self,
        request: CreateVideoParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateVideoResult> {
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
            multipart: true,
            operation_id: "createVideo",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos` — `ListVideos` (json).
    /// Transports: http_json.
    pub async fn list_videos(&self, request: ListVideosParams) -> PlatformResult<ListVideosResult> {
        let path = String::from("/videos");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
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
            operation_id: "ListVideos",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos/characters` — `CreateVideoCharacter` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_video_character(
        &self,
        request: CreateVideoCharacterParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateVideoCharacterResult> {
        let path = String::from("/videos/characters");
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
            operation_id: "CreateVideoCharacter",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/characters/{character_id}` — `GetVideoCharacter` (json).
    /// Transports: http_json.
    pub async fn get_video_character(
        &self,
        request: GetVideoCharacterParams,
    ) -> PlatformResult<GetVideoCharacterResult> {
        let mut path = String::from("/videos/characters/{character_id}");
        path = path.replace(
            "{character_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.character_id),
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
            operation_id: "GetVideoCharacter",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos/edits` — `CreateVideoEdit` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_video_edit(
        &self,
        request: CreateVideoEditParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateVideoEditResult> {
        let path = String::from("/videos/edits");
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
            operation_id: "CreateVideoEdit",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /videos/extensions` — `CreateVideoExtend` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_video_extend(
        &self,
        request: CreateVideoExtendParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateVideoExtendResult> {
        let path = String::from("/videos/extensions");
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
            operation_id: "CreateVideoExtend",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/{video_id}` — `GetVideo` (json).
    /// Transports: http_json.
    pub async fn get_video(&self, request: GetVideoParams) -> PlatformResult<GetVideoResult> {
        let mut path = String::from("/videos/{video_id}");
        path = path.replace(
            "{video_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.video_id),
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
            operation_id: "GetVideo",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /videos/{video_id}` — `DeleteVideo` (json).
    /// Transports: http_json.
    pub async fn delete_video(
        &self,
        request: DeleteVideoParams,
    ) -> PlatformResult<DeleteVideoResult> {
        let mut path = String::from("/videos/{video_id}");
        path = path.replace(
            "{video_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.video_id),
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
            operation_id: "DeleteVideo",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /videos/{video_id}/content` — `RetrieveVideoContent` (binary).
    /// Transports: http_binary, http_json.
    pub async fn retrieve_video_content(
        &self,
        request: RetrieveVideoContentParams,
        sink: Option<&std::path::Path>,
    ) -> PlatformResult<RetrieveVideoContentResult> {
        let mut path = String::from("/videos/{video_id}/content");
        path = path.replace(
            "{video_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.video_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.variant.as_ref() {
            query.insert("variant".into(), query_value(v));
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
            operation_id: "RetrieveVideoContent",
            idempotent: true,
        };
        let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;
        Ok(RetrieveVideoContentResult {
            bytes,
            content_type,
        })
    }

    /// `POST /videos/{video_id}/remix` — `CreateVideoRemix` (multipart).
    /// Transports: http_json, http_multipart.
    pub async fn create_video_remix(
        &self,
        request: CreateVideoRemixParams,
        files: MultipartFiles,
    ) -> PlatformResult<CreateVideoRemixResult> {
        let mut path = String::from("/videos/{video_id}/remix");
        path = path.replace(
            "{video_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.video_id),
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
            multipart: true,
            operation_id: "CreateVideoRemix",
            idempotent: false,
        };
        let raw = self.transport.execute_multipart(spec, files).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }
}
