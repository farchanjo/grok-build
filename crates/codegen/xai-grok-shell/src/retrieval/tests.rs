//! PR17 orchestrator integration tests (fake PR16 clients, injected clock).

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use xai_grok_config_types::RetrievalProfileConfig;
use xai_grok_inference::RetrievalError;

use super::clients::{FakeEmbedScript, FakeRerankScript, FakeRetrievalExecutor, RetrievalExecutor};
use super::clock::MockClock;
use super::error::{DegradationKind, OrchestratorError, RouteFailureClass};
use super::pipeline::{CandidateRow, PipelineOptions};
use super::registry::RetrievalRegistry;
use super::reload::{
    ReloadOutcome, SnapshotBuildInput, test_graph_two_embed_routes, test_provider_views_capable,
};
use super::service::{RetrievalService, RetrieveCandidates};
use super::telemetry::{RecordingTelemetrySink, debug_is_redacted};

fn build_reg(clock: Arc<MockClock>) -> (Arc<RetrievalRegistry>, Arc<FakeRetrievalExecutor>) {
    let reg = RetrievalRegistry::disabled_with_clock("/tmp/pr17-test-home", clock);
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    let input = SnapshotBuildInput {
        graph: test_graph_two_embed_routes(),
        graph_generation: 1,
        provider_generation: 9,
        provider_views: views,
        provider_meta: meta,
        parse_warnings: Vec::new(),
    };
    let out = reg.publish_build_input(0, input);
    assert!(
        matches!(out, ReloadOutcome::Published { .. }),
        "publish: {out:?}"
    );
    let fake = Arc::new(FakeRetrievalExecutor::new());
    (reg, fake)
}

fn service(
    reg: Arc<RetrievalRegistry>,
    fake: Arc<FakeRetrievalExecutor>,
) -> (RetrievalService, Arc<RecordingTelemetrySink>) {
    let tel = Arc::new(RecordingTelemetrySink::new());
    let svc = RetrievalService::new(reg)
        .with_executor(fake)
        .with_telemetry(tel.clone());
    (svc, tel)
}

#[tokio::test]
async fn route0_fail_route1_success_deterministic_order() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed(
        "emb-a",
        FakeEmbedScript::Err(RetrievalError::Http {
            status: 500,
            category: xai_grok_inference::RetrievalErrorCategory::Server,
            message: "boom".into(),
            request_id: None,
            provider_id: Some("acct-a".into()),
        }),
    );
    fake.set_embed("emb-b", FakeEmbedScript::Ok { dims: 8, fill: 0.5 });
    let (svc, _) = service(reg, fake.clone());
    let r = svc
        .embed(
            "default",
            vec!["hello".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .expect("embed");
    assert_eq!(r.route_model_id, "emb-b");
    assert_eq!(r.route_index, 1);
    assert_eq!(fake.embed_calls(), vec!["emb-a", "emb-b"]);
    assert_eq!(
        fake.provider_ids_seen(),
        vec!["acct-a".to_owned(), "acct-b".to_owned()]
    );
}

#[tokio::test]
async fn no_undeclared_sibling_fallback() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    // Only emb-a is declared first; make both fail — must not invent routes.
    fake.set_embed(
        "emb-a",
        FakeEmbedScript::Err(RetrievalError::MissingCredential),
    );
    fake.set_embed(
        "emb-b",
        FakeEmbedScript::Err(RetrievalError::MissingCredential),
    );
    let (svc, _) = service(reg, fake.clone());
    let err = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, OrchestratorError::AllRoutesFailed { .. }));
    assert_eq!(fake.embed_calls(), vec!["emb-a", "emb-b"]);
}

