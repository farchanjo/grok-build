use super::support::*;
use super::*;
use tokio::sync::mpsc;
use xai_grok_paths::AbsPathBuf;
use xai_grok_workspace::file_system::MockFs;
use xai_grok_workspace::permission::PermissionHandle;
#[test]
fn initial_injection_backend_params_use_override_min_score() {
    let params = crate::session::memory::MemoryBackendParams {
        session_id: "test-session".to_owned(),
        embed_config: None,
        embed_base_url: "http://localhost".to_owned(),
        embed_api_key: None,
        search_config: crate::config::MemorySearchConfig {
            min_score: 0.35,
            ..Default::default()
        },
        watcher: None,
        stale_claim_secs: 60,
        search_source: "tool",
        embedding_credentials: crate::session::memory::EndpointScopedCredentials::none(),
        retrieval: None,
        index_config: crate::config::MemoryIndexConfig::default(),
        rebuild_backoff_secs: 0,
    };
    let initial_injection = crate::config::MemoryInitialInjectionConfig {
        enabled: true,
        min_score: Some(0.72),
    };
    let (adjusted, effective_min_score) =
        build_initial_injection_backend_params(&params, &initial_injection);
    assert_eq!("injection", adjusted.search_source);
    assert!((0.72 - adjusted.search_config.min_score).abs() < f32::EPSILON);
    assert!((0.72 - effective_min_score as f32).abs() < f32::EPSILON);
    assert!((0.35 - params.search_config.min_score).abs() < f32::EPSILON);
    assert_eq!("tool", params.search_source);
}
#[test]
fn initial_injection_backend_params_preserve_default_zero_min_score() {
    let params = crate::session::memory::MemoryBackendParams {
        session_id: "test-session".to_owned(),
        embed_config: None,
        embed_base_url: "http://localhost".to_owned(),
        embed_api_key: None,
        search_config: crate::config::MemorySearchConfig {
            min_score: 0.41,
            ..Default::default()
        },
        watcher: None,
        stale_claim_secs: 60,
        search_source: "tool",
        embedding_credentials: crate::session::memory::EndpointScopedCredentials::none(),
        retrieval: None,
        index_config: crate::config::MemoryIndexConfig::default(),
        rebuild_backoff_secs: 0,
    };
    let (adjusted, effective_min_score) = build_initial_injection_backend_params(
        &params,
        &crate::config::MemoryInitialInjectionConfig::default(),
    );
    assert_eq!("injection", adjusted.search_source);
    assert!((0.41 - adjusted.search_config.min_score).abs() < f32::EPSILON);
    assert!((0.0 - effective_min_score as f32).abs() < f32::EPSILON);
}
#[allow(clippy::field_reassign_with_default)]
async fn create_test_actor_with_memory(
    total_tokens: u64,
    context_window: u64,
    threshold_percent: u8,
    gateway_tx: mpsc::UnboundedSender<xai_acp_lib::AcpClientMessage>,
    persistence_tx: mpsc::UnboundedSender<PersistenceMsg>,
    memory_config: Option<crate::config::MemoryConfig>,
) -> SessionActor {
    let tmp = tempfile::TempDir::new().unwrap();
    let cwd_path = tmp.path().to_path_buf();
    let cwd = AbsPathBuf::new(cwd_path.clone()).unwrap();
    let fs = Arc::new(MockFs::new(cwd.to_path_buf()));
    let terminal = Arc::new(DummyTerminal {});
    let (hunk_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let hunk_tracker_handle = xai_hunk_tracker::HunkTrackerActor::spawn(
        "test-memory".to_string(),
        cwd.to_path_buf(),
        hunk_tx,
        xai_hunk_tracker::TrackingMode::AgentOnly,
        tokio_util::sync::CancellationToken::new(),
    );
    let tool_context = ToolContext::new(cwd.clone(), None, None, fs, terminal, hunk_tracker_handle);
    let memory_storage = memory_config
        .as_ref()
        .filter(|mc| mc.enabled)
        .map(|_| crate::session::memory::MemoryStorage::new(&cwd_path, None));
    let state = TokioMutex::new(State {
        running_task: None,
        pending_inputs: VecDeque::new(),
        combine_edit_holds: std::collections::HashSet::new(),
        pending_notifications: Vec::new(),
        notifications_suppressed: false,
        rewindable: false,
        nudges_used_this_session: 0,
    });
    let (chat_event_tx, _chat_event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
    let chat_state_handle = xai_chat_state::ChatStateActor::spawn(
        vec![],
        xai_grok_inference_types::InferenceSettings {
            base_url: "http://localhost".to_string(),
            model: "test".to_string(),
            max_completion_tokens: None,
            temperature: None,
            top_p: None,
            api_backend: Default::default(),
            extra_headers: Default::default(),
            context_window: std::num::NonZeroU64::new(context_window)
                .expect("test context_window must be non-zero"),
            reasoning_effort: None,
            stream_tool_calls: None,
            supports_native_schema: None,
            supports_strict_tools: None,
            supports_image_input: None,
            supports_audio_input: None,
            supports_video_input: None,
        },
        Box::new(xai_chat_state::NullChatPersistence),
        chat_event_tx,
        tokio_util::sync::CancellationToken::new(),
    );
    chat_state_handle.record_token_usage(total_tokens);
    std::mem::forget(tmp);
    let memory_initial_injection_config = memory_config
        .as_ref()
        .map_or_else(Default::default, |mc| mc.initial_injection.clone());
    SessionActor {
        session_info: SessionInfo {
            id: acp::SessionId::new("test-memory"),
            cwd: cwd.as_str().to_string(),
        },
        auth_method_id: test_auth_method_id("test-auth"),
        model_auth_memo: std::cell::RefCell::new(None),
        openrouter_fallback_models: std::cell::RefCell::new(Vec::new()),
        openrouter_provider_preferences: std::cell::RefCell::new(None),
        openrouter_plugins: std::cell::RefCell::new(Vec::new()),
        openrouter_pacing: std::cell::Cell::new(false),
        provider_credential_generation: std::cell::Cell::new(0),
        attribution_callback: None,
        auth_manager: None,
        state,
        notifications: NotificationSender {
            gateway: GatewaySender::new(gateway_tx),
            gateway_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            persistence_tx,
        },
        permissions: PermissionHandle::allow_all(),
        tool_context,
        deny_read_globs: Vec::new(),
        mcp_state: Arc::new(TokioMutex::new(McpState::new(vec![]))),
        mcp_strategy: McpInitStrategy::Blocking,
        chat_state_handle,
        unattributed_background_usage: std::sync::atomic::AtomicBool::new(false),
        current_prompt_id: std::sync::Arc::new(std::sync::Mutex::new(None)),
        pending_interactions: std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        telemetry_enabled: false,
        supports_backend_search: std::cell::Cell::new(false),
        tool_overrides: std::cell::RefCell::new(None),
        resolved_tool_overrides: std::sync::Arc::new(arc_swap::ArcSwapOption::empty()),
        compactions_remaining: std::cell::Cell::new(None),
        compaction_at_tokens: std::cell::Cell::new(None),
        doom_loop_recovery: None,
        doom_loop_turn_tally: Default::default(),
        file_state_tracker: Arc::new(FileStateTracker::new()),
        rewind_pending_prompt: std::sync::Mutex::new(None),
        startup_hints: StartupHints::default(),
        forked_tool_override: None,
        compaction: crate::session::compaction_config::CompactionConfig {
            threshold_percent: std::cell::Cell::new(threshold_percent),
            force_compact: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            context_window_override: None,
            count: std::sync::atomic::AtomicU64::new(0),
            auto_compact_suppressed: std::sync::atomic::AtomicU8::new(0),
            previous_model: std::cell::Cell::new(None),
            compaction_mode: xai_chat_state::CompactionMode::Transcript,
            verbatim_input: true,
            tool_choice: crate::util::config::CompactionToolChoice::Auto,
            prefire: crate::session::compaction_config::PrefireState::default(),
            prefix_released: std::sync::atomic::AtomicBool::new(false),
            rolling_in_flight: std::sync::atomic::AtomicBool::new(false),
        },
        memory: crate::session::memory_state::SessionMemory {
            flush_config: memory_config
                .as_ref()
                .map_or_else(Default::default, |mc| mc.flush.clone()),
            is_flushing: std::sync::atomic::AtomicBool::new(false),
            last_flush_compaction: std::sync::atomic::AtomicU64::new(0),
            storage: std::cell::RefCell::new(memory_storage),
            save_on_end: true,
            backend_params: None,
            initial_injection_config: memory_initial_injection_config,
            context_injected: std::sync::atomic::AtomicBool::new(false),
            flush_count: std::sync::atomic::AtomicU64::new(0),
            last_flush_content: std::cell::RefCell::new(None),
            flush_success_count: std::sync::atomic::AtomicU64::new(0),
            flush_error_count: std::sync::atomic::AtomicU64::new(0),
            search_counter: std::cell::RefCell::new(None),
            injection_count: std::sync::atomic::AtomicU64::new(0),
            compaction_recovery_count: std::sync::atomic::AtomicU64::new(0),
            chunks_added: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            dream_config: Default::default(),
            dream_count: std::sync::atomic::AtomicU64::new(0),
            dream_success_count: std::sync::atomic::AtomicU64::new(0),
            dream_error_count: std::sync::atomic::AtomicU64::new(0),
        },
        session_start: std::time::Instant::now(),
        inference_idle_timeout: Duration::from_secs(300),
        max_retries: 3,
        max_turns: None,
        pending_interjections: InterjectionBuffer::new(),
        pending_skill_reminders: Mutex::new(Vec::new()),
        idle_flush_timeout: memory_config
            .as_ref()
            .and_then(|mc| mc.flush.idle_timeout_secs)
            .map(std::time::Duration::from_secs),
        dream_check_timeout: memory_config
            .as_ref()
            .filter(|mc| mc.dream.enabled)
            .and_then(|mc| mc.dream.check_interval_secs)
            .filter(|&s| s > 0)
            .map(std::time::Duration::from_secs),
        last_idle_flush_conversation_len: std::sync::atomic::AtomicUsize::new(0),
        event_tx,
        buffering_settings: None,
        client_identifier: None,
        origin_client: None,
        feedback_manager: Arc::new(FeedbackManager::local_only("test-memory")),
        upload_queue: Arc::new(OnceLock::new()),
        sync_loop_cancel: None,
        turn_cancel: std::cell::RefCell::new(tokio_util::sync::CancellationToken::new()),
        agent: std::cell::RefCell::new(test_agent_default().await),
        last_reported_branch: std::sync::Arc::new(parking_lot::Mutex::new(None)),
        git_head_enabled: false,
        models_manager: Default::default(),
        selection_model_id: std::cell::RefCell::new(acp::ModelId::new(
            crate::test_support::TEST_MODEL,
        )),
        route_context: std::cell::RefCell::new(None),
        display_cwd: std::sync::OnceLock::new(),
        active_agent_type: parking_lot::Mutex::new(None),
        queue_exit_reminder_on_approved_exit: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        active_skill: parking_lot::Mutex::new(None),
        prime_cache: crate::session::prime::inventory::InventoryCache::new(),
        current_prompt_mode: Arc::new(parking_lot::Mutex::new(PromptMode::Agent)),
        turn_start_prompt_mode: parking_lot::Mutex::new(PromptMode::Agent),
        turn_prompt_mode: Arc::new(parking_lot::Mutex::new(PromptMode::Agent)),
        plan_mode: Arc::new(parking_lot::Mutex::new(
            crate::session::plan_mode::PlanModeTracker::new(std::path::PathBuf::from(
                "/tmp/test-session",
            )),
        )),
        goal_enabled: false,
        background_workflows_enabled: false,
        goal_harness_enabled: std::sync::atomic::AtomicBool::new(false),
        goal_harness_availability_reconciled: std::sync::atomic::AtomicBool::new(false),
        goal_tracker: Arc::new(parking_lot::Mutex::new(
            crate::session::goal_tracker::GoalTracker::new(std::path::PathBuf::from(
                "/tmp/test-session",
            )),
        )),
        goal_turn_task_ids: parking_lot::Mutex::new(std::collections::HashSet::new()),
        goal_continuation_streak: std::sync::atomic::AtomicU32::new(0),
        goal_blocked_streak: std::sync::atomic::AtomicU32::new(0),
        goal_update_rx: std::cell::RefCell::new(None),
        goal_update_tx: tokio::sync::mpsc::unbounded_channel().0,
        workflow_manager: crate::session::workflow::manager::WorkflowManager::test_bundle().0,
        workflow_launch_tx: tokio::sync::mpsc::unbounded_channel().0,
        goal_classifier_enabled: false,
        goal_planner_enabled: false,
        goal_summary_enabled: false,
        goal_verifier_skeptic_count: 1,
        goal_role_models: Default::default(),
        goal_use_current_model_only: false,
        goal_classifier_max_runs: crate::session::goal_classifier::GOAL_CLASSIFIER_MAX_RUNS_DEFAULT,
        goal_strategist_every: 5,
        goal_reverify_after: crate::session::acp_session::GOAL_REVERIFY_AFTER_DEFAULT,
        goal_plan_reconciled: std::sync::atomic::AtomicBool::new(false),
        pending_classifier_completions: parking_lot::Mutex::new(std::collections::VecDeque::new()),
        goal_classifier_in_flight: std::sync::atomic::AtomicBool::new(false),
        managed_mcp_handle: Default::default(),
        managed_mcp_expires_at: std::sync::Mutex::new(None),
        initial_client_mcp_servers: vec![],
        tool_metadata_snapshot: Arc::new(std::sync::Mutex::new(Default::default())),
        mcp_announced_servers: Mutex::new(HashMap::new()),
        mcp_reminder_mode: McpReminderMode::Delta,
        mcp_reminder_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        mcp_connecting_reminder_injected: std::cell::Cell::new(false),
        mcp_handshakes_done: Arc::new(tokio::sync::Notify::new()),
        user_input_generation: std::sync::atomic::AtomicU64::new(0),
        laziness_debug_log: None,
        deferred_prefix: TaskSlot::new(),
        extension_registry: xai_agent_lifecycle::LocalExtensionRegistry::default(),
        last_announced_local_date: std::cell::Cell::new(chrono::Local::now().date_naive()),
        prefix_carries_fallback_date: std::cell::Cell::new(false),
        last_search_prompt_index: std::sync::atomic::AtomicI64::new(-1),
        last_api_request_at: std::sync::atomic::AtomicI64::new(0),
        hook_registry: std::cell::RefCell::new(None),
        client_hooks: Default::default(),
        hook_resolved_workspace_root: String::new(),
        vcs_kind: xai_grok_workspace::session::git::VcsKind::Git,
        hook_load_errors: std::cell::RefCell::new(Vec::new()),
        plugin_registry: std::cell::RefCell::new(None),
        plugin_registry_handle: None,
        events: crate::session::events::EventTracker::new(std::path::Path::new("/tmp")),
        observability_bridge: noop_observability_bridge(),
        current_turn_number: std::cell::Cell::new(0),
        last_recap_main_turn: std::cell::Cell::new(0),
        recap_in_flight: std::cell::Cell::new(false),
        recap_epoch: std::cell::Cell::new(0),
        session_turn_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        streaming_turn_capture: parking_lot::Mutex::new(StreamingTurnCapture::default()),
        turn_stream_drained: parking_lot::Mutex::new(None),
        sampler_handle: xai_grok_inference::InferenceHandle::noop(),
        execution_backend: std::cell::Cell::new(
            crate::agent::execution_backend::ExecutionBackend::NativeInference,
        ),
        external_runtime: std::cell::RefCell::new(None),
        external_agent_runtime: std::cell::RefCell::new(None),
        rebuild_spec: crate::session::agent_rebuild::test_rebuild_spec_default(),
        media_config: std::cell::RefCell::new(crate::config::MediaConfig {
            image_model: Some(crate::test_support::TEST_MODEL.to_owned()),
            ..Default::default()
        }),
        image_describe_cache: Arc::new(crate::session::image_describe::ImageDescribeCache::new()),
        media_descriptor_store: Arc::new(
            crate::session::media_descriptors::MediaDescriptorStore::empty(std::path::Path::new(
                "/tmp",
            )),
        ),
        subagent_token_records: parking_lot::Mutex::new(HashMap::new()),
        workspace_ops: xai_grok_workspace::WorkspaceOps::for_test(),
        trace_config_template: std::cell::RefCell::new(None),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn test_is_flushing_suppresses_auto_compact() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel();
            let (persistence_tx, _) = mpsc::unbounded_channel();
            let actor = create_test_actor(90_000, 100_000, 85, gateway_tx, persistence_tx).await;
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_some(), "should trigger at 90%");
            actor
                .memory
                .is_flushing
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_none(), "should suppress when is_flushing=true");
            actor
                .memory
                .is_flushing
                .store(false, std::sync::atomic::Ordering::Relaxed);
            let result = actor.check_auto_compact_needed().await;
            assert!(
                result.is_some(),
                "should trigger again after is_flushing=false"
            );
        })
        .await;
}
/// Test that `force_compact` triggers auto-compact even below threshold,
/// and is consumed (reset to false) after a single use.
#[tokio::test(flavor = "current_thread")]
async fn test_force_compact_triggers_below_threshold() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel();
            let (persistence_tx, _) = mpsc::unbounded_channel();
            let actor = create_test_actor(10_000, 100_000, 85, gateway_tx, persistence_tx).await;
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_none(), "should not trigger at 10%");
            actor
                .compaction
                .force_compact
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let result = actor.check_auto_compact_needed().await;
            assert!(
                result.is_some(),
                "force_compact should trigger at any usage"
            );
            let info = result.unwrap();
            assert_eq!(info.tokens_used, 10_000);
            assert!(
                !actor
                    .compaction
                    .force_compact
                    .load(std::sync::atomic::Ordering::Relaxed),
                "force_compact should be consumed after use"
            );
            let result = actor.check_auto_compact_needed().await;
            assert!(result.is_none(), "should not trigger after flag consumed");
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
#[allow(clippy::field_reassign_with_default)]
async fn test_flush_config_from_memory_config() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel();
            let (persistence_tx, _) = mpsc::unbounded_channel();
            let mut config = crate::config::MemoryConfig::default();
            config.enabled = true;
            config.pruning.keep_last_n_turns = 7;
            config.pruning.soft_trim_threshold = 9999;
            config.flush.soft_threshold_tokens = 12345;
            let actor = create_test_actor_with_memory(
                50_000,
                100_000,
                85,
                gateway_tx,
                persistence_tx,
                Some(config),
            )
            .await;
            assert_eq!(actor.memory.flush_config.soft_threshold_tokens, 12345);
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
#[allow(clippy::field_reassign_with_default)]
async fn test_memory_flush_enabled_from_config() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel();
            let (persistence_tx, _) = mpsc::unbounded_channel();
            let mut config = crate::config::MemoryConfig::default();
            config.enabled = true;
            config.flush.enabled = true;
            let actor = create_test_actor_with_memory(
                50_000,
                100_000,
                85,
                gateway_tx.clone(),
                persistence_tx,
                Some(config),
            )
            .await;
            assert!(
                actor.memory.flush_config.enabled,
                "flush_config.enabled should be true from MemoryConfig"
            );
            let (persistence_tx2, _) = mpsc::unbounded_channel();
            let mut config2 = crate::config::MemoryConfig::default();
            config2.enabled = true;
            config2.flush.enabled = false;
            let actor2 = create_test_actor_with_memory(
                50_000,
                100_000,
                85,
                gateway_tx,
                persistence_tx2,
                Some(config2),
            )
            .await;
            assert!(
                !actor2.memory.flush_config.enabled,
                "flush_config.enabled should be false when config says so"
            );
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
#[allow(clippy::field_reassign_with_default)]
async fn test_memory_storage_created_when_enabled() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel();
            let (persistence_tx, _) = mpsc::unbounded_channel();
            let mut config = crate::config::MemoryConfig::default();
            config.enabled = true;
            let actor = create_test_actor_with_memory(
                50_000,
                100_000,
                85,
                gateway_tx.clone(),
                persistence_tx,
                Some(config),
            )
            .await;
            assert!(
                actor.memory.is_enabled(),
                "memory_storage should be Some when enabled"
            );
            let (persistence_tx2, _) = mpsc::unbounded_channel();
            let actor2 = create_test_actor_with_memory(
                50_000,
                100_000,
                85,
                gateway_tx,
                persistence_tx2,
                None,
            )
            .await;
            assert!(
                !actor2.memory.is_enabled(),
                "memory_storage should be None when disabled"
            );
        })
        .await;
}
/// Actor with injection enabled and an FTS index matching the test query, so
/// `first_turn_memory_reminder()` WOULD inject — tests can then prove the
/// idempotency guard alone is what suppresses re-injection.
#[allow(clippy::field_reassign_with_default)]
async fn create_injection_ready_actor(
    initial_conversation: Vec<xai_grok_inference_types::ConversationItem>,
) -> SessionActor {
    let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
    let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
    let mut config = crate::config::MemoryConfig::default();
    config.enabled = true;
    config.initial_injection = crate::config::MemoryInitialInjectionConfig {
        enabled: true,
        min_score: None,
    };
    let mut actor =
        create_test_actor_with_memory(1_000, 100_000, 85, gateway_tx, persistence_tx, Some(config))
            .await;
    let tmp = tempfile::TempDir::new().unwrap();
    let global_dir = tmp.path().join("memory");
    let workspace_dir = global_dir.join("test_ws");
    std::fs::create_dir_all(&workspace_dir).unwrap();
    let storage =
        crate::session::memory::MemoryStorage::with_paths(global_dir, workspace_dir.clone());
    crate::session::memory::index::init_sqlite_vec();
    let note = tmp.path().join("note.md");
    std::fs::write(
        &note,
        "# Conventions\n\nProject uses Rust for backend services.",
    )
    .unwrap();
    let mut idx = crate::session::memory::index::MemoryIndex::open_or_create(
        &workspace_dir.join("index.sqlite"),
        storage.clone(),
        crate::config::MemoryIndexConfig::default(),
        4,
    )
    .unwrap();
    idx.reindex_file(&note, "workspace").unwrap();
    drop(idx);
    actor.memory.storage = std::cell::RefCell::new(Some(storage));
    std::mem::forget(tmp);
    actor.memory.backend_params = Some(crate::session::memory::MemoryBackendParams {
        session_id: "test-memory".to_owned(),
        embed_config: None,
        embed_base_url: "http://localhost".to_owned(),
        embed_api_key: None,
        search_config: crate::config::MemorySearchConfig::default(),
        watcher: None,
        stale_claim_secs: 60,
        search_source: "tool",
        embedding_credentials: crate::session::memory::EndpointScopedCredentials::none(),
        retrieval: None,
        index_config: crate::config::MemoryIndexConfig::default(),
        rebuild_backoff_secs: 0,
    });
    actor
        .chat_state_handle
        .replace_conversation(initial_conversation);
    actor
}
/// Control: proves the harness setup is sufficient for injection, so the
/// companion test below isolates the idempotency guard.
#[tokio::test(flavor = "current_thread")]
async fn test_first_turn_reminder_injects_without_persisted_block() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let actor = create_injection_ready_actor(vec![
                xai_grok_inference_types::ConversationItem::system("You are a helpful assistant."),
                xai_grok_inference_types::ConversationItem::user(
                    "tell me about rust backend services conventions",
                ),
            ])
            .await;
            let reminder = actor.first_turn_memory_reminder().await;
            let reminder = reminder.expect("first turn with matching index must inject");
            assert!(
                reminder.contains(xai_chat_state::MEMORY_CONTEXT_OPEN_TAG),
                "reminder must be a tagged memory-context block, got: {reminder}"
            );
            assert!(
                actor
                    .memory
                    .context_injected
                    .load(std::sync::atomic::Ordering::Relaxed),
                "latch must be set after the injection decision runs"
            );
            assert_eq!(None, actor.first_turn_memory_reminder().await);
        })
        .await;
}
/// A block persisted by an earlier `--resume` segment must suppress the
/// re-search — a re-scored block would bust the prompt-prefix KV cache.
#[tokio::test(flavor = "current_thread")]
async fn test_first_turn_reminder_skips_when_block_persisted() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let persisted_block =
                crate::session::helpers::memory_context::format_memory_reminder(&[
                    xai_grok_tools::types::memory_backend::MemorySearchResult {
                        chunk_id: "prev:0".into(),
                        path: "MEMORY.md".into(),
                        start_line: 0,
                        end_line: 5,
                        score: 0.98,
                        snippet: "Project uses Rust for backend services.".into(),
                        source: "workspace".into(),
                        created_at: None,
                    },
                ])
                .unwrap();
            let actor = create_injection_ready_actor(vec![
                xai_grok_inference_types::ConversationItem::system(format!(
                    "You are a helpful assistant.\n\n{persisted_block}"
                )),
                xai_grok_inference_types::ConversationItem::user(
                    "tell me about rust backend services conventions",
                ),
            ])
            .await;
            assert_eq!(
                None,
                actor.first_turn_memory_reminder().await,
                "persisted block must be reused verbatim — no re-search, no re-injection"
            );
            assert_eq!(
                0,
                actor
                    .memory
                    .injection_count
                    .load(std::sync::atomic::Ordering::Relaxed),
                "guard skip must not count as an injection"
            );
            assert!(
                actor
                    .memory
                    .context_injected
                    .load(std::sync::atomic::Ordering::Relaxed),
                "latch must still be set so later turns skip cheaply"
            );
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
#[allow(clippy::field_reassign_with_default)]
async fn test_idle_flush_timeout_from_config() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = mpsc::unbounded_channel();
            let (persistence_tx, _) = mpsc::unbounded_channel();
            let mut config = crate::config::MemoryConfig::default();
            config.enabled = true;
            config.flush.idle_timeout_secs = Some(120);
            let actor = create_test_actor_with_memory(
                50_000,
                100_000,
                85,
                gateway_tx.clone(),
                persistence_tx,
                Some(config),
            )
            .await;
            assert_eq!(
                actor.idle_flush_timeout,
                Some(std::time::Duration::from_secs(120))
            );
            let (persistence_tx2, _) = mpsc::unbounded_channel();
            let mut config2 = crate::config::MemoryConfig::default();
            config2.enabled = true;
            config2.flush.idle_timeout_secs = None;
            let actor2 = create_test_actor_with_memory(
                50_000,
                100_000,
                85,
                gateway_tx,
                persistence_tx2,
                Some(config2),
            )
            .await;
            assert_eq!(actor2.idle_flush_timeout, None);
        })
        .await;
}

