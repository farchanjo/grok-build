//! Typed CLI dispatch runtime. DO NOT EDIT BY HAND.
//! Source: baselines/scripts/generate_operation_metadata.py
use super::generated_ops::CliOperation;
use super::output::{ExitCode, write_binary, write_json, write_ndjson_line};
use crate::provider_registry::id::ProviderId;
use crate::provider_registry::secrets::{
    admin_key_scope, application_key_scope, read_provider_secret,
};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;
use xai_grok_inference::openai_platform::MultipartFiles;
use xai_grok_inference::openai_platform::generated::{
    openai_admin_types, openai_types, openrouter_types,
};
use xai_grok_inference::{
    OpenAiAdminClient, OpenAiClient, OpenRouterClient, PlatformClientConfig, TransportPolicy,
};

fn merge_params(
    input_json: Option<&str>,
    path_params: &[(String, String)],
    query: &[(String, String)],
) -> Result<Value, String> {
    let mut obj = match input_json {
        Some(s) if !s.trim().is_empty() => {
            let v: Value =
                serde_json::from_str(s).map_err(|e| format!("typed request JSON: {e}"))?;
            match v {
                Value::Object(m) => m,
                other => {
                    let mut m = serde_json::Map::new();
                    m.insert("body".into(), other);
                    m
                }
            }
        }
        _ => serde_json::Map::new(),
    };
    for (k, v) in path_params {
        obj.insert(k.clone(), Value::String(v.clone()));
    }
    for (k, v) in query {
        obj.insert(k.clone(), Value::String(v.clone()));
    }
    Ok(Value::Object(obj))
}
fn decode_params<T: DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| {
        format!(
            "typed Params deserialize ({}): {e}",
            std::any::type_name::<T>()
        )
    })
}
fn multipart_from(files: &[(String, PathBuf)]) -> MultipartFiles {
    let mut m = MultipartFiles::new();
    for (field, path) in files {
        m = m.file(field.clone(), path.clone());
    }
    m
}

pub async fn dispatch_runtime(
    provider: &str,
    op: &CliOperation,
    path_params: &[(String, String)],
    query: &[(String, String)],
    input_json: Option<&str>,
    dry_run: bool,
    stream: bool,
    output: Option<&Path>,
    multipart_files: &[(String, PathBuf)],
) -> Result<ExitCode, String> {
    if !matches!(
        op.method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD"
    ) {
        return Err(format!("unsupported HTTP method {}", op.method));
    }
    // Merge typed params first. Dry-run must return before any credential
    // resolver, env/vault/auth-file access, token construction, or network setup.
    let merged = merge_params(input_json, path_params, query)?;
    if dry_run {
        write_json(&json!({
            "provider": provider,
            "operation_id": op.operation_id,
            "request_type": op.request_type,
            "response_type": op.response_type,
            "client_method": op.client_method,
            "transports": op.transports,
            "credential_class": op.credential_class,
            "requires_confirmation": op.requires_confirmation,
            "typed_request": merged,
            "dry_run": true,
        }))
        .map_err(|e| e.to_string())?;
        return Ok(ExitCode::Success);
    }
    note_live_dispatch_credential_phase();
    let home = xai_grok_config::grok_home();
    let meta = resolve_provider_from_registry(provider, &home)?;
    let pid = ProviderId::new(provider).map_err(|e| e.to_string())?;
    // Credential selection is provider-native and metadata-driven: admin slots
    // never fall back to the application key when admin is missing.
    let want_admin = op.is_admin || op.credential_class == "admin";
    let app_token = if want_admin {
        None
    } else {
        resolve_app_token(provider, &home, &pid, meta.env_key.as_deref())
    };
    let admin_token = resolve_admin_token(provider, &home, &pid, meta.admin_env_key.as_deref());
    if want_admin
        && admin_token
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    {
        return Err(format!(
            "admin credential required for {}::{} (never borrowing application key)",
            op.provider_namespace, op.operation_id
        ));
    }
    let cfg = PlatformClientConfig {
        provider_id: provider.to_owned(),
        display_name: meta.display_name,
        base_url: meta.base_url,
        admin_base_url: meta.admin_base_url,
        application_token: if want_admin { None } else { app_token },
        admin_token,
        extra_headers: meta.extra_headers.into_iter().collect(),
        policy: TransportPolicy::default(),
    };
    match op.provider_namespace {
        "openai" => {
            let client = OpenAiClient::from_config(cfg, CancellationToken::new())
                .map_err(|e| e.to_string())?;
            dispatch_openai(client, op, merged, stream, output, multipart_files).await
        }
        "openai_admin" => {
            let client = OpenAiAdminClient::from_config(cfg, CancellationToken::new())
                .map_err(|e| e.to_string())?;
            dispatch_openai_admin(client, op, merged, stream, output, multipart_files).await
        }
        "openrouter" => {
            let client = OpenRouterClient::from_config(cfg, CancellationToken::new())
                .map_err(|e| e.to_string())?;
            dispatch_openrouter(client, op, merged, stream, output, multipart_files).await
        }
        other => Err(format!("unknown namespace {other}")),
    }
}