#[tokio::test]
async fn attempt_budget_across_routes_not_multiplied() {
    let clock = Arc::new(MockClock::new());
    let reg = RetrievalRegistry::disabled_with_clock("/tmp/pr17-budget", clock);
    let mut graph = test_graph_two_embed_routes();
    // three routes declared but max_attempts = 2
    graph.embedding_models.insert(
        "emb-c".into(),
        xai_grok_config_types::EmbeddingModelConfig {
            provider: "acct-a".into(),
            model: "c".into(),
            dimensions: Some(8),
            ..Default::default()
        },
    );
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .embedding_models = vec!["emb-a".into(), "emb-b".into(), "emb-c".into()];
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .max_attempts = 2;
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    reg.publish_build_input(
        0,
        SnapshotBuildInput {
            graph,
            graph_generation: 1,
            provider_generation: 1,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        },
    );
    let fake = Arc::new(FakeRetrievalExecutor::new());
    fake.set_embed("emb-a", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-b", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-c", FakeEmbedScript::Ok { dims: 8, fill: 1.0 });
    let (svc, _) = service(reg, fake.clone());
    let err = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            OrchestratorError::AllRoutesFailed { .. }
                | OrchestratorError::AttemptBudgetExceeded { .. }
        ),
        "{err:?}"
    );
    // Only 2 attempts — emb-c never reached.
    assert_eq!(fake.embed_calls(), vec!["emb-a", "emb-b"]);
}

#[tokio::test]
async fn cancellation_during_attempt() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::WaitForCancel);
    let (svc, _) = service(reg, fake);
    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();
    let handle = tokio::spawn(async move {
        svc.embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            cancel2,
        )
        .await
    });
    // Poll until the worker is parked on cancel.cancelled() (no wall sleeps).
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    cancel.cancel();
    let err = handle.await.unwrap().unwrap_err();
    assert!(matches!(err, OrchestratorError::Cancelled { .. }));
}

#[tokio::test]
async fn cancellation_before_start() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 8, fill: 0.1 });
    let (svc, _) = service(reg, fake);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let err = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            cancel,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, OrchestratorError::Cancelled { .. }));
}

#[tokio::test]
async fn cooldown_exact_route_sibling_unaffected() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock.clone());
    // Threshold 2 with short cooldown — use registry default threshold.
    fake.set_embed("emb-a", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-b", FakeEmbedScript::Ok { dims: 8, fill: 0.2 });
    let (svc, _) = service(reg.clone(), fake.clone());
    // First call: emb-a fails, emb-b succeeds (emb-a failure_count=1).
    let _ = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    // Second: emb-a fails again → cooldown; emb-b succeeds without trying emb-a?
    // After threshold=2, emb-a cools. Third call skips emb-a.
    let _ = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 8, fill: 9.0 });
    let before = fake.embed_calls().len();
    let r = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    // emb-a on cooldown → emb-b used.
    assert_eq!(r.route_model_id, "emb-b");
    let calls = fake.embed_calls();
    assert_eq!(&calls[before..], &["emb-b".to_owned()]);
    // Advance past cooldown; emb-a usable again.
    clock.advance(Duration::from_secs(31));
    let r2 = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(r2.route_model_id, "emb-a");
}

#[tokio::test]
async fn all_embed_fail_lexical_degrade_in_retrieve() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-b", FakeEmbedScript::Err(RetrievalError::Timeout));
    let (svc, _) = service(reg, fake);
    let rows = vec![
        CandidateRow {
            id: "1".into(),
            text: "alpha".into(),
            score: None,
            metadata: None,
        },
        CandidateRow {
            id: "2".into(),
            text: "beta".into(),
            score: None,
            metadata: None,
        },
    ];
    let out = svc
        .retrieve(
            "default",
            "query",
            RetrieveCandidates::Explicit(rows.clone()),
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(out.embed.is_none());
    assert!(
        out.degradations
            .iter()
            .any(|d| d.kind == DegradationKind::SemanticUnavailable)
    );
    // Lexical order preserved.
    assert_eq!(
        out.candidates
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec!["1", "2"]
    );
}

#[tokio::test]
async fn hard_error_opt_in_on_semantic_failure() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-b", FakeEmbedScript::Err(RetrievalError::Timeout));
    let (svc, _) = service(reg, fake);
    let err = svc
        .retrieve(
            "default",
            "q",
            RetrieveCandidates::Explicit(vec![CandidateRow {
                id: "1".into(),
                text: "t".into(),
                score: None,
                metadata: None,
            }]),
            PipelineOptions {
                hard_error_on_semantic_failure: true,
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        OrchestratorError::AllRoutesFailed {
            stage: super::error::RetrievalStage::Embed,
            ..
        }
    ));
}