/// Flush and Dream embedding use the named facade through the shared factory.
#[tokio::test]
async fn facade_powers_flush_and_dream_embedding_emitters() {
    let dims = 4;
    let fake = std::sync::Arc::new(crate::session::memory::retrieval::FakeMemoryRetrieval::new(
        dims,
        "pinned-model",
    ));
    // What spawn sets under a named retrieval profile: embed_config = None,
    // retrieval = facade.
    let params = crate::session::memory::MemoryBackendParams {
        session_id: "t".into(),
        embed_config: None,
        embed_base_url: "http://chat.example/v1".into(),
        embed_api_key: Some("chat-key".into()),
        search_config: crate::config::MemorySearchConfig::default(),
        watcher: None,
        stale_claim_secs: 60,
        search_source: "tool",
        embedding_credentials: crate::session::memory::EndpointScopedCredentials::none(),
        retrieval: Some(fake.clone()),
        index_config: crate::config::MemoryIndexConfig::default(),
        rebuild_backoff_secs: 0,
    };
    let provider = params.make_embedding_provider().await;
    assert!(
        provider.is_some(),
        "a named facade must power flush/Dream embedding emitters"
    );
    assert_eq!(
        provider.unwrap().model_name(),
        "pinned-model",
        "flush/Dream must use the named facade"
    );

    // Unresolved named profile ⇒ FTS-only for these emitters too.
    let unresolved = crate::session::memory::MemoryBackendParams {
        retrieval: None,
        embed_config: None,
        ..params
    };
    assert!(
        unresolved.make_embedding_provider().await.is_none(),
        "unresolved named profile must leave flush/Dream emitters FTS-only"
    );
}

