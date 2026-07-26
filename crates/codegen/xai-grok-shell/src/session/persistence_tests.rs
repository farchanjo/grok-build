use super::*;
use crate::session::storage::jsonl::AppendDurability;

struct ActorGuard {
    handle: PersistenceHandle,
    task: tokio::task::JoinHandle<()>,
}

impl ActorGuard {
    async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

fn test_actor(info: Info, storage: Arc<dyn StorageAdapter>) -> ActorGuard {
    test_actor_with_remote_sync(info, storage, None)
}

fn test_actor_with_remote_sync(
    info: Info,
    storage: Arc<dyn StorageAdapter>,
    remote_sync: Option<RemoteSync>,
) -> ActorGuard {
    let (tx, rx) = mpsc::unbounded_channel();
    let summary_tx = tx.clone();
    let sampling_client =
        OaiCompatClient::new(xai_grok_inference::InferenceConfig::default()).unwrap();
    let task = tokio::spawn(
        SessionPersistence {
            info,
            storage,
            pending_notification: None,
            rx,
            remote_sync,
            relay_sync: None,
            summary: crate::session::summary::SummaryGenerator::new(
                crate::session::summary::SummaryConfig {
                    sampling_client,
                    model: String::new(),
                    persistence_tx: summary_tx,
                },
            ),
            registry_title_sync: None,
            gateway: None,
        }
        .run(),
    );
    ActorGuard {
        handle: PersistenceHandle { tx, noop: false },
        task,
    }
}

fn notification(info: &Info, text: &str) -> acp::SessionNotification {
    acp::SessionNotification::new(
        info.id.clone(),
        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
            acp::TextContent::new(text),
        ))),
    )
}

fn neutral_update(info: &Info, text: &str) -> SessionUpdate {
    SessionUpdate::Acp(Box::new(notification(info, text)))
}

fn compaction_request(
    id: &str,
    previous_history: Vec<ConversationItem>,
    compacted_history: Vec<ConversationItem>,
) -> xai_chat_state::CompactionPersistenceRequest {
    xai_chat_state::CompactionPersistenceRequest {
        metadata: xai_chat_state::CompactionPersistenceMetadata {
            checkpoint_id: id.to_owned(),
            prompt_index: 1,
            auto_continue_prompt: None,
            original_user_info: Some("<user_info>test</user_info>".to_owned()),
            created_at: "2026-07-26T00:00:00Z".to_owned(),
        },
        previous_history,
        compacted_history,
    }
}

async fn commit_compaction_through_actor(
    actor: &ActorGuard,
    request: xai_chat_state::CompactionPersistenceRequest,
) -> Result<(), xai_chat_state::CompactionPersistenceError> {
    let (respond_to, response) = tokio::sync::oneshot::channel();
    actor
        .handle
        .tx
        .send(PersistenceMsg::CommitCompactionAndAck {
            request,
            respond_to,
        })
        .unwrap();
    response.await.unwrap()
}

fn break_summary_writes(dir: &std::path::Path) {
    let summary = dir.join("summary.json");
    std::fs::remove_file(&summary).unwrap();
    std::fs::create_dir(summary).unwrap();
}

async fn recv_observed(
    observed: &mut tokio::sync::mpsc::UnboundedReceiver<acp::SessionNotification>,
) -> acp::SessionNotification {
    tokio::time::timeout(std::time::Duration::from_secs(1), observed.recv())
        .await
        .expect("remote sync timed out")
        .expect("remote sync observer closed")
}

#[test]
fn committed_error_returns_sync_disposition() {
    let info = Info {
        id: acp::SessionId::new("committed-update"),
        cwd: "/test".into(),
    };
    let notification = notification(&info, "committed");
    let PendingAppendOutcome::CommittedErr(sync_notification, error) =
        SessionPersistence::finish_pending_append(
            notification,
            Err(crate::session::storage::AppendUpdateError::Committed(
                io::Error::other("summary patch failed"),
            )),
        )
    else {
        panic!("expected committed failure");
    };
    assert_eq!(sync_notification.session_id, info.id);
    assert_eq!(error.to_string(), "summary patch failed");
}

#[test]
fn uncommitted_error_returns_restore_disposition() {
    let info = Info {
        id: acp::SessionId::new("uncommitted-update"),
        cwd: "/test".into(),
    };
    let notification = notification(&info, "pending");
    let PendingAppendOutcome::NotCommittedErr(pending_notification, error) =
        SessionPersistence::finish_pending_append(
            notification,
            Err(crate::session::storage::AppendUpdateError::NotCommitted(
                io::Error::other("append failed"),
            )),
        )
    else {
        panic!("expected uncommitted failure");
    };
    assert_eq!(pending_notification.session_id, info.id);
    assert_eq!(error.to_string(), "append failed");
}