#[tokio::test]
async fn hard_retrieve_rerank_budget_propagates_typed_attempt_error() {
    let clock = Arc::new(MockClock::new());
    let reg = RetrievalRegistry::disabled_with_clock("/tmp/pr17-hard-rerank-budget", clock);
    let mut graph = test_graph_two_embed_routes();
    // One embed success consumes 1 attempt; max_attempts=1 leaves none for rerank.
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .max_attempts = 1;
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .deadline_ms = 60_000;
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    reg.publish_build_input(
        0,
        SnapshotBuildInput {
            graph,
            graph_generation: 1,
            provider_generation: 1,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        },
    );
    let fake = Arc::new(FakeRetrievalExecutor::new());
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 8, fill: 0.1 });
    fake.set_rerank("rr-a", FakeRerankScript::ReverseOrder);
    let (svc, _) = service(reg, fake.clone());
    let err = svc
        .retrieve(
            "default",
            "q",
            RetrieveCandidates::Explicit(vec![
                CandidateRow {
                    id: "1".into(),
                    text: "a".into(),
                    score: None,
                    metadata: None,
                },
                CandidateRow {
                    id: "2".into(),
                    text: "b".into(),
                    score: None,
                    metadata: None,
                },
            ]),
            PipelineOptions {
                hard_error_on_semantic_failure: true,
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            OrchestratorError::AttemptBudgetExceeded {
                stage: super::error::RetrievalStage::Rerank,
                ..
            }
        ),
        "hard mode must surface typed rerank attempt error, got {err:?}"
    );
    assert!(
        fake.rerank_calls().is_empty(),
        "rerank executor must not run when attempt budget is exhausted"
    );
}

#[tokio::test]
async fn retrieve_cancel_stage_is_orchestrate() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::WaitForCancel);
    let (svc, _) = service(reg, fake);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let err = svc
        .retrieve(
            "default",
            "q",
            RetrieveCandidates::Explicit(vec![CandidateRow {
                id: "1".into(),
                text: "t".into(),
                score: None,
                metadata: None,
            }]),
            PipelineOptions::default(),
            cancel,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            OrchestratorError::Cancelled {
                stage: super::error::RetrievalStage::Orchestrate,
                ..
            }
        ),
        "retrieve cancel must use Orchestrate stage, got {err:?}"
    );
}

