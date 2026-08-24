//! `grok skills validate` / `grok skills regress` — hermetic JSON/text CLI.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use clap::Subcommand;
use serde::Serialize;
use xai_grok_tools::implementations::skills::strict::{
    LocalSkillEvidence, SKILLS_API_VERSION, SkillHealthStatus, SkillIdentity, StrictSkillOutcome,
    load_eval_suite_from_dir, run_eval_suite, validate_strict_skill_dir,
};

#[derive(Debug, clap::Args, Clone)]
pub struct SkillsCliArgs {
    #[command(subcommand)]
    pub command: SkillsCliCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SkillsCliCommand {
    /// Strict-validate a skill directory and print JSON or text.
    Validate {
        /// Skill directory (contains SKILL.md). Defaults to the current directory.
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Run local offline evals/cases.yaml twice and print JSON or text.
    Regress {
        /// Skill directory (contains SKILL.md and optional evals/cases.yaml).
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidateJson {
    api_version: u32,
    status: SkillHealthStatus,
    identity: SkillIdentity,
    diagnostics: Vec<xai_grok_tools::implementations::skills::strict::SkillDiagnostic>,
}

/// Returns a process exit code: 0 success, 1 validation/regress failure, 2 usage.
pub fn run(args: SkillsCliArgs) -> Result<i32> {
    match args.command {
        SkillsCliCommand::Validate { path, json } => run_validate(path, json),
        SkillsCliCommand::Regress { path, json } => run_regress(path, json),
    }
}

fn resolve_path(path: Option<PathBuf>) -> Result<PathBuf> {
    let path =
        path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if !path.exists() {
        anyhow::bail!("skill path does not exist");
    }
    Ok(path)
}

fn run_validate(path: Option<PathBuf>, json: bool) -> Result<i32> {
    let path = resolve_path(path)?;
    let dir = if path.is_file() {
        path.parent().unwrap_or(&path).to_path_buf()
    } else {
        path
    };
    let outcome = validate_strict_skill_dir(&dir, None);
    match outcome {
        StrictSkillOutcome::Valid(discovered) => {
            let payload = ValidateJson {
                api_version: SKILLS_API_VERSION,
                status: SkillHealthStatus::Untested,
                identity: discovered.identity,
                diagnostics: Vec::new(),
            };
            emit(json, &payload, "untested")?;
            Ok(0)
        }
        StrictSkillOutcome::Quarantined(row) => {
            let payload = ValidateJson {
                api_version: SKILLS_API_VERSION,
                status: SkillHealthStatus::Quarantined,
                identity: row.identity,
                diagnostics: row.diagnostics,
            };
            emit(json, &payload, "quarantined")?;
            Ok(1)
        }
    }
}

fn run_regress(path: Option<PathBuf>, json: bool) -> Result<i32> {
    let path = resolve_path(path)?;
    let dir = if path.is_file() {
        path.parent().unwrap_or(&path).to_path_buf()
    } else {
        path
    };
    let outcome = validate_strict_skill_dir(&dir, None);
    let StrictSkillOutcome::Valid(discovered) = outcome else {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "apiVersion": SKILLS_API_VERSION,
                    "status": "quarantined",
                })
            );
        } else {
            println!("quarantined");
        }
        return Ok(1);
    };
    let suite = match load_eval_suite_from_dir(&dir) {
        Ok(Some(suite)) => suite,
        Ok(None) => {
            emit_error(json, "evals/cases.yaml is missing")?;
            return Ok(1);
        }
        Err(err) => {
            emit_error(json, &err.message)?;
            return Ok(1);
        }
    };
    let subject = LocalSkillEvidence {
        name: discovered.manifest.name.clone(),
        description: discovered.manifest.description.clone(),
        when_to_use: discovered.manifest.grok.when_to_use.clone(),
        paths: discovered.manifest.grok.paths.clone().unwrap_or_default(),
        short_description: discovered.manifest.grok.short_description.clone(),
    };
    let report = run_eval_suite(
        &suite,
        &subject,
        &[],
        discovered.identity,
        0,
        "cli",
        &AtomicBool::new(false),
    );
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", report.status.as_str());
    }
    Ok(if report.status == SkillHealthStatus::ValidPass {
        0
    } else {
        1
    })
}

fn emit<T: Serialize>(json: bool, payload: &T, text: &str) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(payload)?);
    } else {
        println!("{text}");
    }
    Ok(())
}

fn emit_error(json: bool, message: &str) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "apiVersion": SKILLS_API_VERSION,
                "status": "failed",
                "error": message,
            })
        );
    } else {
        println!("{message}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use xai_grok_tools::implementations::skills::strict::render_skill_md;

    fn skill_dir(tmp: &Path, name: &str, description: &str, cases: Option<&str>) -> PathBuf {
        let dir = tmp.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            render_skill_md(name, description, "# Body\n"),
        )
        .unwrap();
        if let Some(yaml) = cases {
            std::fs::create_dir_all(dir.join("evals")).unwrap();
            std::fs::write(dir.join("evals/cases.yaml"), yaml).unwrap();
        }
        dir
    }

    #[test]
    fn validate_json_exit_codes() {
        let tmp = tempfile::tempdir().unwrap();
        let good = skill_dir(
            tmp.path(),
            "commit",
            "Create well-formatted git commits.",
            None,
        );
        assert_eq!(run_validate(Some(good), true).unwrap(), 0);
        let bad = tmp.path().join("bad");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("SKILL.md"), "nope\n").unwrap();
        assert_eq!(run_validate(Some(bad), true).unwrap(), 1);
    }

    #[test]
    fn regress_json_pass_and_fail() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
version: 1
cases:
  - id: pin
    kind: explicit_pin
    skill: commit
"#;
        let good = skill_dir(
            tmp.path(),
            "commit",
            "Create well-formatted git commits.",
            Some(yaml),
        );
        assert_eq!(run_regress(Some(good), true).unwrap(), 0);
        let miss = skill_dir(
            tmp.path(),
            "review",
            "Review pull requests with care.",
            None,
        );
        assert_eq!(run_regress(Some(miss), true).unwrap(), 1);
    }
}