#[tokio::test]
async fn compaction_commit_persists_checkpoint_history_then_marker() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-commit"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![
        ConversationItem::system("sys"),
        ConversationItem::user("old"),
    ];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    let compacted = vec![
        ConversationItem::system("sys"),
        ConversationItem::compaction_summary("summary"),
    ];
    let actor = test_actor(info.clone(), storage.clone());
    commit_compaction_through_actor(
        &actor,
        compaction_request("checkpoint-1", previous, compacted.clone()),
    )
    .await
    .unwrap();

    let persisted = storage.load_session(&info).await.unwrap();
    assert_eq!(
        persisted
            .chat_history
            .iter()
            .map(ConversationItem::text_content)
            .collect::<Vec<_>>(),
        vec!["sys", "summary"]
    );
    assert!(persisted.updates.iter().any(|update| matches!(
        update,
        SessionUpdate::Xai(notification)
            if matches!(
                notification.update,
                crate::extensions::notification::SessionUpdate::CompactionCheckpoint(_)
            )
    )));
    let checkpoint = storage
        .read_compaction_checkpoint(&info, "compaction_checkpoints/checkpoint-1.json")
        .await
        .unwrap();
    assert_eq!(checkpoint.schema_version, 2);
    assert_eq!(checkpoint.compacted_history.len(), compacted.len());
    assert!(
        !dir.path().join("compaction_pending.json").exists(),
        "the recovery marker must be removed after the durable marker commits"
    );
    actor.stop().await;
}

#[tokio::test]
async fn history_replacement_and_rollback_failure_does_not_append_marker() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-history-failure"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![
        ConversationItem::system("sys"),
        ConversationItem::user("old"),
    ];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    std::fs::remove_file(dir.path().join("chat_history.jsonl")).unwrap();
    std::fs::create_dir(dir.path().join("chat_history.jsonl")).unwrap();
    let actor = test_actor(info.clone(), storage.clone());
    let result = commit_compaction_through_actor(
        &actor,
        compaction_request(
            "checkpoint-history-failure",
            previous,
            vec![ConversationItem::compaction_summary("summary")],
        ),
    )
    .await;
    assert!(matches!(
        result,
        Err(xai_chat_state::CompactionPersistenceError::Indeterminate(_))
    ));
    let updates_path = dir.path().join("updates.jsonl");
    let updates = match std::fs::read_to_string(&updates_path) {
        Ok(updates) => updates,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!("failed to read {}: {error}", updates_path.display()),
    };
    assert!(!updates.contains("compaction_checkpoint"));
    assert!(
        dir.path().join("compaction_pending.json").exists(),
        "indeterminate replacement must retain the startup recovery journal"
    );
    actor.stop().await;
}

#[tokio::test]
async fn compaction_marker_failure_restores_previous_history() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-marker-failure"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_update_append_probe(
        dir.path().to_path_buf(),
        |durability| {
            assert!(matches!(durability, AppendDurability::Durable));
            Err(io::Error::other("marker append failed"))
        },
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![
        ConversationItem::system("sys"),
        ConversationItem::user("old"),
    ];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    let actor = test_actor(info.clone(), storage.clone());

    let result = commit_compaction_through_actor(
        &actor,
        compaction_request(
            "checkpoint-marker-failure",
            previous.clone(),
            vec![
                ConversationItem::system("sys"),
                ConversationItem::compaction_summary("summary"),
            ],
        ),
    )
    .await;
    assert!(matches!(
        result,
        Err(xai_chat_state::CompactionPersistenceError::NotCommitted(_))
    ));
    let persisted = storage.load_session(&info).await.unwrap();
    assert_eq!(
        persisted
            .chat_history
            .iter()
            .map(ConversationItem::text_content)
            .collect::<Vec<_>>(),
        vec!["sys", "old"]
    );
    assert!(persisted.updates.is_empty());
    assert!(
        !dir.path().join("compaction_pending.json").exists(),
        "rollback must clear the pending recovery marker"
    );
    actor.stop().await;
}