#[tokio::test]
async fn hard_retrieve_preserves_typed_deadline_error() {
    let clock = Arc::new(MockClock::new());
    let reg = RetrievalRegistry::disabled_with_clock("/tmp/pr17-hard-deadline", clock.clone());
    let mut graph = test_graph_two_embed_routes();
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .deadline_ms = 50;
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .max_attempts = 10;
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    reg.publish_build_input(
        0,
        SnapshotBuildInput {
            graph,
            graph_generation: 1,
            provider_generation: 1,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        },
    );
    let fake = Arc::new(FakeRetrievalExecutor::new());
    struct Adv {
        clock: Arc<MockClock>,
        inner: Arc<FakeRetrievalExecutor>,
        done: std::sync::atomic::AtomicBool,
    }
    #[async_trait::async_trait]
    impl super::clients::RetrievalExecutor for Adv {
        async fn embed(
            &self,
            home: &std::path::Path,
            model_id: &str,
            config: &xai_grok_config_types::EmbeddingModelConfig,
            pins: &super::clients::RouteCallPins,
            inputs: Vec<String>,
            cancel: CancellationToken,
        ) -> xai_grok_inference::RetrievalResult<xai_grok_inference::EmbeddingResult> {
            if !self.done.swap(true, std::sync::atomic::Ordering::SeqCst) {
                self.clock.advance(Duration::from_secs(60));
            }
            self.inner
                .embed(home, model_id, config, pins, inputs, cancel)
                .await
        }
        async fn rerank(
            &self,
            home: &std::path::Path,
            model_id: &str,
            config: &xai_grok_config_types::RerankerModelConfig,
            pins: &super::clients::RouteCallPins,
            query: String,
            documents: Vec<String>,
            top_n: Option<u32>,
            cancel: CancellationToken,
        ) -> xai_grok_inference::RetrievalResult<xai_grok_inference::RerankResult> {
            self.inner
                .rerank(
                    home, model_id, config, pins, query, documents, top_n, cancel,
                )
                .await
        }
    }
    fake.set_embed("emb-a", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-b", FakeEmbedScript::Ok { dims: 8, fill: 1.0 });
    let adv = Arc::new(Adv {
        clock,
        inner: fake,
        done: std::sync::atomic::AtomicBool::new(false),
    });
    let tel = Arc::new(RecordingTelemetrySink::new());
    let svc = RetrievalService::new(reg)
        .with_executor(adv)
        .with_telemetry(tel);
    let err = svc
        .retrieve(
            "default",
            "q",
            RetrieveCandidates::Explicit(vec![CandidateRow {
                id: "1".into(),
                text: "t".into(),
                score: None,
                metadata: None,
            }]),
            PipelineOptions {
                hard_error_on_semantic_failure: true,
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, OrchestratorError::DeadlineExceeded { .. }),
        "hard mode must preserve typed deadline, got {err:?}"
    );
}

#[tokio::test]
async fn route_error_debug_redacts_adapter_message() {
    let err = OrchestratorError::Route(RetrievalError::Http {
        status: 500,
        category: xai_grok_inference::RetrievalErrorCategory::Server,
        message: "sk-SECRET adapter body".into(),
        request_id: None,
        provider_id: Some("acct".into()),
    });
    let dbg = format!("{err:?}");
    let disp = format!("{err}");
    assert!(!dbg.contains("sk-SECRET"), "{dbg}");
    assert!(!disp.contains("sk-SECRET"), "{disp}");
}

#[tokio::test]
async fn fake_zero_deadline_pin_enforced() {
    let fake = FakeRetrievalExecutor::new();
    fake.set_embed("m", FakeEmbedScript::Ok { dims: 2, fill: 0.0 });
    let pins = super::clients::RouteCallPins {
        provenance_incarnation: None,
        session_registry_generation: None,
        total_deadline: Some(Duration::ZERO),
    };
    let err = fake
        .embed(
            std::path::Path::new("/tmp"),
            "m",
            &xai_grok_config_types::EmbeddingModelConfig {
                provider: "p".into(),
                model: "m".into(),
                ..Default::default()
            },
            &pins,
            vec!["q".into()],
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, RetrievalError::DeadlineExceeded));
}