/// Test seam: live dispatch increments this before any credential resolution.
/// Dry-run must never call this (proves zero credential-phase entry).
#[cfg(test)]
pub(crate) static LIVE_CREDENTIAL_PHASE_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[inline]
fn note_live_dispatch_credential_phase() {
    #[cfg(test)]
    LIVE_CREDENTIAL_PHASE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

async fn dispatch_openai(
    client: OpenAiClient,
    op: &CliOperation,
    merged: Value,
    stream: bool,
    output: Option<&Path>,
    multipart_files: &[(String, PathBuf)],
) -> Result<ExitCode, String> {
    match op.operation_id {
        "listAssistants" => {
            let req: openai_types::ListAssistantsParams = decode_params(merged)?;
            let resp = client
                .list_assistants(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createAssistant" => {
            let req: openai_types::CreateAssistantParams = decode_params(merged)?;
            let resp = client
                .create_assistant(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteAssistant" => {
            let req: openai_types::DeleteAssistantParams = decode_params(merged)?;
            let resp = client
                .delete_assistant(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getAssistant" => {
            let req: openai_types::GetAssistantParams = decode_params(merged)?;
            let resp = client.get_assistant(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modifyAssistant" => {
            let req: openai_types::ModifyAssistantParams = decode_params(merged)?;
            let resp = client
                .modify_assistant(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createSpeech" => {
            let req: openai_types::CreateSpeechParams = decode_params(merged)?;
            let resp = client
                .create_speech(req, output)
                .await
                .map_err(|e| e.to_string())?;
            if output.is_some() {
                write_json(&json!({"ok": true, "bytes": resp.bytes.len()}))
                    .map_err(|e| e.to_string())?;
            } else {
                return write_binary(&resp.bytes, None).map_err(|e| e.to_string());
            }
            Ok(ExitCode::Success)
        }

        "createSpeech_stream" => {
            let req: openai_types::CreateSpeechParams = decode_params(merged)?;
            let resp = client
                .create_speech_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createTranscription" => {
            let req: openai_types::CreateTranscriptionParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_transcription(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "createTranscription_stream" => {
            let req: openai_types::CreateTranscriptionParams = decode_params(merged)?;
            let resp = client
                .create_transcription_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createTranslation" => {
            let req: openai_types::CreateTranslationParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_translation(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "listVoiceConsents" => {
            let req: openai_types::ListVoiceConsentsParams = decode_params(merged)?;
            let resp = client
                .list_voice_consents(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createVoiceConsent" => {
            let req: openai_types::CreateVoiceConsentParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_voice_consent(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "deleteVoiceConsent" => {
            let req: openai_types::DeleteVoiceConsentParams = decode_params(merged)?;
            let resp = client
                .delete_voice_consent(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getVoiceConsent" => {
            let req: openai_types::GetVoiceConsentParams = decode_params(merged)?;
            let resp = client
                .get_voice_consent(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateVoiceConsent" => {
            let req: openai_types::UpdateVoiceConsentParams = decode_params(merged)?;
            let resp = client
                .update_voice_consent(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createVoice" => {
            let req: openai_types::CreateVoiceParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_voice(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "listBatches" => {
            let req: openai_types::ListBatchesParams = decode_params(merged)?;
            let resp = client.list_batches(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createBatch" => {
            let req: openai_types::CreateBatchParams = decode_params(merged)?;
            let resp = client.create_batch(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieveBatch" => {
            let req: openai_types::RetrieveBatchParams = decode_params(merged)?;
            let resp = client
                .retrieve_batch(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "cancelBatch" => {
            let req: openai_types::CancelBatchParams = decode_params(merged)?;
            let resp = client.cancel_batch(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listChatCompletions" => {
            let req: openai_types::ListChatCompletionsParams = decode_params(merged)?;
            let resp = client
                .list_chat_completions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createChatCompletion" => {
            let req: openai_types::CreateChatCompletionParams = decode_params(merged)?;
            let resp = client
                .create_chat_completion(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createChatCompletion_stream" => {
            let req: openai_types::CreateChatCompletionParams = decode_params(merged)?;
            let resp = client
                .create_chat_completion_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteChatCompletion" => {
            let req: openai_types::DeleteChatCompletionParams = decode_params(merged)?;
            let resp = client
                .delete_chat_completion(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getChatCompletion" => {
            let req: openai_types::GetChatCompletionParams = decode_params(merged)?;
            let resp = client
                .get_chat_completion(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateChatCompletion" => {
            let req: openai_types::UpdateChatCompletionParams = decode_params(merged)?;
            let resp = client
                .update_chat_completion(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getChatCompletionMessages" => {
            let req: openai_types::GetChatCompletionMessagesParams = decode_params(merged)?;
            let resp = client
                .get_chat_completion_messages(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "CreateChatSessionMethod" => {
            let req: openai_types::CreateChatSessionMethodParams = decode_params(merged)?;
            let resp = client
                .create_chat_session_method(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "CancelChatSessionMethod" => {
            let req: openai_types::CancelChatSessionMethodParams = decode_params(merged)?;
            let resp = client
                .cancel_chat_session_method(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "ListThreadsMethod" => {
            let req: openai_types::ListThreadsMethodParams = decode_params(merged)?;
            let resp = client
                .list_threads_method(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "DeleteThreadMethod" => {
            let req: openai_types::DeleteThreadMethodParams = decode_params(merged)?;
            let resp = client
                .delete_thread_method(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "GetThreadMethod" => {
            let req: openai_types::GetThreadMethodParams = decode_params(merged)?;
            let resp = client
                .get_thread_method(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "ListThreadItemsMethod" => {
            let req: openai_types::ListThreadItemsMethodParams = decode_params(merged)?;
            let resp = client
                .list_thread_items_method(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createCompletion" => {
            let req: openai_types::CreateCompletionParams = decode_params(merged)?;
            let resp = client
                .create_completion(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createCompletion_stream" => {
            let req: openai_types::CreateCompletionParams = decode_params(merged)?;
            let resp = client
                .create_completion_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "ListContainers" => {
            let req: openai_types::ListContainersParams = decode_params(merged)?;
            let resp = client
                .list_containers(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "CreateContainer" => {
            let req: openai_types::CreateContainerParams = decode_params(merged)?;
            let resp = client
                .create_container(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "DeleteContainer" => {
            let req: openai_types::DeleteContainerParams = decode_params(merged)?;
            let resp = client
                .delete_container(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "RetrieveContainer" => {
            let req: openai_types::RetrieveContainerParams = decode_params(merged)?;
            let resp = client
                .retrieve_container(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "ListContainerFiles" => {
            let req: openai_types::ListContainerFilesParams = decode_params(merged)?;
            let resp = client
                .list_container_files(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "CreateContainerFile" => {
            let req: openai_types::CreateContainerFileParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_container_file(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "DeleteContainerFile" => {
            let req: openai_types::DeleteContainerFileParams = decode_params(merged)?;
            let resp = client
                .delete_container_file(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "RetrieveContainerFile" => {
            let req: openai_types::RetrieveContainerFileParams = decode_params(merged)?;
            let resp = client
                .retrieve_container_file(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "RetrieveContainerFileContent" => {
            let req: openai_types::RetrieveContainerFileContentParams = decode_params(merged)?;
            let resp = client
                .retrieve_container_file_content(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createConversation" => {
            let req: openai_types::CreateConversationParams = decode_params(merged)?;
            let resp = client
                .create_conversation(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteConversation" => {
            let req: openai_types::DeleteConversationParams = decode_params(merged)?;
            let resp = client
                .delete_conversation(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getConversation" => {
            let req: openai_types::GetConversationParams = decode_params(merged)?;
            let resp = client
                .get_conversation(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateConversation" => {
            let req: openai_types::UpdateConversationParams = decode_params(merged)?;
            let resp = client
                .update_conversation(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listConversationItems" => {
            let req: openai_types::ListConversationItemsParams = decode_params(merged)?;
            let resp = client
                .list_conversation_items(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createConversationItems" => {
            let req: openai_types::CreateConversationItemsParams = decode_params(merged)?;
            let resp = client
                .create_conversation_items(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteConversationItem" => {
            let req: openai_types::DeleteConversationItemParams = decode_params(merged)?;
            let resp = client
                .delete_conversation_item(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getConversationItem" => {
            let req: openai_types::GetConversationItemParams = decode_params(merged)?;
            let resp = client
                .get_conversation_item(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createEmbedding" => {
            let req: openai_types::CreateEmbeddingParams = decode_params(merged)?;
            let resp = client
                .create_embedding(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listEvals" => {
            let req: openai_types::ListEvalsParams = decode_params(merged)?;
            let resp = client.list_evals(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createEval" => {
            let req: openai_types::CreateEvalParams = decode_params(merged)?;
            let resp = client.create_eval(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteEval" => {
            let req: openai_types::DeleteEvalParams = decode_params(merged)?;
            let resp = client.delete_eval(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getEval" => {
            let req: openai_types::GetEvalParams = decode_params(merged)?;
            let resp = client.get_eval(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateEval" => {
            let req: openai_types::UpdateEvalParams = decode_params(merged)?;
            let resp = client.update_eval(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getEvalRuns" => {
            let req: openai_types::GetEvalRunsParams = decode_params(merged)?;
            let resp = client.get_eval_runs(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createEvalRun" => {
            let req: openai_types::CreateEvalRunParams = decode_params(merged)?;
            let resp = client
                .create_eval_run(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteEvalRun" => {
            let req: openai_types::DeleteEvalRunParams = decode_params(merged)?;
            let resp = client
                .delete_eval_run(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getEvalRun" => {
            let req: openai_types::GetEvalRunParams = decode_params(merged)?;
            let resp = client.get_eval_run(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "cancelEvalRun" => {
            let req: openai_types::CancelEvalRunParams = decode_params(merged)?;
            let resp = client
                .cancel_eval_run(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getEvalRunOutputItems" => {
            let req: openai_types::GetEvalRunOutputItemsParams = decode_params(merged)?;
            let resp = client
                .get_eval_run_output_items(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getEvalRunOutputItem" => {
            let req: openai_types::GetEvalRunOutputItemParams = decode_params(merged)?;
            let resp = client
                .get_eval_run_output_item(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listFiles" => {
            let req: openai_types::ListFilesParams = decode_params(merged)?;
            let resp = client.list_files(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createFile" => {
            let req: openai_types::CreateFileParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_file(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "deleteFile" => {
            let req: openai_types::DeleteFileParams = decode_params(merged)?;
            let resp = client.delete_file(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieveFile" => {
            let req: openai_types::RetrieveFileParams = decode_params(merged)?;
            let resp = client.retrieve_file(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "downloadFile" => {
            let req: openai_types::DownloadFileParams = decode_params(merged)?;
            let resp = client.download_file(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "runGrader" => {
            let req: openai_types::RunGraderParams = decode_params(merged)?;
            let resp = client.run_grader(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "validateGrader" => {
            let req: openai_types::ValidateGraderParams = decode_params(merged)?;
            let resp = client
                .validate_grader(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listFineTuningCheckpointPermissions" => {
            let req: openai_types::ListFineTuningCheckpointPermissionsParams =
                decode_params(merged)?;
            let resp = client
                .list_fine_tuning_checkpoint_permissions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createFineTuningCheckpointPermission" => {
            let req: openai_types::CreateFineTuningCheckpointPermissionParams =
                decode_params(merged)?;
            let resp = client
                .create_fine_tuning_checkpoint_permission(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteFineTuningCheckpointPermission" => {
            let req: openai_types::DeleteFineTuningCheckpointPermissionParams =
                decode_params(merged)?;
            let resp = client
                .delete_fine_tuning_checkpoint_permission(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listPaginatedFineTuningJobs" => {
            let req: openai_types::ListPaginatedFineTuningJobsParams = decode_params(merged)?;
            let resp = client
                .list_paginated_fine_tuning_jobs(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createFineTuningJob" => {
            let req: openai_types::CreateFineTuningJobParams = decode_params(merged)?;
            let resp = client
                .create_fine_tuning_job(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieveFineTuningJob" => {
            let req: openai_types::RetrieveFineTuningJobParams = decode_params(merged)?;
            let resp = client
                .retrieve_fine_tuning_job(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "cancelFineTuningJob" => {
            let req: openai_types::CancelFineTuningJobParams = decode_params(merged)?;
            let resp = client
                .cancel_fine_tuning_job(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listFineTuningJobCheckpoints" => {
            let req: openai_types::ListFineTuningJobCheckpointsParams = decode_params(merged)?;
            let resp = client
                .list_fine_tuning_job_checkpoints(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listFineTuningEvents" => {
            let req: openai_types::ListFineTuningEventsParams = decode_params(merged)?;
            let resp = client
                .list_fine_tuning_events(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "pauseFineTuningJob" => {
            let req: openai_types::PauseFineTuningJobParams = decode_params(merged)?;
            let resp = client
                .pause_fine_tuning_job(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "resumeFineTuningJob" => {
            let req: openai_types::ResumeFineTuningJobParams = decode_params(merged)?;
            let resp = client
                .resume_fine_tuning_job(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createImageEdit" => {
            let req: openai_types::CreateImageEditParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_image_edit(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "createImageEdit_stream" => {
            let req: openai_types::CreateImageEditParams = decode_params(merged)?;
            let resp = client
                .create_image_edit_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createImage" => {
            let req: openai_types::CreateImageParams = decode_params(merged)?;
            let resp = client.create_image(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createImage_stream" => {
            let req: openai_types::CreateImageParams = decode_params(merged)?;
            let resp = client
                .create_image_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createImageVariation" => {
            let req: openai_types::CreateImageVariationParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_image_variation(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "listModels" => {
            let req: openai_types::ListModelsParams = decode_params(merged)?;
            let resp = client.list_models(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteModel" => {
            let req: openai_types::DeleteModelParams = decode_params(merged)?;
            let resp = client.delete_model(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieveModel" => {
            let req: openai_types::RetrieveModelParams = decode_params(merged)?;
            let resp = client
                .retrieve_model(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createModeration" => {
            let req: openai_types::CreateModerationParams = decode_params(merged)?;
            let resp = client
                .create_moderation(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-group-role-assignments" => {
            let req: openai_types::ListProjectGroupRoleAssignmentsParams = decode_params(merged)?;
            let resp = client
                .list_project_group_role_assignments(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "assign-project-group-role" => {
            let req: openai_types::AssignProjectGroupRoleParams = decode_params(merged)?;
            let resp = client
                .assign_project_group_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "unassign-project-group-role" => {
            let req: openai_types::UnassignProjectGroupRoleParams = decode_params(merged)?;
            let resp = client
                .unassign_project_group_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-group-role" => {
            let req: openai_types::RetrieveProjectGroupRoleParams = decode_params(merged)?;
            let resp = client
                .retrieve_project_group_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-roles" => {
            let req: openai_types::ListProjectRolesParams = decode_params(merged)?;
            let resp = client
                .list_project_roles(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-project-role" => {
            let req: openai_types::CreateProjectRoleParams = decode_params(merged)?;
            let resp = client
                .create_project_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-project-role" => {
            let req: openai_types::DeleteProjectRoleParams = decode_params(merged)?;
            let resp = client
                .delete_project_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-role" => {
            let req: openai_types::RetrieveProjectRoleParams = decode_params(merged)?;
            let resp = client
                .retrieve_project_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-project-role" => {
            let req: openai_types::UpdateProjectRoleParams = decode_params(merged)?;
            let resp = client
                .update_project_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-user-role-assignments" => {
            let req: openai_types::ListProjectUserRoleAssignmentsParams = decode_params(merged)?;
            let resp = client
                .list_project_user_role_assignments(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "assign-project-user-role" => {
            let req: openai_types::AssignProjectUserRoleParams = decode_params(merged)?;
            let resp = client
                .assign_project_user_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "unassign-project-user-role" => {
            let req: openai_types::UnassignProjectUserRoleParams = decode_params(merged)?;
            let resp = client
                .unassign_project_user_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-user-role" => {
            let req: openai_types::RetrieveProjectUserRoleParams = decode_params(merged)?;
            let resp = client
                .retrieve_project_user_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-realtime-call" => {
            let req: openai_types::CreateRealtimeCallParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_realtime_call(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "accept-realtime-call" => {
            let req: openai_types::AcceptRealtimeCallParams = decode_params(merged)?;
            let resp = client
                .accept_realtime_call(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "hangup-realtime-call" => {
            let req: openai_types::HangupRealtimeCallParams = decode_params(merged)?;
            let resp = client
                .hangup_realtime_call(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "refer-realtime-call" => {
            let req: openai_types::ReferRealtimeCallParams = decode_params(merged)?;
            let resp = client
                .refer_realtime_call(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "reject-realtime-call" => {
            let req: openai_types::RejectRealtimeCallParams = decode_params(merged)?;
            let resp = client
                .reject_realtime_call(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-realtime-client-secret" => {
            let req: openai_types::CreateRealtimeClientSecretParams = decode_params(merged)?;
            let resp = client
                .create_realtime_client_secret(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-realtime-session" => {
            let req: openai_types::CreateRealtimeSessionParams = decode_params(merged)?;
            let resp = client
                .create_realtime_session(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-realtime-transcription-session" => {
            let req: openai_types::CreateRealtimeTranscriptionSessionParams =
                decode_params(merged)?;
            let resp = client
                .create_realtime_transcription_session(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-realtime-translation-client-secret" => {
            let req: openai_types::CreateRealtimeTranslationClientSecretParams =
                decode_params(merged)?;
            let resp = client
                .create_realtime_translation_client_secret(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createResponse" => {
            let req: openai_types::CreateResponseParams = decode_params(merged)?;
            let resp = client
                .create_response(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createResponse_stream" => {
            let req: openai_types::CreateResponseParams = decode_params(merged)?;
            let resp = client
                .create_response_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "Compactconversation" => {
            let req: openai_types::CompactconversationParams = decode_params(merged)?;
            let resp = client
                .compactconversation(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "beta_Compactconversation" => {
            let req: openai_types::BetaCompactconversationParams = decode_params(merged)?;
            let resp = client
                .beta_compactconversation(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "Getinputtokencounts" => {
            let req: openai_types::GetinputtokencountsParams = decode_params(merged)?;
            let resp = client
                .getinputtokencounts(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "beta_Getinputtokencounts" => {
            let req: openai_types::BetaGetinputtokencountsParams = decode_params(merged)?;
            let resp = client
                .beta_getinputtokencounts(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteResponse" => {
            let req: openai_types::DeleteResponseParams = decode_params(merged)?;
            let resp = client
                .delete_response(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getResponse" => {
            let req: openai_types::GetResponseParams = decode_params(merged)?;
            let resp = client.get_response(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "cancelResponse" => {
            let req: openai_types::CancelResponseParams = decode_params(merged)?;
            let resp = client
                .cancel_response(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "beta_cancelResponse" => {
            let req: openai_types::BetaCancelResponseParams = decode_params(merged)?;
            let resp = client
                .beta_cancel_response(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listInputItems" => {
            let req: openai_types::ListInputItemsParams = decode_params(merged)?;
            let resp = client
                .list_input_items(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "beta_listInputItems" => {
            let req: openai_types::BetaListInputItemsParams = decode_params(merged)?;
            let resp = client
                .beta_list_input_items(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "beta_deleteResponse" => {
            let req: openai_types::BetaDeleteResponseParams = decode_params(merged)?;
            let resp = client
                .beta_delete_response(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "beta_getResponse" => {
            let req: openai_types::BetaGetResponseParams = decode_params(merged)?;
            let resp = client
                .beta_get_response(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "beta_createResponse" => {
            let req: openai_types::BetaCreateResponseParams = decode_params(merged)?;
            let resp = client
                .beta_create_response(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "beta_createResponse_stream" => {
            let req: openai_types::BetaCreateResponseParams = decode_params(merged)?;
            let resp = client
                .beta_create_response_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "ListSkills" => {
            let req: openai_types::ListSkillsParams = decode_params(merged)?;
            let resp = client.list_skills(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "CreateSkill" => {
            let req: openai_types::CreateSkillParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_skill(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "DeleteSkill" => {
            let req: openai_types::DeleteSkillParams = decode_params(merged)?;
            let resp = client.delete_skill(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "GetSkill" => {
            let req: openai_types::GetSkillParams = decode_params(merged)?;
            let resp = client.get_skill(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "UpdateSkillDefaultVersion" => {
            let req: openai_types::UpdateSkillDefaultVersionParams = decode_params(merged)?;
            let resp = client
                .update_skill_default_version(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "GetSkillContent" => {
            let req: openai_types::GetSkillContentParams = decode_params(merged)?;
            let resp = client
                .get_skill_content(req, output)
                .await
                .map_err(|e| e.to_string())?;
            if output.is_some() {
                write_json(&json!({"ok": true, "bytes": resp.bytes.len()}))
                    .map_err(|e| e.to_string())?;
            } else {
                return write_binary(&resp.bytes, None).map_err(|e| e.to_string());
            }
            Ok(ExitCode::Success)
        }

        "ListSkillVersions" => {
            let req: openai_types::ListSkillVersionsParams = decode_params(merged)?;
            let resp = client
                .list_skill_versions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "CreateSkillVersion" => {
            let req: openai_types::CreateSkillVersionParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_skill_version(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "DeleteSkillVersion" => {
            let req: openai_types::DeleteSkillVersionParams = decode_params(merged)?;
            let resp = client
                .delete_skill_version(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "GetSkillVersion" => {
            let req: openai_types::GetSkillVersionParams = decode_params(merged)?;
            let resp = client
                .get_skill_version(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "GetSkillVersionContent" => {
            let req: openai_types::GetSkillVersionContentParams = decode_params(merged)?;
            let resp = client
                .get_skill_version_content(req, output)
                .await
                .map_err(|e| e.to_string())?;
            if output.is_some() {
                write_json(&json!({"ok": true, "bytes": resp.bytes.len()}))
                    .map_err(|e| e.to_string())?;
            } else {
                return write_binary(&resp.bytes, None).map_err(|e| e.to_string());
            }
            Ok(ExitCode::Success)
        }

        "createThread" => {
            let req: openai_types::CreateThreadParams = decode_params(merged)?;
            let resp = client.create_thread(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createThreadAndRun" => {
            let req: openai_types::CreateThreadAndRunParams = decode_params(merged)?;
            let resp = client
                .create_thread_and_run(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createThreadAndRun_stream" => {
            let req: openai_types::CreateThreadAndRunParams = decode_params(merged)?;
            let resp = client
                .create_thread_and_run_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteThread" => {
            let req: openai_types::DeleteThreadParams = decode_params(merged)?;
            let resp = client.delete_thread(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getThread" => {
            let req: openai_types::GetThreadParams = decode_params(merged)?;
            let resp = client.get_thread(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modifyThread" => {
            let req: openai_types::ModifyThreadParams = decode_params(merged)?;
            let resp = client.modify_thread(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listMessages" => {
            let req: openai_types::ListMessagesParams = decode_params(merged)?;
            let resp = client.list_messages(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createMessage" => {
            let req: openai_types::CreateMessageParams = decode_params(merged)?;
            let resp = client
                .create_message(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteMessage" => {
            let req: openai_types::DeleteMessageParams = decode_params(merged)?;
            let resp = client
                .delete_message(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getMessage" => {
            let req: openai_types::GetMessageParams = decode_params(merged)?;
            let resp = client.get_message(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modifyMessage" => {
            let req: openai_types::ModifyMessageParams = decode_params(merged)?;
            let resp = client
                .modify_message(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listRuns" => {
            let req: openai_types::ListRunsParams = decode_params(merged)?;
            let resp = client.list_runs(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createRun" => {
            let req: openai_types::CreateRunParams = decode_params(merged)?;
            let resp = client.create_run(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createRun_stream" => {
            let req: openai_types::CreateRunParams = decode_params(merged)?;
            let resp = client
                .create_run_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getRun" => {
            let req: openai_types::GetRunParams = decode_params(merged)?;
            let resp = client.get_run(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modifyRun" => {
            let req: openai_types::ModifyRunParams = decode_params(merged)?;
            let resp = client.modify_run(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "cancelRun" => {
            let req: openai_types::CancelRunParams = decode_params(merged)?;
            let resp = client.cancel_run(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listRunSteps" => {
            let req: openai_types::ListRunStepsParams = decode_params(merged)?;
            let resp = client
                .list_run_steps(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getRunStep" => {
            let req: openai_types::GetRunStepParams = decode_params(merged)?;
            let resp = client.get_run_step(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "submitToolOuputsToRun" => {
            let req: openai_types::SubmitToolOuputsToRunParams = decode_params(merged)?;
            let resp = client
                .submit_tool_ouputs_to_run(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "submitToolOuputsToRun_stream" => {
            let req: openai_types::SubmitToolOuputsToRunParams = decode_params(merged)?;
            let resp = client
                .submit_tool_ouputs_to_run_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createUpload" => {
            let req: openai_types::CreateUploadParams = decode_params(merged)?;
            let resp = client.create_upload(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "cancelUpload" => {
            let req: openai_types::CancelUploadParams = decode_params(merged)?;
            let resp = client.cancel_upload(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "completeUpload" => {
            let req: openai_types::CompleteUploadParams = decode_params(merged)?;
            let resp = client
                .complete_upload(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "addUploadPart" => {
            let req: openai_types::AddUploadPartParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .add_upload_part(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "listVectorStores" => {
            let req: openai_types::ListVectorStoresParams = decode_params(merged)?;
            let resp = client
                .list_vector_stores(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createVectorStore" => {
            let req: openai_types::CreateVectorStoreParams = decode_params(merged)?;
            let resp = client
                .create_vector_store(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteVectorStore" => {
            let req: openai_types::DeleteVectorStoreParams = decode_params(merged)?;
            let resp = client
                .delete_vector_store(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getVectorStore" => {
            let req: openai_types::GetVectorStoreParams = decode_params(merged)?;
            let resp = client
                .get_vector_store(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modifyVectorStore" => {
            let req: openai_types::ModifyVectorStoreParams = decode_params(merged)?;
            let resp = client
                .modify_vector_store(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createVectorStoreFileBatch" => {
            let req: openai_types::CreateVectorStoreFileBatchParams = decode_params(merged)?;
            let resp = client
                .create_vector_store_file_batch(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getVectorStoreFileBatch" => {
            let req: openai_types::GetVectorStoreFileBatchParams = decode_params(merged)?;
            let resp = client
                .get_vector_store_file_batch(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "cancelVectorStoreFileBatch" => {
            let req: openai_types::CancelVectorStoreFileBatchParams = decode_params(merged)?;
            let resp = client
                .cancel_vector_store_file_batch(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listFilesInVectorStoreBatch" => {
            let req: openai_types::ListFilesInVectorStoreBatchParams = decode_params(merged)?;
            let resp = client
                .list_files_in_vector_store_batch(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listVectorStoreFiles" => {
            let req: openai_types::ListVectorStoreFilesParams = decode_params(merged)?;
            let resp = client
                .list_vector_store_files(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createVectorStoreFile" => {
            let req: openai_types::CreateVectorStoreFileParams = decode_params(merged)?;
            let resp = client
                .create_vector_store_file(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteVectorStoreFile" => {
            let req: openai_types::DeleteVectorStoreFileParams = decode_params(merged)?;
            let resp = client
                .delete_vector_store_file(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getVectorStoreFile" => {
            let req: openai_types::GetVectorStoreFileParams = decode_params(merged)?;
            let resp = client
                .get_vector_store_file(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateVectorStoreFileAttributes" => {
            let req: openai_types::UpdateVectorStoreFileAttributesParams = decode_params(merged)?;
            let resp = client
                .update_vector_store_file_attributes(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieveVectorStoreFileContent" => {
            let req: openai_types::RetrieveVectorStoreFileContentParams = decode_params(merged)?;
            let resp = client
                .retrieve_vector_store_file_content(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "searchVectorStore" => {
            let req: openai_types::SearchVectorStoreParams = decode_params(merged)?;
            let resp = client
                .search_vector_store(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "ListVideos" => {
            let req: openai_types::ListVideosParams = decode_params(merged)?;
            let resp = client.list_videos(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createVideo" => {
            let req: openai_types::CreateVideoParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_video(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "CreateVideoCharacter" => {
            let req: openai_types::CreateVideoCharacterParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_video_character(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "GetVideoCharacter" => {
            let req: openai_types::GetVideoCharacterParams = decode_params(merged)?;
            let resp = client
                .get_video_character(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "CreateVideoEdit" => {
            let req: openai_types::CreateVideoEditParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_video_edit(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "CreateVideoExtend" => {
            let req: openai_types::CreateVideoExtendParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_video_extend(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "DeleteVideo" => {
            let req: openai_types::DeleteVideoParams = decode_params(merged)?;
            let resp = client.delete_video(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "GetVideo" => {
            let req: openai_types::GetVideoParams = decode_params(merged)?;
            let resp = client.get_video(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "RetrieveVideoContent" => {
            let req: openai_types::RetrieveVideoContentParams = decode_params(merged)?;
            let resp = client
                .retrieve_video_content(req, output)
                .await
                .map_err(|e| e.to_string())?;
            if output.is_some() {
                write_json(&json!({"ok": true, "bytes": resp.bytes.len()}))
                    .map_err(|e| e.to_string())?;
            } else {
                return write_binary(&resp.bytes, None).map_err(|e| e.to_string());
            }
            Ok(ExitCode::Success)
        }

        "CreateVideoRemix" => {
            let req: openai_types::CreateVideoRemixParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_video_remix(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        other => Err(format!("no typed dispatch arm for {other}")),
    }
}

async fn dispatch_openai_admin(
    client: OpenAiAdminClient,
    op: &CliOperation,
    merged: Value,
    stream: bool,
    output: Option<&Path>,
    multipart_files: &[(String, PathBuf)],
) -> Result<ExitCode, String> {
    match op.operation_id {
        "admin-api-keys-list" => {
            let req: openai_admin_types::AdminApiKeysListParams = decode_params(merged)?;
            let resp = client
                .admin_api_keys_list(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "admin-api-keys-create" => {
            let req: openai_admin_types::AdminApiKeysCreateParams = decode_params(merged)?;
            let resp = client
                .admin_api_keys_create(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "admin-api-keys-delete" => {
            let req: openai_admin_types::AdminApiKeysDeleteParams = decode_params(merged)?;
            let resp = client
                .admin_api_keys_delete(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "admin-api-keys-get" => {
            let req: openai_admin_types::AdminApiKeysGetParams = decode_params(merged)?;
            let resp = client
                .admin_api_keys_get(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-audit-logs" => {
            let req: openai_admin_types::ListAuditLogsParams = decode_params(merged)?;
            let resp = client
                .list_audit_logs(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listOrganizationCertificates" => {
            let req: openai_admin_types::ListOrganizationCertificatesParams =
                decode_params(merged)?;
            let resp = client
                .list_organization_certificates(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "uploadCertificate" => {
            let req: openai_admin_types::UploadCertificateParams = decode_params(merged)?;
            let resp = client
                .upload_certificate(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "activateOrganizationCertificates" => {
            let req: openai_admin_types::ActivateOrganizationCertificatesParams =
                decode_params(merged)?;
            let resp = client
                .activate_organization_certificates(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deactivateOrganizationCertificates" => {
            let req: openai_admin_types::DeactivateOrganizationCertificatesParams =
                decode_params(merged)?;
            let resp = client
                .deactivate_organization_certificates(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteCertificate" => {
            let req: openai_admin_types::DeleteCertificateParams = decode_params(merged)?;
            let resp = client
                .delete_certificate(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getCertificate" => {
            let req: openai_admin_types::GetCertificateParams = decode_params(merged)?;
            let resp = client
                .get_certificate(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modifyCertificate" => {
            let req: openai_admin_types::ModifyCertificateParams = decode_params(merged)?;
            let resp = client
                .modify_certificate(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-costs" => {
            let req: openai_admin_types::UsageCostsParams = decode_params(merged)?;
            let resp = client.usage_costs(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-organization-data-retention" => {
            let req: openai_admin_types::RetrieveOrganizationDataRetentionParams =
                decode_params(merged)?;
            let resp = client
                .retrieve_organization_data_retention(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-organization-data-retention" => {
            let req: openai_admin_types::UpdateOrganizationDataRetentionParams =
                decode_params(merged)?;
            let resp = client
                .update_organization_data_retention(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-groups" => {
            let req: openai_admin_types::ListGroupsParams = decode_params(merged)?;
            let resp = client.list_groups(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-group" => {
            let req: openai_admin_types::CreateGroupParams = decode_params(merged)?;
            let resp = client.create_group(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-group" => {
            let req: openai_admin_types::DeleteGroupParams = decode_params(merged)?;
            let resp = client.delete_group(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-group" => {
            let req: openai_admin_types::RetrieveGroupParams = decode_params(merged)?;
            let resp = client
                .retrieve_group(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-group" => {
            let req: openai_admin_types::UpdateGroupParams = decode_params(merged)?;
            let resp = client.update_group(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-group-role-assignments" => {
            let req: openai_admin_types::ListGroupRoleAssignmentsParams = decode_params(merged)?;
            let resp = client
                .list_group_role_assignments(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "assign-group-role" => {
            let req: openai_admin_types::AssignGroupRoleParams = decode_params(merged)?;
            let resp = client
                .assign_group_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "unassign-group-role" => {
            let req: openai_admin_types::UnassignGroupRoleParams = decode_params(merged)?;
            let resp = client
                .unassign_group_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-group-role" => {
            let req: openai_admin_types::RetrieveGroupRoleParams = decode_params(merged)?;
            let resp = client
                .retrieve_group_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-group-users" => {
            let req: openai_admin_types::ListGroupUsersParams = decode_params(merged)?;
            let resp = client
                .list_group_users(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "add-group-user" => {
            let req: openai_admin_types::AddGroupUserParams = decode_params(merged)?;
            let resp = client
                .add_group_user(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "remove-group-user" => {
            let req: openai_admin_types::RemoveGroupUserParams = decode_params(merged)?;
            let resp = client
                .remove_group_user(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-group-user" => {
            let req: openai_admin_types::RetrieveGroupUserParams = decode_params(merged)?;
            let resp = client
                .retrieve_group_user(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-invites" => {
            let req: openai_admin_types::ListInvitesParams = decode_params(merged)?;
            let resp = client.list_invites(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "inviteUser" => {
            let req: openai_admin_types::InviteUserParams = decode_params(merged)?;
            let resp = client.invite_user(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-invite" => {
            let req: openai_admin_types::DeleteInviteParams = decode_params(merged)?;
            let resp = client.delete_invite(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-invite" => {
            let req: openai_admin_types::RetrieveInviteParams = decode_params(merged)?;
            let resp = client
                .retrieve_invite(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-projects" => {
            let req: openai_admin_types::ListProjectsParams = decode_params(merged)?;
            let resp = client.list_projects(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-project" => {
            let req: openai_admin_types::CreateProjectParams = decode_params(merged)?;
            let resp = client
                .create_project(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project" => {
            let req: openai_admin_types::RetrieveProjectParams = decode_params(merged)?;
            let resp = client
                .retrieve_project(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modify-project" => {
            let req: openai_admin_types::ModifyProjectParams = decode_params(merged)?;
            let resp = client
                .modify_project(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-api-keys" => {
            let req: openai_admin_types::ListProjectApiKeysParams = decode_params(merged)?;
            let resp = client
                .list_project_api_keys(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-project-api-key" => {
            let req: openai_admin_types::DeleteProjectApiKeyParams = decode_params(merged)?;
            let resp = client
                .delete_project_api_key(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-api-key" => {
            let req: openai_admin_types::RetrieveProjectApiKeyParams = decode_params(merged)?;
            let resp = client
                .retrieve_project_api_key(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "archive-project" => {
            let req: openai_admin_types::ArchiveProjectParams = decode_params(merged)?;
            let resp = client
                .archive_project(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listProjectCertificates" => {
            let req: openai_admin_types::ListProjectCertificatesParams = decode_params(merged)?;
            let resp = client
                .list_project_certificates(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "activateProjectCertificates" => {
            let req: openai_admin_types::ActivateProjectCertificatesParams = decode_params(merged)?;
            let resp = client
                .activate_project_certificates(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deactivateProjectCertificates" => {
            let req: openai_admin_types::DeactivateProjectCertificatesParams =
                decode_params(merged)?;
            let resp = client
                .deactivate_project_certificates(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-data-retention" => {
            let req: openai_admin_types::RetrieveProjectDataRetentionParams =
                decode_params(merged)?;
            let resp = client
                .retrieve_project_data_retention(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-project-data-retention" => {
            let req: openai_admin_types::UpdateProjectDataRetentionParams = decode_params(merged)?;
            let resp = client
                .update_project_data_retention(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-groups" => {
            let req: openai_admin_types::ListProjectGroupsParams = decode_params(merged)?;
            let resp = client
                .list_project_groups(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "add-project-group" => {
            let req: openai_admin_types::AddProjectGroupParams = decode_params(merged)?;
            let resp = client
                .add_project_group(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "remove-project-group" => {
            let req: openai_admin_types::RemoveProjectGroupParams = decode_params(merged)?;
            let resp = client
                .remove_project_group(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-group" => {
            let req: openai_admin_types::RetrieveProjectGroupParams = decode_params(merged)?;
            let resp = client
                .retrieve_project_group(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-hosted-tool-permissions" => {
            let req: openai_admin_types::RetrieveProjectHostedToolPermissionsParams =
                decode_params(merged)?;
            let resp = client
                .retrieve_project_hosted_tool_permissions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-project-hosted-tool-permissions" => {
            let req: openai_admin_types::UpdateProjectHostedToolPermissionsParams =
                decode_params(merged)?;
            let resp = client
                .update_project_hosted_tool_permissions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-project-model-permissions" => {
            let req: openai_admin_types::DeleteProjectModelPermissionsParams =
                decode_params(merged)?;
            let resp = client
                .delete_project_model_permissions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-model-permissions" => {
            let req: openai_admin_types::RetrieveProjectModelPermissionsParams =
                decode_params(merged)?;
            let resp = client
                .retrieve_project_model_permissions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-project-model-permissions" => {
            let req: openai_admin_types::UpdateProjectModelPermissionsParams =
                decode_params(merged)?;
            let resp = client
                .update_project_model_permissions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-rate-limits" => {
            let req: openai_admin_types::ListProjectRateLimitsParams = decode_params(merged)?;
            let resp = client
                .list_project_rate_limits(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-project-rate-limits" => {
            let req: openai_admin_types::UpdateProjectRateLimitsParams = decode_params(merged)?;
            let resp = client
                .update_project_rate_limits(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-service-accounts" => {
            let req: openai_admin_types::ListProjectServiceAccountsParams = decode_params(merged)?;
            let resp = client
                .list_project_service_accounts(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-project-service-account" => {
            let req: openai_admin_types::CreateProjectServiceAccountParams = decode_params(merged)?;
            let resp = client
                .create_project_service_account(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-project-service-account" => {
            let req: openai_admin_types::DeleteProjectServiceAccountParams = decode_params(merged)?;
            let resp = client
                .delete_project_service_account(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-service-account" => {
            let req: openai_admin_types::RetrieveProjectServiceAccountParams =
                decode_params(merged)?;
            let resp = client
                .retrieve_project_service_account(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-project-service-account" => {
            let req: openai_admin_types::UpdateProjectServiceAccountParams = decode_params(merged)?;
            let resp = client
                .update_project_service_account(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "CreateanAPIkeyforaserviceaccount" => {
            let req: openai_admin_types::CreateanAPIkeyforaserviceaccountParams =
                decode_params(merged)?;
            let resp = client
                .createan_ap_ikeyforaserviceaccount(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-spend-alerts" => {
            let req: openai_admin_types::ListProjectSpendAlertsParams = decode_params(merged)?;
            let resp = client
                .list_project_spend_alerts(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-project-spend-alert" => {
            let req: openai_admin_types::CreateProjectSpendAlertParams = decode_params(merged)?;
            let resp = client
                .create_project_spend_alert(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-project-spend-alert" => {
            let req: openai_admin_types::DeleteProjectSpendAlertParams = decode_params(merged)?;
            let resp = client
                .delete_project_spend_alert(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-spend-alert" => {
            let req: openai_admin_types::RetrieveProjectSpendAlertParams = decode_params(merged)?;
            let resp = client
                .retrieve_project_spend_alert(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-project-spend-alert" => {
            let req: openai_admin_types::UpdateProjectSpendAlertParams = decode_params(merged)?;
            let resp = client
                .update_project_spend_alert(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "Deleteprojectspendlimit" => {
            let req: openai_admin_types::DeleteprojectspendlimitParams = decode_params(merged)?;
            let resp = client
                .deleteprojectspendlimit(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "Getprojectspendlimit" => {
            let req: openai_admin_types::GetprojectspendlimitParams = decode_params(merged)?;
            let resp = client
                .getprojectspendlimit(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "Updateprojectspendlimit" => {
            let req: openai_admin_types::UpdateprojectspendlimitParams = decode_params(merged)?;
            let resp = client
                .updateprojectspendlimit(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-project-users" => {
            let req: openai_admin_types::ListProjectUsersParams = decode_params(merged)?;
            let resp = client
                .list_project_users(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-project-user" => {
            let req: openai_admin_types::CreateProjectUserParams = decode_params(merged)?;
            let resp = client
                .create_project_user(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-project-user" => {
            let req: openai_admin_types::DeleteProjectUserParams = decode_params(merged)?;
            let resp = client
                .delete_project_user(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-project-user" => {
            let req: openai_admin_types::RetrieveProjectUserParams = decode_params(merged)?;
            let resp = client
                .retrieve_project_user(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modify-project-user" => {
            let req: openai_admin_types::ModifyProjectUserParams = decode_params(merged)?;
            let resp = client
                .modify_project_user(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-roles" => {
            let req: openai_admin_types::ListRolesParams = decode_params(merged)?;
            let resp = client.list_roles(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-role" => {
            let req: openai_admin_types::CreateRoleParams = decode_params(merged)?;
            let resp = client.create_role(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-role" => {
            let req: openai_admin_types::DeleteRoleParams = decode_params(merged)?;
            let resp = client.delete_role(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-role" => {
            let req: openai_admin_types::RetrieveRoleParams = decode_params(merged)?;
            let resp = client.retrieve_role(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-role" => {
            let req: openai_admin_types::UpdateRoleParams = decode_params(merged)?;
            let resp = client.update_role(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-organization-spend-alerts" => {
            let req: openai_admin_types::ListOrganizationSpendAlertsParams = decode_params(merged)?;
            let resp = client
                .list_organization_spend_alerts(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "create-organization-spend-alert" => {
            let req: openai_admin_types::CreateOrganizationSpendAlertParams =
                decode_params(merged)?;
            let resp = client
                .create_organization_spend_alert(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-organization-spend-alert" => {
            let req: openai_admin_types::DeleteOrganizationSpendAlertParams =
                decode_params(merged)?;
            let resp = client
                .delete_organization_spend_alert(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-organization-spend-alert" => {
            let req: openai_admin_types::RetrieveOrganizationSpendAlertParams =
                decode_params(merged)?;
            let resp = client
                .retrieve_organization_spend_alert(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "update-organization-spend-alert" => {
            let req: openai_admin_types::UpdateOrganizationSpendAlertParams =
                decode_params(merged)?;
            let resp = client
                .update_organization_spend_alert(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "Deleteorganizationspendlimit" => {
            let req: openai_admin_types::DeleteorganizationspendlimitParams =
                decode_params(merged)?;
            let resp = client
                .deleteorganizationspendlimit(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "Getorganizationspendlimit" => {
            let req: openai_admin_types::GetorganizationspendlimitParams = decode_params(merged)?;
            let resp = client
                .getorganizationspendlimit(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "Updateorganizationspendlimit" => {
            let req: openai_admin_types::UpdateorganizationspendlimitParams =
                decode_params(merged)?;
            let resp = client
                .updateorganizationspendlimit(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-audio-speeches" => {
            let req: openai_admin_types::UsageAudioSpeechesParams = decode_params(merged)?;
            let resp = client
                .usage_audio_speeches(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-audio-transcriptions" => {
            let req: openai_admin_types::UsageAudioTranscriptionsParams = decode_params(merged)?;
            let resp = client
                .usage_audio_transcriptions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-code-interpreter-sessions" => {
            let req: openai_admin_types::UsageCodeInterpreterSessionsParams =
                decode_params(merged)?;
            let resp = client
                .usage_code_interpreter_sessions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-completions" => {
            let req: openai_admin_types::UsageCompletionsParams = decode_params(merged)?;
            let resp = client
                .usage_completions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-embeddings" => {
            let req: openai_admin_types::UsageEmbeddingsParams = decode_params(merged)?;
            let resp = client
                .usage_embeddings(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-file-search-calls" => {
            let req: openai_admin_types::UsageFileSearchCallsParams = decode_params(merged)?;
            let resp = client
                .usage_file_search_calls(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-images" => {
            let req: openai_admin_types::UsageImagesParams = decode_params(merged)?;
            let resp = client.usage_images(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-moderations" => {
            let req: openai_admin_types::UsageModerationsParams = decode_params(merged)?;
            let resp = client
                .usage_moderations(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-vector-stores" => {
            let req: openai_admin_types::UsageVectorStoresParams = decode_params(merged)?;
            let resp = client
                .usage_vector_stores(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "usage-web-search-calls" => {
            let req: openai_admin_types::UsageWebSearchCallsParams = decode_params(merged)?;
            let resp = client
                .usage_web_search_calls(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-users" => {
            let req: openai_admin_types::ListUsersParams = decode_params(merged)?;
            let resp = client.list_users(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "delete-user" => {
            let req: openai_admin_types::DeleteUserParams = decode_params(merged)?;
            let resp = client.delete_user(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-user" => {
            let req: openai_admin_types::RetrieveUserParams = decode_params(merged)?;
            let resp = client.retrieve_user(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "modify-user" => {
            let req: openai_admin_types::ModifyUserParams = decode_params(merged)?;
            let resp = client.modify_user(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list-user-role-assignments" => {
            let req: openai_admin_types::ListUserRoleAssignmentsParams = decode_params(merged)?;
            let resp = client
                .list_user_role_assignments(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "assign-user-role" => {
            let req: openai_admin_types::AssignUserRoleParams = decode_params(merged)?;
            let resp = client
                .assign_user_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "unassign-user-role" => {
            let req: openai_admin_types::UnassignUserRoleParams = decode_params(merged)?;
            let resp = client
                .unassign_user_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "retrieve-user-role" => {
            let req: openai_admin_types::RetrieveUserRoleParams = decode_params(merged)?;
            let resp = client
                .retrieve_user_role(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        other => Err(format!("no typed dispatch arm for {other}")),
    }
}

async fn dispatch_openrouter(
    client: OpenRouterClient,
    op: &CliOperation,
    merged: Value,
    stream: bool,
    output: Option<&Path>,
    multipart_files: &[(String, PathBuf)],
) -> Result<ExitCode, String> {
    match op.operation_id {
        "getUserActivity" => {
            let req: openrouter_types::GetUserActivityParams = decode_params(merged)?;
            let resp = client
                .get_user_activity(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getAnalyticsMeta" => {
            let req: openrouter_types::GetAnalyticsMetaParams = decode_params(merged)?;
            let resp = client
                .get_analytics_meta(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "queryAnalytics" => {
            let req: openrouter_types::QueryAnalyticsParams = decode_params(merged)?;
            let resp = client
                .query_analytics(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createAudioSpeech" => {
            let req: openrouter_types::CreateAudioSpeechParams = decode_params(merged)?;
            let resp = client
                .create_audio_speech(req, output)
                .await
                .map_err(|e| e.to_string())?;
            if output.is_some() {
                write_json(&json!({"ok": true, "bytes": resp.bytes.len()}))
                    .map_err(|e| e.to_string())?;
            } else {
                return write_binary(&resp.bytes, None).map_err(|e| e.to_string());
            }
            Ok(ExitCode::Success)
        }

        "createAudioTranscriptions" => {
            let req: openrouter_types::CreateAudioTranscriptionsParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .create_audio_transcriptions(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "exchangeAuthCodeForAPIKey" => {
            let req: openrouter_types::ExchangeAuthCodeForAPIKeyParams = decode_params(merged)?;
            let resp = client
                .exchange_auth_code_for_api_key(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createAuthKeysCode" => {
            let req: openrouter_types::CreateAuthKeysCodeParams = decode_params(merged)?;
            let resp = client
                .create_auth_keys_code(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getBenchmarks" => {
            let req: openrouter_types::GetBenchmarksParams = decode_params(merged)?;
            let resp = client
                .get_benchmarks(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listBYOKKeys" => {
            let req: openrouter_types::ListBYOKKeysParams = decode_params(merged)?;
            let resp = client
                .list_byok_keys(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createBYOKKey" => {
            let req: openrouter_types::CreateBYOKKeyParams = decode_params(merged)?;
            let resp = client
                .create_byok_key(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteBYOKKey" => {
            let req: openrouter_types::DeleteBYOKKeyParams = decode_params(merged)?;
            let resp = client
                .delete_byok_key(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getBYOKKey" => {
            let req: openrouter_types::GetBYOKKeyParams = decode_params(merged)?;
            let resp = client.get_byok_key(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateBYOKKey" => {
            let req: openrouter_types::UpdateBYOKKeyParams = decode_params(merged)?;
            let resp = client
                .update_byok_key(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "sendChatCompletionRequest" => {
            let req: openrouter_types::SendChatCompletionRequestParams = decode_params(merged)?;
            let resp = client
                .send_chat_completion_request(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "sendChatCompletionRequest_stream" => {
            let req: openrouter_types::SendChatCompletionRequestParams = decode_params(merged)?;
            let resp = client
                .send_chat_completion_request_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getTaskClassifications" => {
            let req: openrouter_types::GetTaskClassificationsParams = decode_params(merged)?;
            let resp = client
                .get_task_classifications(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getCredits" => {
            let req: openrouter_types::GetCreditsParams = decode_params(merged)?;
            let resp = client.get_credits(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createCoinbaseCharge" => {
            let req: openrouter_types::CreateCoinbaseChargeParams = decode_params(merged)?;
            let resp = client
                .create_coinbase_charge(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getAppRankings" => {
            let req: openrouter_types::GetAppRankingsParams = decode_params(merged)?;
            let resp = client
                .get_app_rankings(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getRankingsDaily" => {
            let req: openrouter_types::GetRankingsDailyParams = decode_params(merged)?;
            let resp = client
                .get_rankings_daily(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createEmbeddings" => {
            let req: openrouter_types::CreateEmbeddingsParams = decode_params(merged)?;
            let resp = client
                .create_embeddings(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createEmbeddings_stream" => {
            let req: openrouter_types::CreateEmbeddingsParams = decode_params(merged)?;
            let resp = client
                .create_embeddings_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listEmbeddingsModels" => {
            let req: openrouter_types::ListEmbeddingsModelsParams = decode_params(merged)?;
            let resp = client
                .list_embeddings_models(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listEndpointsZdr" => {
            let req: openrouter_types::ListEndpointsZdrParams = decode_params(merged)?;
            let resp = client
                .list_endpoints_zdr(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listFiles" => {
            let req: openrouter_types::ListFilesParams = decode_params(merged)?;
            let resp = client.list_files(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "uploadFile" => {
            let req: openrouter_types::UploadFileParams = decode_params(merged)?;
            let files = multipart_from(multipart_files);
            let resp = client
                .upload_file(req, files)
                .await
                .map_err(|e| e.to_string())?;
            write_json(&resp).map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }

        "deleteFile" => {
            let req: openrouter_types::DeleteFileParams = decode_params(merged)?;
            let resp = client.delete_file(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getFileMetadata" => {
            let req: openrouter_types::GetFileMetadataParams = decode_params(merged)?;
            let resp = client
                .get_file_metadata(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "downloadFileContent" => {
            let req: openrouter_types::DownloadFileContentParams = decode_params(merged)?;
            let resp = client
                .download_file_content(req, output)
                .await
                .map_err(|e| e.to_string())?;
            if output.is_some() {
                write_json(&json!({"ok": true, "bytes": resp.bytes.len()}))
                    .map_err(|e| e.to_string())?;
            } else {
                return write_binary(&resp.bytes, None).map_err(|e| e.to_string());
            }
            Ok(ExitCode::Success)
        }

        "getGeneration" => {
            let req: openrouter_types::GetGenerationParams = decode_params(merged)?;
            let resp = client
                .get_generation(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listGenerationContent" => {
            let req: openrouter_types::ListGenerationContentParams = decode_params(merged)?;
            let resp = client
                .list_generation_content(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "submitGenerationFeedback" => {
            let req: openrouter_types::SubmitGenerationFeedbackParams = decode_params(merged)?;
            let resp = client
                .submit_generation_feedback(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listGuardrails" => {
            let req: openrouter_types::ListGuardrailsParams = decode_params(merged)?;
            let resp = client
                .list_guardrails(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createGuardrail" => {
            let req: openrouter_types::CreateGuardrailParams = decode_params(merged)?;
            let resp = client
                .create_guardrail(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listKeyAssignments" => {
            let req: openrouter_types::ListKeyAssignmentsParams = decode_params(merged)?;
            let resp = client
                .list_key_assignments(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listMemberAssignments" => {
            let req: openrouter_types::ListMemberAssignmentsParams = decode_params(merged)?;
            let resp = client
                .list_member_assignments(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteGuardrail" => {
            let req: openrouter_types::DeleteGuardrailParams = decode_params(merged)?;
            let resp = client
                .delete_guardrail(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getGuardrail" => {
            let req: openrouter_types::GetGuardrailParams = decode_params(merged)?;
            let resp = client.get_guardrail(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateGuardrail" => {
            let req: openrouter_types::UpdateGuardrailParams = decode_params(merged)?;
            let resp = client
                .update_guardrail(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listGuardrailKeyAssignments" => {
            let req: openrouter_types::ListGuardrailKeyAssignmentsParams = decode_params(merged)?;
            let resp = client
                .list_guardrail_key_assignments(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "bulkAssignKeysToGuardrail" => {
            let req: openrouter_types::BulkAssignKeysToGuardrailParams = decode_params(merged)?;
            let resp = client
                .bulk_assign_keys_to_guardrail(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "bulkUnassignKeysFromGuardrail" => {
            let req: openrouter_types::BulkUnassignKeysFromGuardrailParams = decode_params(merged)?;
            let resp = client
                .bulk_unassign_keys_from_guardrail(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listGuardrailMemberAssignments" => {
            let req: openrouter_types::ListGuardrailMemberAssignmentsParams =
                decode_params(merged)?;
            let resp = client
                .list_guardrail_member_assignments(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "bulkAssignMembersToGuardrail" => {
            let req: openrouter_types::BulkAssignMembersToGuardrailParams = decode_params(merged)?;
            let resp = client
                .bulk_assign_members_to_guardrail(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "bulkUnassignMembersFromGuardrail" => {
            let req: openrouter_types::BulkUnassignMembersFromGuardrailParams =
                decode_params(merged)?;
            let resp = client
                .bulk_unassign_members_from_guardrail(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createImages" => {
            let req: openrouter_types::CreateImagesParams = decode_params(merged)?;
            let resp = client.create_images(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createImages_stream" => {
            let req: openrouter_types::CreateImagesParams = decode_params(merged)?;
            let resp = client
                .create_images_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listImageModels" => {
            let req: openrouter_types::ListImageModelsParams = decode_params(merged)?;
            let resp = client
                .list_image_models(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listImageModelEndpoints" => {
            let req: openrouter_types::ListImageModelEndpointsParams = decode_params(merged)?;
            let resp = client
                .list_image_model_endpoints(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getCurrentKey" => {
            let req: openrouter_types::GetCurrentKeyParams = decode_params(merged)?;
            let resp = client
                .get_current_key(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "list" => {
            let req: openrouter_types::ListParams = decode_params(merged)?;
            let resp = client.list(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createKeys" => {
            let req: openrouter_types::CreateKeysParams = decode_params(merged)?;
            let resp = client.create_keys(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteKeys" => {
            let req: openrouter_types::DeleteKeysParams = decode_params(merged)?;
            let resp = client.delete_keys(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getKey" => {
            let req: openrouter_types::GetKeyParams = decode_params(merged)?;
            let resp = client.get_key(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateKeys" => {
            let req: openrouter_types::UpdateKeysParams = decode_params(merged)?;
            let resp = client.update_keys(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createMessages" => {
            let req: openrouter_types::CreateMessagesParams = decode_params(merged)?;
            let resp = client
                .create_messages(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createMessages_stream" => {
            let req: openrouter_types::CreateMessagesParams = decode_params(merged)?;
            let resp = client
                .create_messages_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getModel" => {
            let req: openrouter_types::GetModelParams = decode_params(merged)?;
            let resp = client.get_model(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getModels" => {
            let req: openrouter_types::GetModelsParams = decode_params(merged)?;
            let resp = client.get_models(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listModelsCount" => {
            let req: openrouter_types::ListModelsCountParams = decode_params(merged)?;
            let resp = client
                .list_models_count(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listModelsUser" => {
            let req: openrouter_types::ListModelsUserParams = decode_params(merged)?;
            let resp = client
                .list_models_user(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listEndpoints" => {
            let req: openrouter_types::ListEndpointsParams = decode_params(merged)?;
            let resp = client
                .list_endpoints(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listObservabilityDestinations" => {
            let req: openrouter_types::ListObservabilityDestinationsParams = decode_params(merged)?;
            let resp = client
                .list_observability_destinations(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createObservabilityDestination" => {
            let req: openrouter_types::CreateObservabilityDestinationParams =
                decode_params(merged)?;
            let resp = client
                .create_observability_destination(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteObservabilityDestination" => {
            let req: openrouter_types::DeleteObservabilityDestinationParams =
                decode_params(merged)?;
            let resp = client
                .delete_observability_destination(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getObservabilityDestination" => {
            let req: openrouter_types::GetObservabilityDestinationParams = decode_params(merged)?;
            let resp = client
                .get_observability_destination(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateObservabilityDestination" => {
            let req: openrouter_types::UpdateObservabilityDestinationParams =
                decode_params(merged)?;
            let resp = client
                .update_observability_destination(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listOrganizationMembers" => {
            let req: openrouter_types::ListOrganizationMembersParams = decode_params(merged)?;
            let resp = client
                .list_organization_members(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listPresets" => {
            let req: openrouter_types::ListPresetsParams = decode_params(merged)?;
            let resp = client.list_presets(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getPreset" => {
            let req: openrouter_types::GetPresetParams = decode_params(merged)?;
            let resp = client.get_preset(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createPresetsChatCompletions" => {
            let req: openrouter_types::CreatePresetsChatCompletionsParams = decode_params(merged)?;
            let resp = client
                .create_presets_chat_completions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createPresetsChatCompletions_stream" => {
            let req: openrouter_types::CreatePresetsChatCompletionsParams = decode_params(merged)?;
            let resp = client
                .create_presets_chat_completions_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createPresetsMessages" => {
            let req: openrouter_types::CreatePresetsMessagesParams = decode_params(merged)?;
            let resp = client
                .create_presets_messages(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createPresetsMessages_stream" => {
            let req: openrouter_types::CreatePresetsMessagesParams = decode_params(merged)?;
            let resp = client
                .create_presets_messages_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createPresetsResponses" => {
            let req: openrouter_types::CreatePresetsResponsesParams = decode_params(merged)?;
            let resp = client
                .create_presets_responses(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createPresetsResponses_stream" => {
            let req: openrouter_types::CreatePresetsResponsesParams = decode_params(merged)?;
            let resp = client
                .create_presets_responses_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listPresetVersions" => {
            let req: openrouter_types::ListPresetVersionsParams = decode_params(merged)?;
            let resp = client
                .list_preset_versions(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getPresetVersion" => {
            let req: openrouter_types::GetPresetVersionParams = decode_params(merged)?;
            let resp = client
                .get_preset_version(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listProviders" => {
            let req: openrouter_types::ListProvidersParams = decode_params(merged)?;
            let resp = client
                .list_providers(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createRerank" => {
            let req: openrouter_types::CreateRerankParams = decode_params(merged)?;
            let resp = client.create_rerank(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createRerank_stream" => {
            let req: openrouter_types::CreateRerankParams = decode_params(merged)?;
            let resp = client
                .create_rerank_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createResponses" => {
            let req: openrouter_types::CreateResponsesParams = decode_params(merged)?;
            let resp = client
                .create_responses(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createResponses_stream" => {
            let req: openrouter_types::CreateResponsesParams = decode_params(merged)?;
            let resp = client
                .create_responses_stream(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                for ev in &resp.events {
                    write_ndjson_line(ev).map_err(|e| e.to_string())?;
                }
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createVideos" => {
            let req: openrouter_types::CreateVideosParams = decode_params(merged)?;
            let resp = client.create_videos(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listVideosModels" => {
            let req: openrouter_types::ListVideosModelsParams = decode_params(merged)?;
            let resp = client
                .list_videos_models(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getVideos" => {
            let req: openrouter_types::GetVideosParams = decode_params(merged)?;
            let resp = client.get_videos(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listVideosContent" => {
            let req: openrouter_types::ListVideosContentParams = decode_params(merged)?;
            let resp = client
                .list_videos_content(req, output)
                .await
                .map_err(|e| e.to_string())?;
            if output.is_some() {
                write_json(&json!({"ok": true, "bytes": resp.bytes.len()}))
                    .map_err(|e| e.to_string())?;
            } else {
                return write_binary(&resp.bytes, None).map_err(|e| e.to_string());
            }
            Ok(ExitCode::Success)
        }

        "listWorkspaces" => {
            let req: openrouter_types::ListWorkspacesParams = decode_params(merged)?;
            let resp = client
                .list_workspaces(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "createWorkspace" => {
            let req: openrouter_types::CreateWorkspaceParams = decode_params(merged)?;
            let resp = client
                .create_workspace(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteWorkspace" => {
            let req: openrouter_types::DeleteWorkspaceParams = decode_params(merged)?;
            let resp = client
                .delete_workspace(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "getWorkspace" => {
            let req: openrouter_types::GetWorkspaceParams = decode_params(merged)?;
            let resp = client.get_workspace(req).await.map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "updateWorkspace" => {
            let req: openrouter_types::UpdateWorkspaceParams = decode_params(merged)?;
            let resp = client
                .update_workspace(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listWorkspaceBudgets" => {
            let req: openrouter_types::ListWorkspaceBudgetsParams = decode_params(merged)?;
            let resp = client
                .list_workspace_budgets(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "deleteWorkspaceBudget" => {
            let req: openrouter_types::DeleteWorkspaceBudgetParams = decode_params(merged)?;
            let resp = client
                .delete_workspace_budget(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "upsertWorkspaceBudget" => {
            let req: openrouter_types::UpsertWorkspaceBudgetParams = decode_params(merged)?;
            let resp = client
                .upsert_workspace_budget(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "listWorkspaceMembers" => {
            let req: openrouter_types::ListWorkspaceMembersParams = decode_params(merged)?;
            let resp = client
                .list_workspace_members(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "bulkAddWorkspaceMembers" => {
            let req: openrouter_types::BulkAddWorkspaceMembersParams = decode_params(merged)?;
            let resp = client
                .bulk_add_workspace_members(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        "bulkRemoveWorkspaceMembers" => {
            let req: openrouter_types::BulkRemoveWorkspaceMembersParams = decode_params(merged)?;
            let resp = client
                .bulk_remove_workspace_members(req)
                .await
                .map_err(|e| e.to_string())?;
            if stream {
                write_ndjson_line(&resp).map_err(|e| e.to_string())?;
            } else {
                write_json(&resp).map_err(|e| e.to_string())?;
            }
            Ok(ExitCode::Success)
        }

        other => Err(format!("no typed dispatch arm for {other}")),
    }
}

struct ProviderMeta {
    base_url: String,
    display_name: String,
    admin_base_url: Option<String>,
    extra_headers: IndexMap<String, String>,
    env_key: Option<String>,
    admin_env_key: Option<String>,
}

fn resolve_provider_from_registry(provider: &str, home: &Path) -> Result<ProviderMeta, String> {
    match provider {
        "openai" => {
            return Ok(ProviderMeta {
                base_url: "https://api.openai.com/v1".into(),
                display_name: "OpenAI".into(),
                admin_base_url: None,
                extra_headers: IndexMap::new(),
                env_key: Some("OPENAI_API_KEY".into()),
                admin_env_key: Some("OPENAI_ADMIN_KEY".into()),
            });
        }
        "openrouter" => {
            return Ok(ProviderMeta {
                base_url: "https://openrouter.ai/api/v1".into(),
                display_name: "OpenRouter".into(),
                admin_base_url: None,
                extra_headers: IndexMap::new(),
                env_key: Some("OPENROUTER_API_KEY".into()),
                // Prefer OPENROUTER_ADMIN_API_KEY; OPENROUTER_MANAGEMENT_API_KEY is alias.
                admin_env_key: Some("OPENROUTER_ADMIN_API_KEY".into()),
            });
        }
        "zai" | "zai-model-api" => {
            return Ok(ProviderMeta {
                base_url: crate::agent::zai::ZAI_DEFAULT_BASE_URL.into(),
                display_name: "Z.ai".into(),
                admin_base_url: None,
                extra_headers: IndexMap::new(),
                env_key: Some(crate::agent::zai::ZAI_ENV_KEY.into()),
                admin_env_key: None,
            });
        }
        _ => {}
    }
    let cfg_path = home.join("config.toml");
    let raw = std::fs::read_to_string(&cfg_path).map_err(|e| format!("read config.toml: {e}"))?;
    let val: toml::Value = raw.parse().map_err(|e| format!("parse config: {e}"))?;
    let entry = val
        .get("model_providers")
        .and_then(|t| t.get(provider))
        .ok_or_else(|| format!("provider `{provider}` not found in config.toml"))?;
    let _ = ProviderId::new(provider).map_err(|e| e.to_string())?;
    let base = entry
        .get("base_url")
        .or_else(|| entry.get("api_base_url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("provider `{provider}` missing base_url"))?
        .to_owned();
    crate::provider_registry::lifecycle::validate_http_base_url(&base)
        .map_err(|e| e.to_string())?;
    let admin_base = entry
        .get("admin_base_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    if let Some(ref a) = admin_base {
        crate::provider_registry::lifecycle::validate_http_base_url(a)
            .map_err(|e| e.to_string())?;
    }
    let display_name = entry
        .get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or(provider)
        .to_owned();
    let mut extra_headers = IndexMap::new();
    if let Some(h) = entry.get("extra_headers").and_then(|v| v.as_table()) {
        for (k, v) in h {
            if let Some(s) = v.as_str() {
                extra_headers.insert(k.clone(), s.to_owned());
            }
        }
    }
    crate::provider_registry::lifecycle::validate_extra_headers(&extra_headers)
        .map_err(|e| e.to_string())?;
    let env_key = entry.get("env_key").and_then(|v| {
        v.as_str().map(|s| s.to_owned()).or_else(|| {
            v.as_array()
                .and_then(|a| a.first())
                .and_then(|x| x.as_str())
                .map(|s| s.to_owned())
        })
    });
    let admin_env_key = entry
        .get("admin_env_key")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    Ok(ProviderMeta {
        base_url: base,
        display_name,
        admin_base_url: admin_base,
        extra_headers,
        env_key,
        admin_env_key,
    })
}

fn resolve_app_token(
    provider: &str,
    home: &Path,
    pid: &ProviderId,
    env_key: Option<&str>,
) -> Option<String> {
    if let Some(name) = env_key {
        if let Ok(v) = std::env::var(name) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    match provider {
        "openai" => crate::auth::read_provider_api_key(home, crate::auth::OPENAI_API_KEY_SCOPE)
            .ok()
            .flatten()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok()),
        "openrouter" => {
            crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_API_KEY_SCOPE)
                .ok()
                .flatten()
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
        }
        "zai" | "zai-model-api" => read_provider_secret(home, &application_key_scope(pid))
            .ok()
            .flatten()
            .or_else(|| std::env::var(crate::agent::zai::ZAI_ENV_KEY).ok()),
        _ => read_provider_secret(home, &application_key_scope(pid))
            .ok()
            .flatten(),
    }
}

fn resolve_admin_token(
    provider: &str,
    home: &Path,
    pid: &ProviderId,
    admin_env_key: Option<&str>,
) -> Option<String> {
    // Never fall back to the application key when admin is missing.
    if let Some(name) = admin_env_key {
        if let Ok(v) = std::env::var(name) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    // Built-in OpenRouter management alias.
    if provider == "openrouter" {
        if let Ok(v) = std::env::var("OPENROUTER_MANAGEMENT_API_KEY") {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(v) = std::env::var("OPENROUTER_ADMIN_API_KEY") {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(Some(v)) =
            crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_ADMIN_KEY_SCOPE)
        {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(Some(v)) =
            crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_MANAGEMENT_KEY_SCOPE)
        {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    if provider == "openai" {
        if let Ok(v) = std::env::var("OPENAI_ADMIN_KEY") {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(Some(v)) =
            crate::auth::read_provider_api_key(home, crate::auth::OPENAI_ADMIN_KEY_SCOPE)
        {
            return Some(v);
        }
    }
    read_provider_secret(home, &admin_key_scope(pid))
        .ok()
        .flatten()
}