#[tokio::test]
async fn load_restores_previous_history_from_pending_compaction_marker() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-crash-recovery"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![
        ConversationItem::system("sys"),
        ConversationItem::user("old"),
    ];
    let compacted = vec![
        ConversationItem::system("sys"),
        ConversationItem::compaction_summary("uncommitted summary"),
    ];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    storage
        .replace_chat_history_with_compaction_pending(&info, "crash-window", &previous, &compacted)
        .await
        .unwrap();
    assert!(dir.path().join("compaction_pending.json").exists());

    let persisted = storage.load_session(&info).await.unwrap();
    assert_eq!(
        persisted
            .chat_history
            .iter()
            .map(ConversationItem::text_content)
            .collect::<Vec<_>>(),
        vec!["sys", "old"]
    );
    assert!(
        !dir.path().join("compaction_pending.json").exists(),
        "startup recovery must clear the pending marker after rollback"
    );
}

#[tokio::test]
async fn load_preserves_items_appended_after_indeterminate_compaction_marker() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-indeterminate-tail-recovery"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![
        ConversationItem::system("sys"),
        ConversationItem::user("old"),
    ];
    let compacted = vec![
        ConversationItem::system("sys"),
        ConversationItem::compaction_summary("uncertain summary"),
    ];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    storage
        .replace_chat_history_with_compaction_pending(
            &info,
            "indeterminate-window",
            &previous,
            &compacted,
        )
        .await
        .unwrap();
    storage
        .append_chat_message(&info, &ConversationItem::user("later user"))
        .await
        .unwrap();
    storage
        .append_chat_message(&info, &ConversationItem::assistant("later assistant"))
        .await
        .unwrap();

    let persisted = storage.load_session(&info).await.unwrap();
    assert_eq!(
        persisted
            .chat_history
            .iter()
            .map(ConversationItem::text_content)
            .collect::<Vec<_>>(),
        vec!["sys", "old", "later user", "later assistant"]
    );
    assert!(!dir.path().join("compaction_pending.json").exists());
}

#[tokio::test]
async fn load_clears_pending_marker_after_rollback_cleanup_failure() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-rollback-cleanup-recovery"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![ConversationItem::user("old")];
    let compacted = vec![ConversationItem::compaction_summary("uncertain summary")];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    storage
        .replace_chat_history_with_compaction_pending(
            &info,
            "rolled-back-window",
            &previous,
            &compacted,
        )
        .await
        .unwrap();
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    storage
        .append_chat_message(&info, &ConversationItem::user("later user"))
        .await
        .unwrap();

    let persisted = storage.load_session(&info).await.unwrap();
    assert_eq!(
        persisted
            .chat_history
            .iter()
            .map(ConversationItem::text_content)
            .collect::<Vec<_>>(),
        vec!["old", "later user"]
    );
    assert!(!dir.path().join("compaction_pending.json").exists());
}

#[tokio::test]
async fn load_keeps_compacted_history_when_marker_committed_before_pending_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-committed-pending-recovery"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![ConversationItem::user("old")];
    let compacted = vec![ConversationItem::compaction_summary("committed summary")];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    let checkpoint = crate::extensions::notification::CompactionCheckpointFile {
        checkpoint_id: "committed-window".to_string(),
        prompt_index_at_compaction: 1,
        compacted_history: compacted.clone(),
        schema_version: 2,
        created_at: "2026-07-26T00:00:00Z".to_string(),
        original_user_info: None,
        reread_file_paths: vec![],
    };
    storage
        .write_compaction_checkpoint(&info, &checkpoint)
        .await
        .unwrap();
    storage
        .replace_chat_history_with_compaction_pending(
            &info,
            "committed-window",
            &previous,
            &compacted,
        )
        .await
        .unwrap();
    let marker = SessionUpdate::Xai(Box::new(
        crate::extensions::notification::SessionNotification {
            session_id: info.id.clone(),
            update: crate::extensions::notification::SessionUpdate::CompactionCheckpoint(Box::new(
                crate::extensions::notification::CompactionCheckpointInfo {
                    checkpoint_id: "committed-window".to_string(),
                    prompt_index_at_compaction: 1,
                    checkpoint_file: "compaction_checkpoints/committed-window.json".to_string(),
                    auto_continue: None,
                    schema_version: 2,
                    created_at: "2026-07-26T00:00:00Z".to_string(),
                },
            )),
            meta: None,
        },
    ));
    storage
        .append_update_durable_commit_aware(&info, &marker)
        .await
        .unwrap();

    let persisted = storage.load_session(&info).await.unwrap();
    assert_eq!(
        persisted
            .chat_history
            .iter()
            .map(ConversationItem::text_content)
            .collect::<Vec<_>>(),
        vec!["committed summary"]
    );
    assert!(!dir.path().join("compaction_pending.json").exists());
}