#[tokio::test]
async fn bypass_missing_profile_still_bounds_candidates() {
    let reg = RetrievalRegistry::disabled("/tmp/pr17-bypass-bound");
    let svc = RetrievalService::new(reg);
    let many: Vec<CandidateRow> = (0..20_000)
        .map(|i| CandidateRow {
            id: i.to_string(),
            text: "x".into(),
            score: None,
            metadata: None,
        })
        .collect();
    let out = svc
        .retrieve(
            "missing",
            "q",
            RetrieveCandidates::Explicit(many),
            PipelineOptions {
                bypass_semantic: true,
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(out.candidates.len() <= 10_000);
}

#[test]
fn stable_home_key_independent_of_directory_existence() {
    let base = std::env::temp_dir().join(format!("pr17-home-key-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let key_before = super::stable_home_key(&base);
    std::fs::create_dir_all(&base).unwrap();
    let key_after = super::stable_home_key(&base);
    assert_eq!(
        key_before, key_after,
        "key must not change when directory is created"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn all_rerank_fail_preserves_pre_order() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 8, fill: 0.1 });
    fake.set_rerank(
        "rr-a",
        FakeRerankScript::Err(RetrievalError::MalformedResponse("bad".into())),
    );
    let (svc, _) = service(reg, fake);
    let rows = vec![
        CandidateRow {
            id: "a".into(),
            text: "doc-a".into(),
            score: None,
            metadata: None,
        },
        CandidateRow {
            id: "b".into(),
            text: "doc-b".into(),
            score: None,
            metadata: None,
        },
    ];
    let out = svc
        .retrieve(
            "default",
            "q",
            RetrieveCandidates::Explicit(rows),
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        out.candidates
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert!(
        out.rerank
            .as_ref()
            .is_some_and(|r| r.preserved_pre_rerank_order)
    );
}

#[tokio::test]
async fn rerank_success_reorders() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 8, fill: 0.1 });
    fake.set_rerank("rr-a", FakeRerankScript::ReverseOrder);
    let (svc, _) = service(reg, fake);
    let rows = vec![
        CandidateRow {
            id: "a".into(),
            text: "doc-a".into(),
            score: None,
            metadata: None,
        },
        CandidateRow {
            id: "b".into(),
            text: "doc-b".into(),
            score: None,
            metadata: None,
        },
    ];
    let out = svc
        .retrieve(
            "default",
            "q",
            RetrieveCandidates::Explicit(rows),
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        out.candidates
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b", "a"]
    );
}

#[tokio::test]
async fn in_flight_old_snapshot_next_call_new() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 8, fill: 0.1 });
    let (svc, _) = service(reg.clone(), fake.clone());
    let old = svc.load_snapshot();
    let gen_old = old.generation;

    // Publish new snapshot.
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    let mut graph = test_graph_two_embed_routes();
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .max_results = 3;
    let out = reg.publish_build_input(
        gen_old,
        SnapshotBuildInput {
            graph,
            graph_generation: 2,
            provider_generation: 10,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        },
    );
    assert!(matches!(out, ReloadOutcome::Published { .. }));
    let new = svc.load_snapshot();
    assert_ne!(new.generation, gen_old);
    // Old Arc retained.
    assert_eq!(old.generation, gen_old);
    assert_eq!(old.profiles["default"].config.max_results, 5);
    assert_eq!(new.profiles["default"].config.max_results, 3);
}

#[tokio::test]
async fn profile_missing_typed_error() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    let (svc, _) = service(reg, fake);
    let err = svc
        .embed(
            "no-such-profile",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        OrchestratorError::ProfileMissing { profile_id } if profile_id == "no-such-profile"
    ));
}

#[tokio::test]
async fn disabled_service_startup() {
    let reg = RetrievalRegistry::disabled("/tmp/pr17-disabled");
    let svc = RetrievalService::new(reg);
    assert!(!svc.is_enabled());
    let err = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, OrchestratorError::ServiceDisabled));
}

#[tokio::test]
async fn auth_failure_tries_next_explicit_route_no_credential_fallback() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed(
        "emb-a",
        FakeEmbedScript::Err(RetrievalError::MissingCredential),
    );
    fake.set_embed("emb-b", FakeEmbedScript::Ok { dims: 8, fill: 0.3 });
    let (svc, _) = service(reg, fake.clone());
    let r = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(r.route_model_id, "emb-b");
    // Exact providers only.
    assert_eq!(fake.provider_ids_seen(), vec!["acct-a", "acct-b"]);
}

#[tokio::test]
async fn malformed_rerank_falls_back_to_pre_order() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 4, fill: 0.0 });
    fake.set_rerank(
        "rr-a",
        FakeRerankScript::Err(RetrievalError::Decode("x".into())),
    );
    let (svc, _) = service(reg, fake);
    let out = svc
        .rerank(
            "default",
            "q".into(),
            vec!["d1".into(), "d2".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(out.preserved_pre_rerank_order);
    assert!(out.result.is_none());
}

#[tokio::test]
async fn input_budget_enforced() {
    let clock = Arc::new(MockClock::new());
    let reg = RetrievalRegistry::disabled_with_clock("/tmp/pr17-inbudget", clock);
    let mut graph = test_graph_two_embed_routes();
    let p = graph.retrieval_profiles.get_mut("default").unwrap();
    p.max_input_tokens = 1; // tiny
    p.max_attempts = 3;
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    reg.publish_build_input(
        0,
        SnapshotBuildInput {
            graph,
            graph_generation: 1,
            provider_generation: 1,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        },
    );
    let fake = Arc::new(FakeRetrievalExecutor::new());
    let (svc, _) = service(reg, fake);
    let big = "x".repeat(100);
    let err = svc
        .embed(
            "default",
            vec![big],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, OrchestratorError::InputBudgetExceeded { .. }));
}

