use crate::auth::AuthMode;
use crate::auth::GrokAuth;
use crate::auth::token_output::parse_token_output;
use crate::util::subprocess::CommandLog;
use crate::util::subprocess::RunError;
use crate::util::subprocess::RunOptions;
use crate::util::subprocess::run_detached_with_timeout;
use crate::util::subprocess::shell_c;
use std::time::Duration;

/// Parse stdout into a session-credential `GrokAuth`.
pub(crate) fn parse_output(output: &std::process::Output) -> anyhow::Result<GrokAuth> {
    let parsed = parse_token_output(output)?;
    Ok(GrokAuth {
        key: parsed.access_token,
        auth_mode: AuthMode::External,
        create_time: chrono::Utc::now(),
        user_id: String::new(),
        email: None,
        first_name: None,
        last_name: None,
        profile_image_asset_id: None,
        principal_type: None,
        principal_id: None,
        team_id: None,
        team_name: None,
        team_role: None,
        organization_id: None,
        organization_name: None,
        organization_role: None,
        user_blocked_reason: None,
        team_blocked_reasons: vec![],
        coding_data_retention_opt_out: crate::auth::default_coding_data_retention_opt_out(),
        has_grok_code_access: None,
        refresh_token: parsed.refresh_token,
        expires_at: parsed.expires_at,
        oidc_issuer: parsed.issuer,
        oidc_client_id: None,
    })
}

/// Short timeout for a mid-session refresh: it must not hang the session.
/// The single external-provider run gets this entire budget.
const EXTERNAL_AUTH_REFRESH_TIMEOUT: Duration = Duration::from_secs(7);

/// Runs the external auth binary for a headless mid-session refresh. Initial,
/// interactive sign-in takes a separate path (`flow::run_external_auth_provider`,
/// which bridges the provider's stderr link), so this handles refresh only.
pub(crate) async fn run_external_refresh(command: &str) -> Option<GrokAuth> {
    tracing::info!(cmd = %command, timeout_secs = EXTERNAL_AUTH_REFRESH_TIMEOUT.as_secs(), "auth: running external auth provider (headless refresh)");

    let mut cmd = shell_c(command);
    cmd.env("GROK_AUTH_EXPIRED", "1");
    // Route through the group-killing runner so a provider that spawns helpers
    // is torn down as a unit on timeout.
    let output = match run_detached_with_timeout(
        cmd,
        EXTERNAL_AUTH_REFRESH_TIMEOUT,
        RunOptions {
            label: "external auth provider",
            command_log: CommandLog::Shown(command),
        },
    )
    .await
    {
        Ok(output) => output,
        Err(e) => {
            let reason = match e {
                RunError::TimedOut => {
                    "timed out (a timeout usually means it needs interactive sign-in)"
                }
                RunError::SpawnFailed => "failed to start",
                RunError::WaitFailed => "errored while running",
            };
            tracing::warn!(cmd = %command, "auth: external auth provider {reason}");
            return None;
        }
    };

    match parse_output(&output) {
        Ok(auth) => {
            tracing::info!("auth: external auth provider returned fresh token");
            Some(auth)
        }
        Err(e) => {
            tracing::warn!(error = %e, "auth: external auth provider failed");
            None
        }
    }
}