#[tokio::test]
async fn invalid_compaction_checkpoint_id_is_rejected_before_writes() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-invalid-id"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![ConversationItem::user("old")];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    let actor = test_actor(info.clone(), storage.clone());

    let result = commit_compaction_through_actor(
        &actor,
        compaction_request(
            "../escape",
            previous,
            vec![ConversationItem::compaction_summary("summary")],
        ),
    )
    .await;
    assert!(matches!(
        result,
        Err(xai_chat_state::CompactionPersistenceError::NotCommitted(_))
    ));
    assert!(!dir.path().join("compaction_pending.json").exists());
    assert!(!dir.path().join("compaction_checkpoints").exists());
    actor.stop().await;
}

#[tokio::test]
async fn history_replacement_with_failed_rollback_is_indeterminate() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-history-rollback-failure"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![
        ConversationItem::system("sys"),
        ConversationItem::user("old"),
    ];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    // Both replacement attempts rename chat_history.jsonl before summary
    // bookkeeping fails. Persistence therefore cannot claim that the old base
    // was restored, and startup must resolve the retained pending journal.
    break_summary_writes(dir.path());
    let compacted = vec![
        ConversationItem::system("sys"),
        ConversationItem::compaction_summary("summary"),
    ];
    let actor = test_actor(info.clone(), storage.clone());

    let result = commit_compaction_through_actor(
        &actor,
        compaction_request("checkpoint-history-rollback-failure", previous, compacted),
    )
    .await;
    assert!(matches!(
        result,
        Err(xai_chat_state::CompactionPersistenceError::Indeterminate(_))
    ));
    let updates_path = dir.path().join("updates.jsonl");
    let updates = match std::fs::read_to_string(&updates_path) {
        Ok(updates) => updates,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!("failed to read {}: {error}", updates_path.display()),
    };
    assert!(!updates.contains("compaction_checkpoint"));
    assert!(
        dir.path().join("compaction_pending.json").exists(),
        "indeterminate replacement must retain the startup recovery journal"
    );
    actor.stop().await;
}