#[tokio::test]
async fn bypass_semantic_hard_pin() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    let (svc, _) = service(reg, fake.clone());
    let out = svc
        .retrieve(
            "default",
            "q",
            RetrieveCandidates::Explicit(vec![CandidateRow {
                id: "n".into(),
                text: "native".into(),
                score: Some(1.0),
                metadata: None,
            }]),
            PipelineOptions {
                bypass_semantic: true,
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(out.embed.is_none());
    assert_eq!(out.candidates[0].id, "n");
    assert!(fake.embed_calls().is_empty());
}

#[tokio::test]
async fn telemetry_and_error_debug_redaction() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 2, fill: 0.0 });
    let (svc, tel) = service(reg, fake);
    let _ = svc
        .embed(
            "default",
            vec!["secret prompt sk-ABC123 should not leak".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    for ev in tel.events() {
        assert!(debug_is_redacted(&ev), "{ev:?}");
        let s = format!("{ev:?}");
        assert!(!s.contains("sk-ABC"));
        assert!(!s.contains("secret prompt"));
    }
    let err = OrchestratorError::AllRoutesFailed {
        profile_id: "default".into(),
        stage: super::error::RetrievalStage::Embed,
        last_failure: Some(RouteFailureClass::Auth),
    };
    let ds = format!("{err:?}");
    assert!(!ds.contains("sk-"));
    assert!(!ds.contains("Bearer"));
}

#[tokio::test]
async fn embedding_space_pins_first_success() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-b", FakeEmbedScript::Ok { dims: 8, fill: 0.7 });
    let (svc, _) = service(reg.clone(), fake);
    let r = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let space_b = reg
        .load()
        .embedding_route("emb-b")
        .unwrap()
        .embedding_space
        .clone();
    assert_eq!(r.embedding_space, space_b);
    // Spaces for a and b differ (different providers).
    let space_a = reg
        .load()
        .embedding_route("emb-a")
        .unwrap()
        .embedding_space
        .clone();
    assert_ne!(space_a.fingerprint(), space_b.fingerprint());
}

#[test]
fn stale_publish_and_lkg_pointer_stability() {
    let reg = RetrievalRegistry::disabled("/tmp/pr17-lkg");
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    let input = SnapshotBuildInput {
        graph: test_graph_two_embed_routes(),
        graph_generation: 1,
        provider_generation: 1,
        provider_views: views.clone(),
        provider_meta: meta.clone(),
        parse_warnings: Vec::new(),
    };
    reg.publish_build_input(0, input);
    let good = reg.load();
    let mut bad_graph = test_graph_two_embed_routes();
    bad_graph.retrieval_profiles.insert(
        "default".into(),
        RetrievalProfileConfig {
            embedding_models: vec!["does-not-exist".into()],
            ..Default::default()
        },
    );
    let bad = SnapshotBuildInput {
        graph: bad_graph,
        graph_generation: 2,
        provider_generation: 1,
        provider_views: views,
        provider_meta: meta,
        parse_warnings: Vec::new(),
    };
    let out = reg.publish_build_input(reg.generation(), bad);
    assert!(matches!(out, ReloadOutcome::RetainedLastKnownGood { .. }));
    let after = reg.load();
    assert_eq!(after.generation, good.generation);
    assert_eq!(after.fingerprint, good.fingerprint);
}

