#![cfg_attr(rustfmt, rustfmt::skip)]
    use super::*;

    // ── apply_session_event ────────────────────────────────────────────

    #[test]
    fn apply_compaction_started_sets_activity() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "hi".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(1),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        let update = XaiSessionUpdate::AutoCompactStarted {
            tokens_used: 90000,
            context_window: 131072,
            percentage: 85,
            reason: "threshold".into(),
        };
        assert!(apply_session_event(&update, &mut session, &mut scrollback, false));
        assert!(
            session.in_flight_prompt.is_none(),
            "compaction start implies server activity — cancel must not rewind prompt"
        );
        assert_eq!(
            session.compact_held_prompt.as_ref().map(|p| p.text.as_str()),
            Some("hi"),
            "hold prompt text for re-auth auto-resubmit if compact fails with auth"
        );
    }

    /// Compact failure keeps the hold; PromptResponse reauth gate decides stash.
    #[test]
    fn apply_compaction_failed_keeps_held_prompt() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.compact_held_prompt = Some(InFlightPrompt {
            text: "retry after login".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(1),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        for error in [
            "authentication problem — re-authenticate using /login and retry.",
            "this conversation is too large to compact.",
        ] {
            let update = XaiSessionUpdate::AutoCompactFailed {
                error: error.into(),
            };
            assert!(apply_session_event(&update, &mut session, &mut scrollback, false));
            assert_eq!(
                session.compact_held_prompt.as_ref().map(|p| p.text.as_str()),
                Some("retry after login"),
            );
        }
    }

    /// `ImageDropped` joins notes with `\n` and pushes a system block.
    /// Pin the `\n` separator so a `notes.join(" ")` regression is caught.
    #[test]
    fn apply_image_dropped_pushes_scrollback_block() {
        use crate::scrollback::block::RenderBlock;
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        let before = scrollback.len();
        let notes = vec![
            "Image 1 was dropped: corrupt.".to_string(),
            "Image 2 was dropped: too small (4×3).".to_string(),
        ];
        let update = XaiSessionUpdate::ImageDropped {
            notes: notes.clone(),
        };
        let changed = apply_session_event(&update, &mut session, &mut scrollback, false);
        assert!(changed);
        assert_eq!(scrollback.len(), before + 1);
        let entry = scrollback.entries_mut().last().expect("entry pushed");
        match &entry.block {
            RenderBlock::System(b) => {
                assert!(b.text.contains(&notes[0]));
                assert!(b.text.contains(&notes[1]));
                assert!(
                    b.text.contains('\n'),
                    "expected \\n separator between dropped notes, got: {:?}",
                    b.text
                );
            }
            other => panic!("expected System block, got {other:?}"),
        }
    }

    /// A successful compression needs no user action: log-only — no toast,
    /// no scrollback block, no redraw. Same live and on session replay.
    #[test]
    fn image_compressed_is_invisible_in_tui() {
        for replay in [false, true] {
            let mut agent = make_agent(Some("s1"));
            agent.session.loading_replay = replay;
            assert!(!apply_image_compressed(
                &mut agent,
                &[compressed_entry(1), compressed_entry(2)],
                "Compressed Image 1: 4.2 MB (3024x1964) \u{2192} 780 KB (1568x1018)",
            ));
            assert!(agent.toast.is_none(), "no toast (replay={replay})");
            assert_eq!(agent.scrollback.len(), 0, "no block (replay={replay})");
        }
    }

    /// The re-encode fallback (empty `images`) means the oversized original
    /// was kept — a persistent warning line, not a transient toast.
    #[test]
    fn image_compressed_fallback_warning_stays_in_scrollback() {
        use crate::scrollback::block::RenderBlock;
        let mut agent = make_agent(Some("s1"));
        let msg = "Image 1 could not be re-encoded under the 1.5 MB limit; the original attachment was kept.";
        assert!(apply_image_compressed(&mut agent, &[], msg));
        assert!(agent.toast.is_none(), "warning must not be transient");
        let entry = agent.scrollback.entries_mut().last().expect("block pushed");
        match &entry.block {
            RenderBlock::System(b) => assert_eq!(b.text, msg),
            other => panic!("expected System block, got {other:?}"),
        }
    }

    #[test]
    fn apply_retry_state_retrying_clears_in_flight_prompt() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "retry me".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(2),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        let retry = RetryState::Retrying {
            attempt: 1,
            max_retries: 3,
            reason: "rate limited".into(),
            backoff_ms: Some(60_000),
            is_rate_limited: true,
            provider_name: Some("OpenRouter".into()),
            provider_code: Some("rate_limited".into()),
        };
        apply_retry_state(&retry, &mut session, &mut scrollback, false);
        assert!(
            session.in_flight_prompt.is_none(),
            "RetryState bypasses session/update in_flight hook"
        );
    }

    #[test]
    fn retry_exhausted_rate_limited_sets_flag() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();

        assert!(!session.rate_limited);
        apply_retry_state(
            &RetryState::Exhausted {
                attempts: 3,
                reason: "rate limited".into(),
                is_rate_limited: true,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            session.rate_limited,
            "rate_limited flag must be set when is_rate_limited is true"
        );
    }

    #[test]
    fn retry_exhausted_rate_limited_empty_reason_uses_oauth_fallback() {
        use xai_grok_shell::inference::error::RATE_LIMITED_USER_MESSAGE_OAUTH;

        let empty = RetryState::Exhausted {
            attempts: 3,
            reason: "".into(),
            is_rate_limited: true,
        };

        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(&empty, &mut session, &mut scrollback, false);
        match last_session_event(&scrollback) {
            Some(SessionEvent::RetryFailed { error, .. }) => {
                assert_eq!(error, RATE_LIMITED_USER_MESSAGE_OAUTH);
            }
            other => panic!("expected empty-rate-limit RetryFailed, got {other:?}"),
        }
    }

    /// Production `RetryState::Exhausted.reason` is `InferenceError::Api`'s
    /// Display: `API error (status 429 Too Many Requests): …`.
    #[test]
    fn retry_exhausted_rate_limited_surfaces_server_detail() {
        let body = "The model is currently at capacity due to high demand. Please try again.";
        let reason = format!("API error (status 429 Too Many Requests): {body}");
        let exhausted = RetryState::Exhausted {
            attempts: 3,
            reason: reason.clone(),
            is_rate_limited: true,
        };

        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(&exhausted, &mut session, &mut scrollback, false);
        match last_session_event(&scrollback) {
            Some(SessionEvent::RetryFailed { error, .. }) => {
                assert_eq!(error, body);
                assert!(!error.contains("API error (status"));
            }
            other => panic!("expected detail RetryFailed, got {other:?}"),
        }
    }

    #[test]
    fn retry_exhausted_api_key_rewrites_consumer_subscription_upsell() {
        use xai_grok_shell::inference::error::RATE_LIMITED_USER_MESSAGE_API_KEY;

        let rpm = RetryState::Exhausted {
            attempts: 2,
            reason: "API error (status 429 Too Many Requests): \
                     Some resource has been exhausted: You are sending requests too quickly. \
                     Please slow down, or upgrade to a Grok subscription for higher limits: \
                     https://grok.com/supergrok"
                .into(),
            is_rate_limited: true,
        };

        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(&rpm, &mut session, &mut scrollback, true);
        match last_session_event(&scrollback) {
            Some(SessionEvent::RetryFailed { error, .. }) => {
                assert_eq!(error, RATE_LIMITED_USER_MESSAGE_API_KEY);
                assert!(!error.contains("grok.com/supergrok"));
            }
            other => panic!("expected API-key rate-limit RetryFailed, got {other:?}"),
        }
    }

    #[test]
    fn retry_exhausted_non_rate_limited_does_not_set_flag() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();

        apply_retry_state(
            &RetryState::Exhausted {
                attempts: 3,
                reason: "server error".into(),
                is_rate_limited: false,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            !session.rate_limited,
            "rate_limited flag must not be set when is_rate_limited is false"
        );
    }

    /// A rate-limit exhaustion whose flattened reason carries the
    /// free-usage code sets both flags and pushes NO generic block (the
    /// driver shows the paywall modal on PromptResponse; viewers keep no
    /// marker).
    #[test]
    fn retry_exhausted_free_usage_sets_paywall_flag_without_marker() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "try me again".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(2),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });

        apply_retry_state(
            &RetryState::Exhausted {
                attempts: 0,
                reason: "API error (status 429 Too Many Requests): \
                         subscription:free-usage-exhausted: You have used all your free usage."
                    .into(),
                is_rate_limited: true,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            session.rate_limited,
            "free-usage keeps rate_limited (TurnFailed/toast suppression)"
        );
        assert!(session.free_usage_blocked);
        assert_eq!(
            scrollback.len(),
            0,
            "no RetryFailed marker — the paywall modal shows instead"
        );
        assert!(
            session.in_flight_prompt.is_none(),
            "free-usage exhaustion clears in_flight_prompt like other failures"
        );
    }

    #[test]
    fn apply_retry_state_credit_limit_exhausted_preserves_in_flight_prompt() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "stash me".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(2),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        apply_retry_state(
            &RetryState::Exhausted {
                attempts: 3,
                reason: "status 403: run out of credits".into(),
                is_rate_limited: false,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            session.credit_limit_blocked,
            "credit_limit_blocked must be set for credit-limit 403"
        );
        assert!(
            session.in_flight_prompt.is_some(),
            "in_flight_prompt must be preserved so PromptResponse handler can stash it"
        );
        assert_eq!(session.in_flight_prompt.unwrap().text, "stash me");
    }

    #[test]
    fn apply_retry_state_credit_limit_failed_preserves_in_flight_prompt() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "stash me too".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(3),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        apply_retry_state(
            &RetryState::Failed {
                error_type: "proxy_error".into(),
                message: "status 403: run out of credits".into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            session.credit_limit_blocked,
            "credit_limit_blocked must be set for credit-limit 403"
        );
        assert!(
            session.in_flight_prompt.is_some(),
            "in_flight_prompt must be preserved so PromptResponse handler can stash it"
        );
        assert_eq!(session.in_flight_prompt.unwrap().text, "stash me too");
    }

    #[test]
    fn apply_retry_state_pool_402_sets_credit_limit_blocked() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "pool blocked".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(5),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        apply_retry_state(
            &RetryState::Failed {
                error_type: "proxy_error".into(),
                message:
                    "API error (status 402 Payment Required): Grok Build usage balance exhausted"
                        .into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            session.credit_limit_blocked,
            "credit_limit_blocked must be set for pool 402 balance exhausted"
        );
        assert!(session.in_flight_prompt.is_some());
    }

    #[test]
    fn apply_retry_state_non_credit_limit_failed_clears_in_flight_prompt() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "gone".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(4),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        apply_retry_state(
            &RetryState::Failed {
                error_type: "server_error".into(),
                message: "internal server error".into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            !session.credit_limit_blocked,
            "credit_limit_blocked must NOT be set for non-credit-limit errors"
        );
        assert!(
            session.in_flight_prompt.is_none(),
            "in_flight_prompt must be cleared for non-credit-limit errors"
        );
    }

    #[test]
    fn is_reauthable_failure_matrix() {
        assert!(is_reauthable_failure(Some("auth"), "Unauthorized (401)"));
        assert!(is_reauthable_failure(
            Some("api"),
            "Unauthorized (401) from https://proxy/v1/responses"
        ));
        assert!(is_reauthable_failure(None, "Unauthorized (401)"));
        // legacy_auth carries its own migration guidance — excluded.
        assert!(!is_reauthable_failure(
            Some("legacy_auth"),
            "Unauthorized (401) ... deprecated authentication method"
        ));
        // Unrelated failures must not be treated as re-authable.
        assert!(!is_reauthable_failure(
            Some("server_error"),
            "internal server error"
        ));
        assert!(!is_reauthable_failure(Some("api"), "model not found"));
    }

    /// A 401 with `error_type == "auth"` surfaces the actionable re-auth
    /// prompt instead of the raw "Retry failed: Unauthorized (401) …" dump.
    #[test]
    fn apply_retry_state_auth_failure_pushes_reauth_prompt() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(
            &RetryState::Failed {
                error_type: "auth".into(),
                message: "Unauthorized (401) from https://cli-chat-proxy.grok.com/v1/messages: \
                          no auth context"
                    .into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            matches!(
                last_session_event(&scrollback),
                Some(SessionEvent::ReAuthRequired)
            ),
            "auth 401 must surface the actionable re-auth prompt"
        );
        assert!(!session.credit_limit_blocked);
    }

    /// Milestone 1: Moonshot-through-OpenRouter 401 retains OpenRouter
    /// identity at the pager boundary, renders `/providers` repair copy,
    /// never mentions `/login`, and never routes to the global re-auth
    /// (xAI OAuth) path.
    #[test]
    fn apply_retry_state_openrouter_moonshot_401_provider_repair_no_login() {
        use xai_grok_shell::extensions::notification::{
            provider_credential_repair_message, ProviderCredentialFailure,
            PROVIDER_CREDENTIAL_ERROR_TYPE,
        };

        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "moonshot via openrouter".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(9),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        apply_retry_state(
            &RetryState::Failed {
                error_type: PROVIDER_CREDENTIAL_ERROR_TYPE.into(),
                message: provider_credential_repair_message("OpenRouter"),
                provider: Some(ProviderCredentialFailure {
                    provider_id: "openrouter".into(),
                    provider_name: "OpenRouter".into(),
                    failed_model_id: Some("moonshotai/kimi-k2".into()),
                    http_status: Some(401),
                    request_id: None,
                    generation_id: Some("gen-test-moonshot-401".into()),
                    backend: Some("chatcompletions".into()),
                    error_category: Some(PROVIDER_CREDENTIAL_ERROR_TYPE.into()),
                }),
            },
            &mut session,
            &mut scrollback,
            false,
        );

        let event = last_session_event(&scrollback).expect("session event");
        let msg = event.message();
        match &event {
            SessionEvent::ProviderCredentialRequired {
                provider_id,
                provider_name,
                failed_model_id,
            } => {
                assert_eq!(provider_id, "openrouter");
                assert_eq!(provider_name, "OpenRouter");
                assert_eq!(failed_model_id.as_deref(), Some("moonshotai/kimi-k2"));
            }
            other => panic!("expected ProviderCredentialRequired, got {other:?}"),
        }
        assert!(
            msg.contains("OpenRouter"),
            "repair copy must name OpenRouter: {msg}"
        );
        assert!(
            msg.contains("/providers"),
            "repair copy must direct to /providers: {msg}"
        );
        assert!(
            !msg.contains("/login"),
            "OpenRouter 401 must never mention /login: {msg}"
        );
        assert!(
            !matches!(event, SessionEvent::ReAuthRequired),
            "must not use the global ReAuthRequired /login path"
        );
        assert!(
            session.in_flight_prompt.is_some(),
            "provider-scoped auth failure preserves the in-flight prompt"
        );
    }

    /// Legacy-reader compatibility: a new OpenRouter payload whose additive
    /// `provider` field is ignored (defaults to None) must not become global
    /// `ReAuthRequired`, because error_type is `provider_credential` and the
    /// message omits `Unauthorized (401)`.
    #[test]
    fn legacy_reader_openrouter_payload_does_not_become_reauth_required() {
        use xai_grok_shell::extensions::notification::{
            provider_credential_repair_message, PROVIDER_CREDENTIAL_ERROR_TYPE,
        };

        // Simulate an older pager that only sees error_type + message.
        let retry = RetryState::Failed {
            error_type: PROVIDER_CREDENTIAL_ERROR_TYPE.into(),
            message: provider_credential_repair_message("OpenRouter"),
            provider: None,
        };
        // Round-trip JSON without provider must also stay non-reauthable.
        let json = serde_json::to_value(&retry).expect("serialize");
        assert!(json.get("provider").is_none() || json.get("provider").unwrap().is_null());
        let decoded: RetryState = serde_json::from_value(json).expect("deserialize");
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(&decoded, &mut session, &mut scrollback, false);
        let event = last_session_event(&scrollback).expect("event");
        assert!(
            !matches!(event, SessionEvent::ReAuthRequired),
            "legacy reader must not open global /login for OpenRouter payload: {event:?}"
        );
        assert!(
            matches!(event, SessionEvent::RetryFailed { .. }),
            "legacy path should surface RetryFailed, not ReAuthRequired: {event:?}"
        );
    }

    /// First-party xAI auth payloads remain compatible with ReAuthRequired.
    #[test]
    fn legacy_first_party_auth_payload_still_reauth_required() {
        let retry = RetryState::Failed {
            error_type: "auth".into(),
            message: "Unauthorized (401) from first-party proxy".into(),
            provider: None,
        };
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(&retry, &mut session, &mut scrollback, false);
        assert!(matches!(
            last_session_event(&scrollback),
            Some(SessionEvent::ReAuthRequired)
        ));
    }

    /// A recoverable auth failure preserves `in_flight_prompt` so the
    /// PromptResponse handler can stash it for auto-resubmit after re-auth.
    #[test]
    fn apply_retry_state_auth_failure_preserves_in_flight_prompt() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.in_flight_prompt = Some(InFlightPrompt {
            text: "retry after login".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(5),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        apply_retry_state(
            &RetryState::Failed {
                error_type: "auth".into(),
                message: "Unauthorized (401) from https://proxy/v1/messages".into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            session.in_flight_prompt.is_some(),
            "in_flight_prompt must be preserved on a recoverable auth failure"
        );
        assert_eq!(session.in_flight_prompt.unwrap().text, "retry after login");
    }

    /// A 401 reported with a non-auth `error_type` but an "Unauthorized
    /// (401)" message (the `InferenceErrorKind::Api` path) also prompts.
    #[test]
    fn apply_retry_state_401_message_without_auth_type_prompts_reauth() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(
            &RetryState::Failed {
                error_type: "api".into(),
                message: "Unauthorized (401) from https://proxy/v1/responses: invalid credentials"
                    .into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(matches!(
            last_session_event(&scrollback),
            Some(SessionEvent::ReAuthRequired)
        ));
    }

    /// Legacy WebLogin auth keeps its verbose message (with `grok logout` /
    /// `grok login` guidance), not the generic re-auth prompt.
    #[test]
    fn apply_retry_state_legacy_auth_keeps_detailed_message() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(
            &RetryState::Failed {
                error_type: "legacy_auth".into(),
                message: "Unauthorized (401) ... deprecated authentication method (WebLogin) ... \
                          run `grok logout` then `grok login`"
                    .into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(matches!(
            last_session_event(&scrollback),
            Some(SessionEvent::RetryFailed { .. })
        ));
    }

    /// Non-auth terminal failures still render the standard RetryFailed.
    #[test]
    fn apply_retry_state_generic_failure_still_shows_retry_failed() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(
            &RetryState::Failed {
                error_type: "server_error".into(),
                message: "internal server error".into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(matches!(
            last_session_event(&scrollback),
            Some(SessionEvent::RetryFailed { .. })
        ));
    }

    #[test]
    fn apply_retry_state_unsupported_image_shows_actionable_prompt() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(
            &RetryState::Failed {
                error_type: "api_404".into(),
                message: "API error (status 404 Not Found): \
                          No endpoints found that support image input"
                    .into(),
                provider: None,
            },
            &mut session,
            &mut scrollback,
            false,
        );
        let event = last_session_event(&scrollback);
        assert!(
            matches!(event, Some(SessionEvent::ImageInputUnsupported)),
            "unsupported image input must use the dedicated UI event, got {event:?}"
        );
        let message = event.expect("dedicated event").message();
        assert!(message.contains("/model"));
        assert!(!message.contains("404"));
        assert!(!message.contains("endpoint"));
    }

    /// A context overflow surfaces the actionable `ContextTooLarge` prompt (not the
    /// raw `RetryFailed`); `PromptResponse` then suppresses the redundant `TurnFailed`.
    #[test]
    fn apply_retry_state_context_length_shows_context_too_large() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        apply_retry_state(
            &RetryState::Failed {
                error_type: "context_length".into(),
                message: "API error (status 500): the prompt is too long for this model's \
                          context window"
                    .into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            matches!(
                last_session_event(&scrollback),
                Some(SessionEvent::ContextTooLarge)
            ),
            "context overflow must surface the actionable ContextTooLarge prompt"
        );
    }

    /// When the compaction handler already showed its "too large to compact" message,
    /// the overflow path does NOT stack a second `ContextTooLarge` prompt on top.
    #[test]
    fn apply_retry_state_context_length_does_not_duplicate_compaction_failed() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        scrollback.push_block(RenderBlock::session_event(SessionEvent::CompactionFailed {
            error: "this conversation is too large to compact.".into(),
        }));
        apply_retry_state(
            &RetryState::Failed {
                error_type: "context_length".into(),
                message: "the prompt is too long for this model's context window".into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            matches!(
                last_session_event(&scrollback),
                Some(SessionEvent::CompactionFailed { .. })
            ),
            "must not push a duplicate prompt on top of CompactionFailed"
        );
    }

    #[test]
    fn apply_compaction_completed_defers_message_until_turn_end() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        session.set_compaction_activity(Some(TurnActivity::AutoCompacting));
        let update = XaiSessionUpdate::AutoCompactCompleted {
            tokens_before: Some(858_000),
            tokens_after: 66_000,
            elapsed_ms: Some(500),
            summary_preview: None,
        };
        assert!(apply_session_event(&update, &mut session, &mut scrollback, false));
        assert_eq!(
            scrollback.len(),
            0,
            "live compaction completion must be deferred, not pushed immediately"
        );

        session.note_context_used(43_000);

        session.finish_turn(&mut scrollback,
        );
        match last_session_event(&scrollback) {
            Some(SessionEvent::CompactionCompleted {
                tokens_before,
                tokens_after,
                ..
            }) => {
                assert_eq!(tokens_before, Some(858_000));
                assert_eq!(
                    tokens_after, 43_000,
                    "must flush the model-confirmed count, not the 66k estimate"
                );
            }
            other => panic!("expected deferred CompactionCompleted, got {other:?}"),
        }
    }

    #[test]
    fn apply_compaction_completed_falls_back_to_estimate_without_confirmation() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        let update = XaiSessionUpdate::AutoCompactCompleted {
            tokens_before: Some(90_000),
            tokens_after: 20_000,
            elapsed_ms: Some(500),
            summary_preview: None,
        };
        assert!(apply_session_event(&update, &mut session, &mut scrollback, false));
        session.finish_turn(&mut scrollback,
        );
        match last_session_event(&scrollback) {
            Some(SessionEvent::CompactionCompleted { tokens_after, .. }) => {
                assert_eq!(
                    tokens_after, 20_000,
                    "fallback to estimate when unconfirmed"
                );
            }
            other => panic!("expected fallback CompactionCompleted, got {other:?}"),
        }
    }

    #[test]
    fn apply_compaction_completed_renders_immediately_during_replay() {
        let mut session = make_session(Some("s1"));
        session.loading_replay = true;
        let mut scrollback = ScrollbackState::new();
        let update = XaiSessionUpdate::AutoCompactCompleted {
            tokens_before: Some(90_000),
            tokens_after: 20_000,
            elapsed_ms: Some(500),
            summary_preview: None,
        };
        assert!(apply_session_event(&update, &mut session, &mut scrollback, false));
        match last_session_event(&scrollback) {
            Some(SessionEvent::CompactionCompleted { tokens_after, .. }) => {
                assert_eq!(
                    tokens_after, 20_000,
                    "replay renders the recorded count immediately"
                );
            }
            other => panic!("expected immediate CompactionCompleted on replay, got {other:?}"),
        }
    }

    #[test]
    fn deferred_compaction_flushes_confirmed_count_over_estimate_refresh() {
        let mut agent = make_agent(Some("s1"));
        agent
            .session
            .set_compaction_activity(Some(TurnActivity::AutoCompacting));

        let update = XaiSessionUpdate::AutoCompactCompleted {
            tokens_before: Some(858_000),
            tokens_after: 66_000,
            elapsed_ms: Some(500),
            summary_preview: None,
        };
        assert!(apply_session_event(
            &update,
            &mut agent.session,
            &mut agent.scrollback, false));

        refresh_context_used(&mut agent, 66_000);
        confirm_context_used(&mut agent, 43_000);

        agent.session.finish_turn(&mut agent.scrollback,
        );
        match last_session_event(&agent.scrollback) {
            Some(SessionEvent::CompactionCompleted {
                tokens_before,
                tokens_after,
                ..
            }) => {
                assert_eq!(tokens_before, Some(858_000));
                assert_eq!(
                    tokens_after, 43_000,
                    "deferred line must flush the confirmed 43k, not the 66k \
                     estimate refresh that updated the bar first"
                );
            }
            other => panic!("expected deferred CompactionCompleted, got {other:?}"),
        }
    }

    #[test]
    fn apply_unhandled_event_returns_false() {
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();
        let update = XaiSessionUpdate::MemoryFlushStarted;
        assert!(!apply_session_event(&update, &mut session, &mut scrollback, false));
    }

    // ── handle_child_session_notification ──────────────────────────────

    #[test]
    fn child_compact_completed_updates_subagent_info() {
        let mut agent = make_agent(Some("root-sess"));
        let child_sid = "child-sess-1";
        agent
            .subagent_sessions
            .insert(child_sid.into(), make_subagent_info(child_sid));
        let child_view = make_agent(Some(child_sid));
        agent
            .subagent_views
            .insert(child_sid.into(), Box::new(child_view));

        let update = XaiSessionUpdate::AutoCompactCompleted {
            tokens_before: Some(90000),
            tokens_after: 25000,
            elapsed_ms: Some(300),
            summary_preview: None,
        };
        let changed = handle_child_session_notification(update, child_sid, &mut agent, false);
        assert!(changed);

        let info = agent.subagent_sessions.get(child_sid).unwrap();
        assert_eq!(info.tokens_used, Some(25000));
        // 25000 / 131072 * 100 ~= 19
        assert_eq!(info.context_usage_pct, Some(19));

        // The child view's context_state.used (context-bar numerator) must
        // also be reset — see the comment in handle_child_session_notification.
        let child_view = agent.subagent_views.get(child_sid).unwrap();
        assert_eq!(
            child_view.context_state.as_ref().map(|c| c.used),
            Some(25000)
        );
    }

    #[test]
    fn child_compact_started_does_not_reset_context_used() {
        // Sibling variants in the same outer arm must not touch the numerator;
        // guards against accidental widening of the AutoCompactCompleted gate.
        let mut agent = make_agent(Some("root-sess"));
        let child_sid = "child-sess-3";
        agent
            .subagent_sessions
            .insert(child_sid.into(), make_subagent_info(child_sid));
        let mut child_view = make_agent(Some(child_sid));
        child_view.context_state = Some(xai_grok_shell::session::ContextInfo::from_notification(
            90_000, 131_072,
        ));
        agent
            .subagent_views
            .insert(child_sid.into(), Box::new(child_view));

        let update = XaiSessionUpdate::AutoCompactStarted {
            tokens_used: 95_000,
            context_window: 131_072,
            percentage: 72,
            reason: "threshold".into(),
        };
        let _ = handle_child_session_notification(update, child_sid, &mut agent, false);

        let child_view = agent.subagent_views.get(child_sid).unwrap();
        assert_eq!(
            child_view.context_state.as_ref().map(|c| c.used),
            Some(90_000)
        );
    }

    #[test]
    fn child_notification_without_view_returns_false() {
        let mut agent = make_agent(Some("root-sess"));
        // No child view registered.
        let update = XaiSessionUpdate::AutoCompactStarted {
            tokens_used: 90000,
            context_window: 131072,
            percentage: 85,
            reason: "threshold".into(),
        };
        let changed = handle_child_session_notification(update, "unknown-child", &mut agent, false);
        assert!(!changed);
    }

    #[test]
    fn child_compact_completed_without_view_returns_false() {
        let mut agent = make_agent(Some("root-sess"));
        let child_sid = "child-sess-2";
        // SubagentInfo exists but no child view (race between notification and spawn).
        agent
            .subagent_sessions
            .insert(child_sid.into(), make_subagent_info(child_sid));

        let update = XaiSessionUpdate::AutoCompactCompleted {
            tokens_before: Some(90000),
            tokens_after: 25000,
            elapsed_ms: Some(300),
            summary_preview: None,
        };
        let changed = handle_child_session_notification(update, child_sid, &mut agent, false);
        // No child_view means nothing visible changed — must not trigger redraw.
        assert!(!changed);
        // SubagentInfo should still be updated (data correctness).
        let info = agent.subagent_sessions.get(child_sid).unwrap();
        assert_eq!(info.tokens_used, Some(25000));
        assert_eq!(info.context_usage_pct, Some(19));
    }

    #[test]
    fn child_unknown_event_returns_false() {
        let mut agent = make_agent(Some("root-sess"));
        let update = XaiSessionUpdate::MemoryFlushStarted;
        let changed = handle_child_session_notification(update, "child-1", &mut agent, false);
        assert!(!changed);
    }

    // ── apply_retry_state ─────────────────────────────────────────────

    #[test]
    fn retry_failed_encrypted_content_sets_model_incompatible() {
        use xai_grok_shell::extensions::notification::RetryState;
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();

        assert!(!session.model_incompatible);
        apply_retry_state(
            &RetryState::Failed {
                error_type: "encrypted_content_mismatch".into(),
                message: "incompatible history".into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            session.model_incompatible,
            "encrypted_content_mismatch should set model_incompatible flag"
        );
    }

    #[test]
    fn retry_failed_other_type_does_not_set_model_incompatible() {
        use xai_grok_shell::extensions::notification::RetryState;
        let mut session = make_session(Some("s1"));
        let mut scrollback = ScrollbackState::new();

        apply_retry_state(
            &RetryState::Failed {
                error_type: "api_400".into(),
                message: "bad request".into(),
                provider: None,
            },
            &mut session,
            &mut scrollback, false);
        assert!(
            !session.model_incompatible,
            "non-encrypted_content error types must not set model_incompatible"
        );
    }