#[tokio::test]
async fn committed_marker_error_keeps_compacted_history() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("compaction-marker-committed"),
        cwd: "/test".into(),
    };
    let summary_dir = dir.path().to_path_buf();
    let storage = Arc::new(JsonlStorageAdapter::with_update_append_probe(
        dir.path().to_path_buf(),
        move |durability| {
            if matches!(durability, AppendDurability::Durable) {
                break_summary_writes(&summary_dir);
            }
            Ok(())
        },
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let previous = vec![
        ConversationItem::system("sys"),
        ConversationItem::user("old"),
    ];
    storage
        .replace_chat_history(&info, &previous)
        .await
        .unwrap();
    let compacted = vec![
        ConversationItem::system("sys"),
        ConversationItem::compaction_summary("summary"),
    ];
    let actor = test_actor(info.clone(), storage.clone());

    let result = commit_compaction_through_actor(
        &actor,
        compaction_request("checkpoint-marker-committed", previous, compacted),
    )
    .await;
    assert!(matches!(
        result,
        Err(xai_chat_state::CompactionPersistenceError::Committed(_))
    ));
    let history = storage
        .load_chat_history_from_dir(dir.path())
        .expect("committed compacted history remains readable");
    assert_eq!(
        history
            .iter()
            .map(ConversationItem::text_content)
            .collect::<Vec<_>>(),
        vec!["sys", "summary"]
    );
    let updates = std::fs::read_to_string(dir.path().join("updates.jsonl")).unwrap();
    assert!(updates.contains("compaction_checkpoint"));
    assert!(
        !dir.path().join("compaction_pending.json").exists(),
        "a committed marker must clear the rollback marker even if later bookkeeping fails"
    );
    actor.stop().await;
}

#[tokio::test]
async fn noop_handle_rejects_durable_append() {
    let info = Info {
        id: acp::SessionId::new("noop-durable-update"),
        cwd: "/test".into(),
    };
    assert!(matches!(
        PersistenceHandle::noop()
            .append_update_durably(neutral_update(&info, "durable"))
            .await,
        Err(DurableAppendError::NotCommitted(error))
            if error.kind() == io::ErrorKind::Unsupported
    ));
}

#[tokio::test]
async fn pending_drain_disposition_controls_remote_sync() {
    let info = Info {
        id: acp::SessionId::new("pending-remote-sync"),
        cwd: "/test".into(),
    };
    let storage = JsonlStorageAdapter::with_update_append_probe("/unused".into(), |_| {
        Err(io::Error::other("append failed"))
    });
    let (remote_sync, mut observed) = RemoteSync::test_observer();
    let actor = test_actor_with_remote_sync(info.clone(), Arc::new(storage), Some(remote_sync));
    actor
        .handle
        .tx
        .send(PersistenceMsg::Update(neutral_update(&info, "pending")))
        .unwrap();
    assert!(matches!(
        actor
            .handle
            .append_update_durably(neutral_update(&info, "durable"))
            .await,
        Err(DurableAppendError::NotCommitted(_))
    ));
    assert!(observed.try_recv().is_err());
    actor.stop().await;

    let dir = tempfile::tempdir().unwrap();
    let attempts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed_attempts = attempts.clone();
    let storage = Arc::new(JsonlStorageAdapter::with_update_append_probe(
        dir.path().to_path_buf(),
        move |durability| {
            observed_attempts.lock().unwrap().push(durability);
            Ok(())
        },
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let (remote_sync, mut observed) = RemoteSync::test_observer();
    let actor = test_actor_with_remote_sync(info.clone(), storage, Some(remote_sync));
    actor
        .handle
        .tx
        .send(PersistenceMsg::Update(neutral_update(&info, "pending")))
        .unwrap();
    break_summary_writes(dir.path());
    assert!(matches!(
        actor
            .handle
            .append_update_durably(neutral_update(&info, "durable"))
            .await,
        Err(DurableAppendError::Committed(_))
    ));
    let synced = recv_observed(&mut observed).await;
    assert_eq!(synced.session_id, info.id);
    assert!(matches!(
        attempts.lock().unwrap().as_slice(),
        [AppendDurability::Buffered]
    ));
    actor.stop().await;
}

#[tokio::test]
async fn durable_append_committed_failure_is_synced() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("durable-remote-sync"),
        cwd: "/test".into(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    break_summary_writes(dir.path());
    let (remote_sync, mut observed) = RemoteSync::test_observer();
    let actor = test_actor_with_remote_sync(info.clone(), storage, Some(remote_sync));
    assert!(matches!(
        actor
            .handle
            .append_update_durably(neutral_update(&info, "durable"))
            .await,
        Err(DurableAppendError::Committed(_))
    ));
    let synced = recv_observed(&mut observed).await;
    assert_eq!(synced.session_id, info.id);
    actor.stop().await;
}

#[tokio::test]
async fn failed_pending_drain_retains_record_and_skips_durable_update() {
    let info = Info {
        id: acp::SessionId::new("durable-drain-failure"),
        cwd: "/test".into(),
    };
    let attempts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = attempts.clone();
    let storage =
        JsonlStorageAdapter::with_update_append_probe("/unused".into(), move |durability| {
            observed.lock().unwrap().push(durability);
            Err(io::Error::other("pending append failed"))
        });
    let actor = test_actor(info.clone(), Arc::new(storage));
    actor
        .handle
        .tx
        .send(PersistenceMsg::Update(neutral_update(&info, "pending")))
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            actor
                .handle
                .append_update_durably(neutral_update(&info, "durable"))
                .await
                .unwrap_err()
                .to_string(),
            "pending append failed"
        );
    }
    assert!(matches!(
        attempts.lock().unwrap().as_slice(),
        [AppendDurability::Buffered, AppendDurability::Buffered]
    ));
    actor.stop().await;
}

#[tokio::test]
async fn durable_append_drains_pending_update_in_fifo_order() {
    let dir = tempfile::tempdir().unwrap();
    let info = Info {
        id: acp::SessionId::new("durable-update"),
        cwd: dir.path().to_string_lossy().into_owned(),
    };
    let storage = Arc::new(JsonlStorageAdapter::with_explicit_session_dir(
        dir.path().to_path_buf(),
    ));
    storage
        .init_session(&info, default_model_id())
        .await
        .unwrap();
    let actor = test_actor(info.clone(), storage.clone());
    actor
        .handle
        .tx
        .send(PersistenceMsg::Update(neutral_update(&info, "before")))
        .unwrap();
    actor
        .handle
        .append_update_durably(neutral_update(&info, "durable"))
        .await
        .unwrap();
    let summary = storage.load_summary(&info).await.unwrap();
    assert_eq!(summary.num_messages, 2);

    let updates = storage.load_session(&info).await.unwrap().updates;
    let texts = updates
        .iter()
        .filter_map(|update| {
            let SessionUpdate::Acp(notification) = update else {
                return None;
            };
            let acp::SessionUpdate::AgentMessageChunk(chunk) = &notification.update else {
                return None;
            };
            let acp::ContentBlock::Text(text) = &chunk.content else {
                return None;
            };
            Some(text.text.clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(texts, ["before", "durable"]);
    actor.stop().await;
}
