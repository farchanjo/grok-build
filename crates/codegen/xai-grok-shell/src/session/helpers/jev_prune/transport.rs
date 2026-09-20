//! Jev transports: the two wires, kept as data.
//!
//! The native TypeSafe evaluation endpoint and OpenRouter's alpha decisions
//! proxy speak the same request and answer shape and differ in exactly four
//! things: base URL, model string, whether a `provider` routing block is
//! accepted, and which credential names resolve. Keeping those four as data is
//! what stops the mismatch that motivates the enum: setting one field without
//! the others fails confusingly, because the native endpoint with the
//! `~typesafe/` model prefix 404s and the OpenRouter endpoint with a bare
//! `jev-latest` fails model resolution.
//!
//! Both transports were probed on 2026-09-19 against `jev-1.13`: same model
//! behind, identical answers, `choice` and `score` accepted on both.

use std::path::{Path, PathBuf};

use super::types::PruneError;

/// Environment override for the key file consulted by the native chain.
pub const JEV_KEY_FILE_ENV: &str = "LLM_KEY_FILE";
/// Default key file name under `$HOME`. A shell-sourceable list of
/// `export NAME="value"` lines, maintained by the user; unrelated exports are
/// expected and skipped.
pub const DEFAULT_KEY_FILE: &str = ".llm-key";
/// Native credential names, in precedence order.
///
/// `TYPESAFE_API_KEY` is the documented SDK name; `TYPESAFE_AI_FABRICIO_KEY`
/// is the name actually present in the harness key file, so the chain accepts
/// both — resolving only one of them is the difference between "works" and
/// "401 with no explanation".
pub const NATIVE_KEY_NAMES: [&str; 2] = ["TYPESAFE_API_KEY", "TYPESAFE_AI_FABRICIO_KEY"];
/// OpenRouter credential name consulted before `auth.json`.
pub const OPENROUTER_KEY_NAMES: [&str; 1] = ["GROK_JEV_API_KEY"];

/// Which wire the decisions call rides.
///
/// The default is [`JevTransport::Openrouter`], which is what every existing
/// `[compaction.jev]` user already gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JevTransport {
    /// TypeSafe's documented evaluation endpoint. No `provider` block.
    Native,
    /// OpenRouter's alpha decisions proxy. Default.
    #[default]
    Openrouter,
}

impl JevTransport {
    /// Both transports, in display order.
    pub const ALL: [Self; 2] = [Self::Openrouter, Self::Native];

    /// Stable label used in config, logs and the `/jev status` line.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Openrouter => "openrouter",
        }
    }

    /// Parse the config / command spelling. Case-insensitive; `router` and
    /// `open_router` are accepted as aliases.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "native" | "typesafe" => Some(Self::Native),
            "openrouter" | "open_router" | "router" => Some(Self::Openrouter),
            _ => None,
        }
    }

    /// Decisions endpoint that belongs to this transport.
    pub const fn endpoint(self) -> &'static str {
        match self {
            Self::Native => "https://api.typesafe.ai/v1/systemone",
            Self::Openrouter => "https://openrouter.ai/api/alpha/decisions",
        }
    }

    /// Model reference that belongs to this transport. Not interchangeable:
    /// the prefix is what the endpoint resolves.
    pub const fn model(self) -> &'static str {
        match self {
            Self::Native => "jev-latest",
            Self::Openrouter => "~typesafe/jev-latest",
        }
    }

    /// Whether the `provider` routing block is sent. OpenRouter-only: the
    /// native endpoint rejects unknown top-level fields.
    pub const fn sends_provider_block(self) -> bool {
        match self {
            Self::Native => false,
            Self::Openrouter => true,
        }
    }

    /// Credential names tried, in order, before the profile store.
    pub const fn key_names(self) -> &'static [&'static str] {
        match self {
            Self::Native => &NATIVE_KEY_NAMES,
            Self::Openrouter => &OPENROUTER_KEY_NAMES,
        }
    }
}

/// Where a resolved credential came from, never its value.
///
/// This is what makes a two-transport setup debuggable: a 401 names the chain
/// that was tried instead of leaving the user to guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    /// Environment variable.
    Env(String),
    /// Key file entry (`file:<name>:<VAR>`).
    File { path: PathBuf, name: String },
    /// `auth.json` provider scope.
    AuthJson(&'static str),
}

impl CredentialSource {
    /// One-line label for `status` and error messages. Never the key itself.
    pub fn label(&self) -> String {
        match self {
            Self::Env(name) => format!("env:{name}"),
            Self::File { path, name } => format!("file:{}:{name}", key_file_label(path)),
            Self::AuthJson(scope) => format!("auth.json:{scope}"),
        }
    }
}