/// Startup reindex backfill uses the resolved named-profile facade.
#[tokio::test]
#[allow(clippy::field_reassign_with_default)]
async fn startup_reindex_backfill_mixed_config_uses_facade_only() {
    let tmp = tempfile::TempDir::new().unwrap();
    let global_dir = tmp.path().join("memory");
    let workspace_dir = global_dir.join("test_ws");
    std::fs::create_dir_all(&workspace_dir).unwrap();
    let storage =
        crate::session::memory::MemoryStorage::with_paths(global_dir, workspace_dir.clone());
    crate::session::memory::index::init_sqlite_vec();
    let db_path = workspace_dir.join("index.sqlite");

    // Keep a conflicting legacy config and active-chat key in the fixture.
    let facade_dims = 6usize;
    let fake = std::sync::Arc::new(crate::session::memory::retrieval::FakeMemoryRetrieval::new(
        facade_dims,
        "pinned-model",
    ));
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.enabled = true;
    memory_config.retrieval_profile = Some("prof-a".to_string());
    memory_config.embedding = crate::config::MemoryEmbeddingConfig {
        model: Some("legacy-model".into()),
        dimensions: 1024,
        ..Default::default()
    };

    // Exactly what the detached startup task computes at session scope.
    let (embed_config, dims) = super::spawn::startup_reindex_embedding_inputs(
        true,
        Some(&memory_config),
        Some(facade_dims),
    );
    assert!(
        embed_config.is_none(),
        "a named profile must disable the legacy config"
    );
    assert_eq!(
        dims, facade_dims,
        "startup index must open in the facade space"
    );

    // Install a fingerprint in the facade space.
    let small = tmp.path().join("small.md");
    std::fs::write(&small, "# Small\n\nRust small content.").unwrap();
    {
        let mut idx = crate::session::memory::index::MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            crate::config::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        idx.reindex_file(&small, "workspace").unwrap();
    }
    let install_embedder: Option<
        std::sync::Arc<dyn xai_grok_memory::embedding::EmbeddingProvider>,
    > = Some(std::sync::Arc::new(
        xai_grok_memory::embedding::RetrievalEmbeddingProvider::new(fake.clone()),
    ));
    let readiness = xai_grok_memory::rebuild::ensure_vectors_ready(
        &db_path,
        storage.clone(),
        crate::config::MemoryIndexConfig::default(),
        &xai_grok_memory::retrieval::stub_spec(facade_dims, "pinned-model"),
        install_embedder,
        60,
        0,
        Some(usize::MAX),
        tokio_util::sync::CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(
            readiness,
            xai_grok_memory::rebuild::VectorReadiness::Ready
                | xai_grok_memory::rebuild::VectorReadiness::ReadyMissing { .. }
        ),
        "facade-space fingerprint must install, got {readiness:?}"
    );

    // Add chunks without vectors for startup backfill.
    let mut idx = crate::session::memory::index::MemoryIndex::open_or_create(
        &db_path,
        storage.clone(),
        crate::config::MemoryIndexConfig::default(),
        dims,
    )
    .unwrap();
    for name in ["a.md", "b.md"] {
        let f = tmp.path().join(name);
        std::fs::write(&f, format!("# {name}\n\nRust {name} content.")).unwrap();
        idx.reindex_file(&f, "workspace").unwrap();
    }
    let missing_before = idx.chunks_without_embeddings().unwrap().len();
    assert!(missing_before > 0, "the new chunks must lack vectors");

    // Run backfill with the named facade and conflicting chat credentials.
    let credentials = crate::session::memory::EndpointScopedCredentials::none();
    let embed_calls_before = fake.embed_calls();
    let embedded = super::spawn::startup_reindex_backfill(
        &idx,
        Some(fake.clone()),
        embed_config,
        &credentials,
        Some("chat-secret"),
        "http://chat.example/v1",
    )
    .await;
    assert_eq!(
        embedded, missing_before,
        "startup back-fill must embed every missing chunk through the facade"
    );
    assert!(
        fake.embed_calls() > embed_calls_before,
        "the facade must be the only embedding route consulted"
    );
    assert!(
        idx.chunks_without_embeddings().unwrap().is_empty(),
        "the vec gap must be closed in the facade's pinned dimensions"
    );
}