/// Executor that advances the injected clock on the first embed attempt so the
/// profile deadline expires before the second declared route is tried.
struct DeadlineAdvancingExecutor {
    clock: Arc<MockClock>,
    inner: Arc<FakeRetrievalExecutor>,
    advanced: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl super::clients::RetrievalExecutor for DeadlineAdvancingExecutor {
    async fn embed(
        &self,
        home: &std::path::Path,
        model_id: &str,
        config: &xai_grok_config_types::EmbeddingModelConfig,
        pins: &super::clients::RouteCallPins,
        inputs: Vec<String>,
        cancel: CancellationToken,
    ) -> xai_grok_inference::RetrievalResult<xai_grok_inference::EmbeddingResult> {
        if !self
            .advanced
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            // Exhaust profile deadline after the first route attempt starts.
            self.clock.advance(Duration::from_secs(60));
        }
        self.inner
            .embed(home, model_id, config, pins, inputs, cancel)
            .await
    }

    async fn rerank(
        &self,
        home: &std::path::Path,
        model_id: &str,
        config: &xai_grok_config_types::RerankerModelConfig,
        pins: &super::clients::RouteCallPins,
        query: String,
        documents: Vec<String>,
        top_n: Option<u32>,
        cancel: CancellationToken,
    ) -> xai_grok_inference::RetrievalResult<xai_grok_inference::RerankResult> {
        self.inner
            .rerank(
                home, model_id, config, pins, query, documents, top_n, cancel,
            )
            .await
    }
}