/// File name of a key file, for the `file:` label.
fn key_file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl std::fmt::Display for CredentialSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

/// A credential and the chain link that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCredential {
    pub value: String,
    pub source: CredentialSource,
}

/// Key file consulted by the native chain, honouring [`JEV_KEY_FILE_ENV`].
pub fn key_file_path() -> PathBuf {
    match std::env::var(JEV_KEY_FILE_ENV) {
        Ok(path) if !path.trim().is_empty() => PathBuf::from(path.trim()),
        _ => home_dir().join(DEFAULT_KEY_FILE),
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .ok()
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Parse an `export NAME="value"` list. Unparseable lines are skipped rather
/// than fatal: the file is hand-maintained and also holds unrelated exports.
fn parse_key_file(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (name, raw) = line.split_once('=')?;
            let name = name.trim();
            let value = raw.trim().trim_matches('"').trim_matches('\'').trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some((name.to_owned(), value.to_owned()))
        })
        .collect()
}

fn from_env(name: &'static str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Resolve the credential for one transport.
///
/// Chain, in order: an explicit `api_key_env` override, the transport's own
/// environment names, the native key file, then the OpenRouter scope of
/// `auth.json`. A missing credential is [`PruneError::MissingCredential`]
/// carrying the whole chain, so the message names every link that was tried.
pub fn resolve_credential(
    transport: JevTransport,
    api_key_env: Option<&str>,
    grok_home: &Path,
) -> Result<ResolvedCredential, PruneError> {
    let mut tried: Vec<String> = Vec::new();
    if let Some(name) = api_key_env.map(str::trim).filter(|n| !n.is_empty()) {
        tried.push(format!("env:{name}"));
        if let Some(value) = std::env::var(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
        {
            return Ok(ResolvedCredential {
                value,
                source: CredentialSource::Env(name.to_owned()),
            });
        }
    }
    for name in transport.key_names() {
        tried.push(format!("env:{name}"));
        if let Some(value) = from_env(name) {
            return Ok(ResolvedCredential {
                value,
                source: CredentialSource::Env((*name).to_owned()),
            });
        }
    }
    // The key file only serves the native chain; an OpenRouter key living
    // there under the same names would be a different value entirely.
    if transport == JevTransport::Native {
        let path = key_file_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            let entries = parse_key_file(&text);
            for name in transport.key_names() {
                tried.push(format!("file:{}:{name}", key_file_label(&path)));
                if let Some((_, value)) = entries.iter().find(|(key, _)| key == name) {
                    return Ok(ResolvedCredential {
                        value: value.clone(),
                        source: CredentialSource::File {
                            path,
                            name: (*name).to_owned(),
                        },
                    });
                }
            }
        }
    }
    tried.push("auth.json:openrouter".to_owned());
    if let Ok(Some(value)) =
        crate::auth::read_provider_api_key(grok_home, crate::auth::OPENROUTER_API_KEY_SCOPE)
    {
        let value = value.trim().to_owned();
        if !value.is_empty() {
            return Ok(ResolvedCredential {
                value,
                source: CredentialSource::AuthJson("openrouter"),
            });
        }
    }
    Err(PruneError::MissingCredential {
        chain: tried.join(" -> "),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_and_model_are_a_pair() {
        for transport in JevTransport::ALL {
            // The two must never be mixed: this is the 404 the enum prevents.
            let model = transport.model();
            assert_eq!(
                model.starts_with("~typesafe/"),
                transport == JevTransport::Openrouter,
                "{model} belongs to {}",
                transport.as_str()
            );
        }
    }

    #[test]
    fn provider_block_is_openrouter_only() {
        assert!(JevTransport::Openrouter.sends_provider_block());
        assert!(!JevTransport::Native.sends_provider_block());
    }

    #[test]
    fn default_transport_is_openrouter() {
        assert_eq!(JevTransport::default(), JevTransport::Openrouter);
    }

    #[test]
    fn parse_accepts_aliases_and_rejects_junk() {
        assert_eq!(JevTransport::parse(" NATIVE "), Some(JevTransport::Native));
        assert_eq!(JevTransport::parse("typesafe"), Some(JevTransport::Native));
        assert_eq!(
            JevTransport::parse("OpenRouter"),
            Some(JevTransport::Openrouter)
        );
        assert_eq!(
            JevTransport::parse("open_router"),
            Some(JevTransport::Openrouter)
        );
        assert_eq!(JevTransport::parse("bogus"), None);
    }

    #[test]
    fn native_chain_accepts_both_documented_names() {
        assert_eq!(
            JevTransport::Native.key_names(),
            ["TYPESAFE_API_KEY", "TYPESAFE_AI_FABRICIO_KEY"]
        );
        assert_eq!(JevTransport::Openrouter.key_names(), ["GROK_JEV_API_KEY"]);
    }

    #[test]
    fn key_file_parses_exports_quotes_and_junk() {
        let entries = parse_key_file(
            "# comment\n\
             export TYPESAFE_API_KEY=\"abc\"\n\
             TYPESAFE_AI_FABRICIO_KEY='def'\n\
             OTHER=ghi\n\
             not a line\n\
             EMPTY=\n",
        );
        assert_eq!(
            entries,
            vec![
                ("TYPESAFE_API_KEY".to_owned(), "abc".to_owned()),
                ("TYPESAFE_AI_FABRICIO_KEY".to_owned(), "def".to_owned()),
                ("OTHER".to_owned(), "ghi".to_owned()),
            ]
        );
    }

    #[test]
    fn credential_source_label_never_carries_the_key() {
        let label = CredentialSource::File {
            path: PathBuf::from("/home/u/.llm-key"),
            name: "TYPESAFE_API_KEY".to_owned(),
        }
        .label();
        assert_eq!(label, "file:.llm-key:TYPESAFE_API_KEY");
        assert_eq!(
            CredentialSource::AuthJson("openrouter").label(),
            "auth.json:openrouter"
        );
        assert_eq!(
            CredentialSource::Env("GROK_JEV_API_KEY".to_owned()).label(),
            "env:GROK_JEV_API_KEY"
        );
    }

    /// The key file is the native chain's second link: an entry there
    /// resolves even with no environment variable set.
    #[test]
    fn native_chain_reads_the_key_file_after_the_environment() {
        let dir = tempfile::tempdir().unwrap();
        let key_file = dir.path().join(".llm-key");
        std::fs::write(&key_file, "export TYPESAFE_AI_FABRICIO_KEY=\"from-file\"\n").unwrap();
        let resolved = with_clean_env(&key_file, || {
            resolve_credential(JevTransport::Native, None, dir.path()).unwrap()
        });
        assert_eq!(resolved.value, "from-file");
        assert_eq!(
            resolved.source.label(),
            "file:.llm-key:TYPESAFE_AI_FABRICIO_KEY"
        );
    }

    /// The environment wins over the key file, so a one-off override does not
    /// require editing the file.
    #[test]
    fn environment_wins_over_the_key_file() {
        let dir = tempfile::tempdir().unwrap();
        let key_file = dir.path().join(".llm-key");
        std::fs::write(&key_file, "TYPESAFE_API_KEY=from-file\n").unwrap();
        let resolved = with_clean_env(&key_file, || {
            // SAFETY: the test runtime is single-threaded per test process.
            unsafe { std::env::set_var("TYPESAFE_API_KEY", "from-env") };
            resolve_credential(JevTransport::Native, None, dir.path()).unwrap()
        });
        assert_eq!(resolved.value, "from-env");
        assert_eq!(resolved.source.label(), "env:TYPESAFE_API_KEY");
    }

    #[test]
    fn missing_credential_names_the_whole_chain() {
        // A blank override plus a temp home with no auth.json and no key file.
        let dir = tempfile::tempdir().unwrap();
        let key_file = dir.path().join("no-such-key-file");
        let error = with_clean_env(&key_file, || {
            resolve_credential(
                JevTransport::Native,
                Some("JEV_TEST_ABSENT_KEY"),
                dir.path(),
            )
            .unwrap_err()
        });
        match error {
            PruneError::MissingCredential { chain } => {
                assert!(chain.contains("JEV_TEST_ABSENT_KEY"), "{chain}");
                assert!(chain.contains("TYPESAFE_API_KEY"), "{chain}");
                assert!(chain.contains("TYPESAFE_AI_FABRICIO_KEY"), "{chain}");
                assert!(chain.contains("auth.json:openrouter"), "{chain}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    /// Run `body` with the key-file override pointed at `key_file` and every
    /// credential variable the chain reads removed, so the developer's own
    /// environment cannot decide the outcome.
    fn with_clean_env<T>(key_file: &Path, body: impl FnOnce() -> T) -> T {
        const NAMES: [&str; 3] = [
            "TYPESAFE_API_KEY",
            "TYPESAFE_AI_FABRICIO_KEY",
            "JEV_TEST_ABSENT_KEY",
        ];
        let saved: Vec<(&str, Option<String>)> = NAMES
            .iter()
            .map(|name| (*name, std::env::var(name).ok()))
            .collect();
        // SAFETY: the test runtime is single-threaded per test process.
        unsafe {
            std::env::set_var(JEV_KEY_FILE_ENV, key_file);
            for name in NAMES {
                std::env::remove_var(name);
            }
        }
        let out = body();
        // SAFETY: as above.
        unsafe {
            for (name, value) in saved {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
        out
    }
}