/// An unresolved named profile leaves startup backfill FTS-only.
#[tokio::test]
#[allow(clippy::field_reassign_with_default)]
async fn startup_reindex_backfill_unresolved_named_is_fts_only() {
    let tmp = tempfile::TempDir::new().unwrap();
    let global_dir = tmp.path().join("memory");
    let workspace_dir = global_dir.join("test_ws");
    std::fs::create_dir_all(&workspace_dir).unwrap();
    let storage =
        crate::session::memory::MemoryStorage::with_paths(global_dir, workspace_dir.clone());
    crate::session::memory::index::init_sqlite_vec();
    let db_path = workspace_dir.join("index.sqlite");
    let dims = 1024usize;

    // The unresolved named profile disables the legacy config.
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.enabled = true;
    memory_config.retrieval_profile = Some("never-resolves".to_string());
    memory_config.embedding = crate::config::MemoryEmbeddingConfig {
        model: Some("legacy-model".into()),
        dimensions: 1024,
        ..Default::default()
    };
    let (embed_config, dims_from_inputs) =
        super::spawn::startup_reindex_embedding_inputs(true, Some(&memory_config), None);
    assert!(embed_config.is_none(), "unresolved named ⇒ legacy disabled");
    assert_eq!(dims_from_inputs, 1024);

    // Install a prior fingerprint so backfill is otherwise available.
    let small = tmp.path().join("small.md");
    std::fs::write(&small, "# Small\n\nRust small content.").unwrap();
    {
        let mut idx = crate::session::memory::index::MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            crate::config::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        idx.reindex_file(&small, "workspace").unwrap();
    }
    let install_embedder: Option<
        std::sync::Arc<dyn xai_grok_memory::embedding::EmbeddingProvider>,
    > = Some(std::sync::Arc::new(
        xai_grok_memory::embedding::RetrievalEmbeddingProvider::new(std::sync::Arc::new(
            crate::session::memory::retrieval::FakeMemoryRetrieval::new(dims, "prior-model"),
        )),
    ));
    let readiness = xai_grok_memory::rebuild::ensure_vectors_ready(
        &db_path,
        storage.clone(),
        crate::config::MemoryIndexConfig::default(),
        &xai_grok_memory::retrieval::stub_spec(dims, "prior-model"),
        install_embedder,
        60,
        0,
        Some(usize::MAX),
        tokio_util::sync::CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(
            readiness,
            xai_grok_memory::rebuild::VectorReadiness::Ready
                | xai_grok_memory::rebuild::VectorReadiness::ReadyMissing { .. }
        ),
        "fingerprint must install, got {readiness:?}"
    );

    // New chunks so there is back-fill work that must be skipped.
    let mut idx = crate::session::memory::index::MemoryIndex::open_or_create(
        &db_path,
        storage.clone(),
        crate::config::MemoryIndexConfig::default(),
        dims,
    )
    .unwrap();
    let f = tmp.path().join("fresh.md");
    std::fs::write(&f, "# Fresh\n\nRust fresh content.").unwrap();
    idx.reindex_file(&f, "workspace").unwrap();
    assert!(
        idx.vectors_safe_to_backfill(),
        "precondition: steady-state fingerprint present"
    );
    let missing = idx.chunks_without_embeddings().unwrap().len();
    assert!(missing > 0);

    // No provider is available for the unresolved profile.
    let credentials = crate::session::memory::EndpointScopedCredentials::none();
    let embedded = super::spawn::startup_reindex_backfill(
        &idx,
        None,
        None,
        &credentials,
        Some("chat-secret"),
        "http://chat.example/v1",
    )
    .await;
    assert_eq!(
        embedded, 0,
        "unresolved named profile must not embed at startup"
    );
    assert_eq!(
        idx.chunks_without_embeddings().unwrap().len(),
        missing,
        "no vector rows may be written under an unresolved named profile"
    );
}

/// Without a named profile, startup inputs keep the legacy embedding config.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn startup_reindex_embedding_inputs_legacy_only_when_no_named_profile() {
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.enabled = true;
    memory_config.embedding = crate::config::MemoryEmbeddingConfig {
        model: Some("legacy-model".into()),
        dimensions: 768,
        ..Default::default()
    };
    let (embed_config, dims) =
        super::spawn::startup_reindex_embedding_inputs(false, Some(&memory_config), None);
    let cfg = embed_config.expect("no named profile ⇒ legacy config stays");
    assert_eq!(cfg.model.as_deref(), Some("legacy-model"));
    assert_eq!(
        dims, 768,
        "index must open in the legacy space when no named profile"
    );
}