#[tokio::test]
async fn retrieve_shares_deadline_and_attempts_across_embed_rerank() {
    let clock = Arc::new(MockClock::new());
    let reg = RetrievalRegistry::disabled_with_clock("/tmp/pr17-shared-budget", clock.clone());
    let mut graph = test_graph_two_embed_routes();
    // Two embed routes + one rerank; max_attempts=2 so after both embeds fail,
    // rerank must not get a fresh attempt budget.
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .max_attempts = 2;
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .deadline_ms = 60_000;
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    reg.publish_build_input(
        0,
        SnapshotBuildInput {
            graph,
            graph_generation: 1,
            provider_generation: 1,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        },
    );
    let fake = Arc::new(FakeRetrievalExecutor::new());
    fake.set_embed("emb-a", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-b", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_rerank("rr-a", FakeRerankScript::ReverseOrder);
    let (svc, _) = service(reg, fake.clone());
    let out = svc
        .retrieve(
            "default",
            "q",
            RetrieveCandidates::Explicit(vec![
                CandidateRow {
                    id: "1".into(),
                    text: "a".into(),
                    score: None,
                    metadata: None,
                },
                CandidateRow {
                    id: "2".into(),
                    text: "b".into(),
                    score: None,
                    metadata: None,
                },
            ]),
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(out.embed.is_none());
    assert!(
        out.degradations
            .iter()
            .any(|d| d.kind == DegradationKind::SemanticUnavailable)
    );
    // Rerank must not have been attempted — attempt budget exhausted by embeds.
    assert!(
        fake.rerank_calls().is_empty(),
        "rerank calls: {:?}",
        fake.rerank_calls()
    );
    // Lexical order preserved.
    assert_eq!(
        out.candidates
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec!["1", "2"]
    );
}

#[tokio::test]
async fn output_budget_overflow_fails_closed_on_embed() {
    let clock = Arc::new(MockClock::new());
    let reg = RetrievalRegistry::disabled_with_clock("/tmp/pr17-outbudget", clock);
    let mut graph = test_graph_two_embed_routes();
    // Tiny output token budget so a small vector batch overflows.
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .max_output_tokens = 1;
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    reg.publish_build_input(
        0,
        SnapshotBuildInput {
            graph,
            graph_generation: 1,
            provider_generation: 1,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        },
    );
    let fake = Arc::new(FakeRetrievalExecutor::new());
    // 64 dims * 1 vector * 4 bytes = 256 bytes ≈ 64 tokens > 1
    fake.set_embed(
        "emb-a",
        FakeEmbedScript::Ok {
            dims: 64,
            fill: 0.1,
        },
    );
    let (svc, _) = service(reg, fake);
    let err = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, OrchestratorError::OutputBudgetExceeded { .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn fake_records_route_pins_and_deadline() {
    let clock = Arc::new(MockClock::new());
    let (reg, fake) = build_reg(clock);
    fake.set_embed("emb-a", FakeEmbedScript::Ok { dims: 8, fill: 0.1 });
    let (svc, _) = service(reg, fake.clone());
    let _ = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let pins = fake.embed_pins_seen();
    assert_eq!(pins.len(), 1);
    assert_eq!(
        pins[0].provenance_incarnation.as_deref(),
        Some("inc-acct-a")
    );
    assert_eq!(pins[0].session_registry_generation, Some(9));
    assert!(pins[0].total_deadline.is_some());
    assert!(!pins[0].total_deadline.unwrap().is_zero());
}

#[test]
fn multi_home_registries_are_isolated() {
    super::clear_all_registries();
    let dir_a = tempfile::TempDir::new().unwrap();
    let dir_b = tempfile::TempDir::new().unwrap();
    let reg_a = RetrievalRegistry::disabled(dir_a.path());
    let reg_b = RetrievalRegistry::disabled(dir_b.path());
    // Publish different fingerprints via force_publish generations.
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    let input = SnapshotBuildInput {
        graph: test_graph_two_embed_routes(),
        graph_generation: 1,
        provider_generation: 1,
        provider_views: views,
        provider_meta: meta,
        parse_warnings: Vec::new(),
    };
    assert!(matches!(
        reg_a.publish_build_input(0, input.clone()),
        ReloadOutcome::Published { .. }
    ));
    // Second home remains disabled until installed/published.
    super::install_registry_for_home(dir_a.path(), reg_a.clone());
    super::install_registry_for_home(dir_b.path(), reg_b.clone());
    let loaded_a = super::registry_for_home(dir_a.path()).unwrap();
    let loaded_b = super::registry_for_home(dir_b.path()).unwrap();
    assert!(loaded_a.load().enabled);
    assert!(!loaded_b.load().enabled);
    assert!(!std::sync::Arc::ptr_eq(&loaded_a, &loaded_b));
    super::clear_all_registries();
}

#[tokio::test]
async fn candidate_row_debug_omits_text() {
    let row = CandidateRow {
        id: "x".into(),
        text: "secret document body sk-ABC".into(),
        score: Some(0.5),
        metadata: None,
    };
    let dbg = format!("{row:?}");
    assert!(!dbg.contains("secret document"));
    assert!(!dbg.contains("sk-ABC"));
    assert!(dbg.contains("text_chars"));
}

#[tokio::test]
async fn deadline_budget_stops_fallback_chain() {
    let clock = Arc::new(MockClock::new());
    let reg = RetrievalRegistry::disabled_with_clock("/tmp/pr17-deadline", clock.clone());
    let mut graph = test_graph_two_embed_routes();
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .deadline_ms = 50;
    graph
        .retrieval_profiles
        .get_mut("default")
        .unwrap()
        .max_attempts = 10;
    let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
    reg.publish_build_input(
        0,
        SnapshotBuildInput {
            graph,
            graph_generation: 1,
            provider_generation: 1,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        },
    );
    let fake = Arc::new(FakeRetrievalExecutor::new());
    fake.set_embed("emb-a", FakeEmbedScript::Err(RetrievalError::Timeout));
    fake.set_embed("emb-b", FakeEmbedScript::Ok { dims: 8, fill: 1.0 });
    let advancing = Arc::new(DeadlineAdvancingExecutor {
        clock,
        inner: fake.clone(),
        advanced: std::sync::atomic::AtomicBool::new(false),
    });
    let tel = Arc::new(RecordingTelemetrySink::new());
    let svc = RetrievalService::new(reg)
        .with_executor(advancing)
        .with_telemetry(tel);
    let err = svc
        .embed(
            "default",
            vec!["q".into()],
            PipelineOptions::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            OrchestratorError::AllRoutesFailed { .. } | OrchestratorError::DeadlineExceeded { .. }
        ),
        "{err:?}"
    );
    // Second route must not be attempted after deadline expiry.
    assert_eq!(fake.embed_calls(), vec!["emb-a"]);
}