/// Run external auth provider, carrying forward `/user`-derived fields from previous auth.
pub(crate) async fn refresh_with_command(command: &str, prev_auth: &GrokAuth) -> Option<GrokAuth> {
    let mut auth = run_external_refresh(command).await?;
    auth.carry_user_profile_from(prev_auth);
    Some(auth)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn command_output(command: &str, stdout: &[u8]) -> std::process::Output {
        let mut output = shell_c(command).output().await.unwrap();
        output.stdout = stdout.to_vec();
        output.stderr.clear();
        output
    }

    async fn successful_output(stdout: &str) -> std::process::Output {
        command_output("exit 0", stdout.as_bytes()).await
    }

    #[tokio::test]
    async fn parse_output_nonzero_exit_is_err() {
        let output = command_output("exit 1", b"token").await;
        assert!(parse_output(&output).is_err());
    }

    #[tokio::test]
    async fn parse_output_empty_stdout_is_err() {
        assert!(parse_output(&successful_output("  \n").await).is_err());
    }

    #[tokio::test]
    async fn parse_output_issuer_claim_enables_xai_auth() {
        // x.ai issuer claim → first-party session (relay-eligible).
        let auth = parse_output(
            &successful_output(
                r#"{"access_token":"t","expires_in":900,"issuer":"https://auth.x.ai"}"#,
            )
            .await,
        )
        .unwrap();
        assert_eq!(auth.oidc_issuer.as_deref(), Some("https://auth.x.ai"));
        assert!(auth.is_xai_auth());

        // Non-x.ai issuer is stored but stays third-party.
        let auth = parse_output(
            &successful_output(r#"{"access_token":"t","issuer":"https://idp.acme.example"}"#).await,
        )
        .unwrap();
        assert_eq!(
            auth.oidc_issuer.as_deref(),
            Some("https://idp.acme.example")
        );
        assert!(!auth.is_xai_auth());

        // Missing / empty / whitespace issuer → None.
        let auth = parse_output(&successful_output(r#"{"access_token":"t"}"#).await).unwrap();
        assert_eq!(auth.oidc_issuer, None);
        assert!(!auth.is_xai_auth());
        let auth = parse_output(&successful_output(r#"{"access_token":"t","issuer":"  "}"#).await)
            .unwrap();
        assert_eq!(auth.oidc_issuer, None);

        // Bare-token output never carries an issuer.
        let auth = parse_output(&successful_output("bare-token").await).unwrap();
        assert_eq!(auth.oidc_issuer, None);
        assert!(!auth.is_xai_auth());
    }

    #[tokio::test]
    async fn parse_output_json_shaped_but_invalid_is_err() {
        assert!(parse_output(&successful_output("{not valid json}").await).is_err());
    }

    #[tokio::test]
    async fn spawn_failure_returns_none() {
        assert!(run_external_refresh("/nonexistent/binary").await.is_none());
    }

    #[tokio::test]
    async fn sets_grok_auth_expired_env_on_refresh() {
        let command = if cfg!(windows) {
            "echo %GROK_AUTH_EXPIRED%"
        } else {
            "echo $GROK_AUTH_EXPIRED"
        };
        let auth = run_external_refresh(command).await.unwrap();
        assert_eq!(auth.key, "1");
    }

    #[tokio::test]
    async fn refresh_carries_zdr_flags_forward() {
        let prev = GrokAuth {
            user_blocked_reason: Some("BLOCKED_REASON_OTHER".into()),
            team_blocked_reasons: vec!["BLOCKED_REASON_NO_LOGS".into()],
            coding_data_retention_opt_out: true,
            organization_id: Some("org-1".into()),
            ..GrokAuth::test_default()
        };
        let auth = refresh_with_command("echo fresh-token", &prev)
            .await
            .unwrap();
        assert_eq!(auth.key, "fresh-token");
        assert!(auth.is_zdr_team(), "ZDR flag must survive refresh");
        assert!(auth.coding_data_retention_opt_out);
        assert_eq!(
            auth.user_blocked_reason.as_deref(),
            Some("BLOCKED_REASON_OTHER")
        );
        assert_eq!(auth.user_id, "test-user", "profile must survive refresh");
        assert_eq!(auth.organization_id.as_deref(), Some("org-1"));
    }

    #[tokio::test]
    async fn refresh_interactive_times_out_after_one_seven_second_attempt() {
        let temp = tempfile::tempdir().unwrap();
        let count_path = temp.path().join("external-auth-count.txt");
        // Binary blocks for interaction; the 7s refresh timeout kills it. The
        // marker proves the provider was launched exactly once.
        let cmd = if cfg!(windows) {
            let count = count_path.to_string_lossy();
            format!(r#">"{count}" echo run & ping 127.0.0.1 -n 20 >NUL && echo token"#)
        } else {
            let count = count_path.to_string_lossy();
            format!(
                r#"echo run >> '{}'; echo 'Visit http://example.com/auth' >&2; sleep 20; echo token"#,
                count.replace('\\', "\\\\").replace('\'', "'\\''")
            )
        };
        let start = std::time::Instant::now();
        let result = run_external_refresh(&cmd).await;
        let elapsed = start.elapsed();
        assert!(result.is_none(), "should timeout and return None");
        assert_eq!(
            std::fs::read_to_string(&count_path)
                .unwrap()
                .lines()
                .count(),
            1,
            "the refresh provider must run exactly once"
        );
        assert!(
            elapsed >= EXTERNAL_AUTH_REFRESH_TIMEOUT.saturating_sub(Duration::from_secs(1)),
            "refresh returned before its 7s timeout budget (took {elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_secs(12),
            "refresh should use one 7s attempt, not an interactive timeout (took {elapsed:?})"
        );
    }
}
