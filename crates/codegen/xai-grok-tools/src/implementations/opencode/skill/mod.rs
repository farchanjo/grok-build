//! `skill` tool — OpenCode variant of the skill tool.
//!
//! Loads a user-defined skill by name, reads its `SKILL.md` content,
//! lists up to 10 bundled files from the skill directory, and returns
//! the result wrapped in `<skill_content>` XML.

use std::path::Path;

use crate::implementations::skills::skill::{SkillOutput, extract_skill_body, format_skill_name};
use crate::implementations::skills::types::SkillInfo;
use crate::types::requirements::{Expr, ToolRequirement};
#[allow(unused_imports)]
use crate::types::resources::{AvailableSkills, SharedResources};
use crate::types::tool::{ToolKind, ToolNamespace};

// ─── Description ─────────────────────────────────────────────────────

const DESCRIPTION: &str = r#"Load a specialized skill that provides domain-specific instructions and workflows.

When you recognize that a task matches one of the available skills listed below, use this tool to load the full skill instructions.

The skill will inject detailed instructions, workflows, and access to bundled resources into the conversation via a `<skill_content name="...">` block with the loaded content.

The following skills provide specialized sets of instructions for particular tasks.
Invoke this tool to load a skill when a task matches one of the available skills listed below:

<available_skills>
${%- if skills %}
${%- for skill in skills %}
  <skill>
    <name>${{ skill.name }}</name>
    <description>${{ skill.description|e }}</description>
    <location>${{ skill.location }}</location>
  </skill>
${%- endfor %}
${%- else %}
(No skills available. Skills can be added in ~/.grok/skills/ or .grok/skills/)
${%- endif %}
</available_skills>"#;

// ─── Input ───────────────────────────────────────────────────────────

/// Input for the OpenCode `skill` tool.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SkillInput {
    /// The name of the skill to invoke
    #[schemars(description = "The name of the skill to invoke")]
    pub name: String,
}

// ─── ToolInput conversions (via Dynamic variant) ─────────────────────

impl TryFrom<crate::types::tool_io::ToolInput> for SkillInput {
    type Error = String;
    fn try_from(value: crate::types::tool_io::ToolInput) -> Result<Self, Self::Error> {
        match value {
            crate::types::tool_io::ToolInput::Dynamic(v) => {
                serde_json::from_value(v).map_err(|e| format!("SkillInput: {e}"))
            }
            _ => Err("expected Dynamic variant for SkillInput".into()),
        }
    }
}

impl From<SkillInput> for crate::types::tool_io::ToolInput {
    fn from(value: SkillInput) -> Self {
        crate::types::tool_io::ToolInput::Dynamic(
            serde_json::to_value(value).expect("SkillInput serializes to JSON"),
        )
    }
}

// ─── Tool ────────────────────────────────────────────────────────────

/// OpenCode skill tool — loads specialized skill instructions into the conversation.
#[derive(Debug, Default)]
pub struct SkillTool;

/// Result of looking up a skill by name.
#[derive(Debug)]
enum FindSkillResult<'a> {
    /// Exactly one match found.
    Found(&'a SkillInfo),
    /// Multiple skills share the same short name -- caller must use a qualified name.
    Ambiguous(Vec<String>),
    /// No skill matched.
    NotFound,
}

/// Find a skill by name from the available skills list.
///
/// Supports both fully-qualified names (`"local:commit"`) and
/// short names (`"commit"`).
///
/// This is the **model-facing** skill-tool gate: it only returns skills that
/// are eligible for native model invocation ([`SkillInfo::is_native_model_invocable`],
/// i.e. enabled and not `disable_model_invocation`). A
/// `disable_model_invocation` skill is deliberately not invocable through this
/// tool (the model cannot auto-invoke it); user slash-command execution uses a
/// separate path that is not subject to this gate.
///
/// When a short name matches multiple skills across scopes, returns
/// `Ambiguous` with the qualified names so the caller can ask for
/// disambiguation instead of silently picking first-match.
fn find_skill<'a>(name: &str, skills: &'a [SkillInfo]) -> FindSkillResult<'a> {
    // First try exact match with fully qualified name -- always unambiguous.
    if let Some(skill) = skills
        .iter()
        .find(|s| s.is_native_model_invocable() && format_skill_name(s) == name)
    {
        return FindSkillResult::Found(skill);
    }

    // Short-name lookup: collect all matching skills (only invocable ones).
    let matches: Vec<&SkillInfo> = skills
        .iter()
        .filter(|s| s.is_native_model_invocable() && s.name == name)
        .collect();
    match matches.len() {
        0 => FindSkillResult::NotFound,
        1 => FindSkillResult::Found(matches[0]),
        _ => {
            let qualified: Vec<String> = matches.iter().map(|s| format_skill_name(s)).collect();
            FindSkillResult::Ambiguous(qualified)
        }
    }
}

/// Load skill content from its SKILL.md file, stripping YAML frontmatter.
/// Revalidates strictly so a quarantined or swapped file cannot be invoked.
/// The body is the bytes already read from the no-follow fd; the path
/// string is never re-opened.
async fn load_skill_content(skill: &SkillInfo) -> Result<String, String> {
    let loaded = crate::implementations::skills::strict::revalidate_skill_file_at_load(skill)
        .map_err(|e| e.to_string())?;
    Ok(extract_skill_body(&loaded.content))
}

/// List up to `limit` bundled files in the skill directory, excluding SKILL.md.
async fn list_skill_files(skill: &SkillInfo, limit: usize) -> Vec<String> {
    let skill_path = Path::new(&skill.path);
    let dir = match skill_path.parent() {
        Some(d) => d,
        None => return vec![],
    };

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(_) => return vec![],
    };

    let mut files = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        if files.len() >= limit {
            break;
        }
        let path = entry.path();
        // Skip the SKILL.md file itself.
        if path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("SKILL.md"))
        {
            continue;
        }
        files.push(path.display().to_string());
    }
    files
}

/// Format the `<skill_content>` XML output block.
fn format_output(skill: &SkillInfo, content: &str, files: &[String]) -> String {
    let base_dir = Path::new(&skill.path)
        .parent()
        .map(|p| format!("file://{}", p.display()))
        .unwrap_or_default();

    let files_xml: String = files
        .iter()
        .map(|f| format!("<file>{f}</file>"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut out = String::new();
    out.push_str(&format!("<skill_content name=\"{}\">\n", skill.name));
    out.push_str(&format!("# Skill: {}\n\n", skill.name));
    out.push_str(content.trim());
    out.push_str("\n\n");
    out.push_str(&format!("Base directory for this skill: {base_dir}\n"));
    out.push_str(
        "Relative paths in this skill (e.g., scripts/, reference/) are relative to this base directory.\n",
    );
    out.push_str("Note: file list is sampled.\n\n");
    out.push_str("<skill_files>\n");
    out.push_str(&files_xml);
    out.push('\n');
    out.push_str("</skill_files>\n");
    out.push_str("</skill_content>");
    out
}

// ─── Tests ───────────────────────────────────────────────────────────

impl crate::types::tool_metadata::ToolMetadata for SkillTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Skill
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::OpenCode
    }

    fn description_template(&self) -> &str {
        DESCRIPTION
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for SkillTool {
    type Args = SkillInput;
    type Output = SkillOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("skill").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "skill",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.opencode_skill", skip_all, fields(skill_name = %input.name))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: SkillInput,
    ) -> Result<SkillOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;

        let available_skills;
        {
            available_skills = resources
                .lock()
                .await
                .get::<AvailableSkills>()
                .map(|a| a.0.clone())
                .unwrap_or_default();
        }

        // ── Look up the skill ────────────────────────────────────────
        let skill = match find_skill(&input.name, &available_skills) {
            FindSkillResult::Found(s) => s.clone(),
            FindSkillResult::Ambiguous(qualified) => {
                return Ok(SkillOutput {
                    success: false,
                    tool_result: format!(
                        "Skill '{}' is ambiguous -- multiple skills share this name. \
                         Use a qualified name: {}",
                        input.name,
                        qualified.join(", ")
                    ),
                    skill_name: input.name.clone(),
                    skill_message: None,
                    error: Some(format!(
                        "Ambiguous skill '{}': use one of {}",
                        input.name,
                        qualified.join(", ")
                    )),
                });
            }
            FindSkillResult::NotFound => {
                let names: Vec<&str> = available_skills.iter().map(|s| s.name.as_str()).collect();
                let available = if names.is_empty() {
                    "none".to_string()
                } else {
                    names.join(", ")
                };
                return Ok(SkillOutput {
                    success: false,
                    tool_result: format!(
                        "Skill \"{}\" not found. Available skills: {}",
                        input.name, available
                    ),
                    skill_name: input.name.clone(),
                    skill_message: None,
                    error: Some(format!("Skill \"{}\" not found", input.name)),
                });
            }
        };

        // ── Load the skill content ───────────────────────────────────
        let content = match load_skill_content(&skill).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(SkillOutput {
                    success: false,
                    tool_result: format!("Failed to load skill '{}': {}", skill.name, e),
                    skill_name: skill.name.clone(),
                    skill_message: None,
                    error: Some(e),
                });
            }
        };

        // ── List bundled files (up to 10) ────────────────────────────
        let files = list_skill_files(&skill, 10).await;

        // ── Build output ─────────────────────────────────────────────
        let output = format_output(&skill, &content, &files);

        Ok(SkillOutput {
            success: true,
            tool_result: format!("Loaded skill: {}", skill.name),
            skill_name: skill.name.clone(),
            skill_message: Some(output),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tool_metadata::test_ctx;

    use crate::implementations::skills::types::SkillScope;
    use crate::types::resources::Resources;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn official_skill_md(name: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: A test skill called {name}.\n---\n{body}")
    }

    fn write_official_skill(root: &Path, name: &str, body: &str) -> PathBuf {
        let dir = root.join("skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("SKILL.md");
        std::fs::write(&path, official_skill_md(name, body)).unwrap();
        path
    }

    fn make_test_skill(name: &str, scope: SkillScope, path: &str) -> SkillInfo {
        SkillInfo {
            name: name.to_string(),
            display_name: None,
            description: format!("Test skill: {}", name),
            short_description: None,
            author: None,
            argument_hint: None,
            path: path.to_string(),
            scope,
            config_source: None,
            plugin_name: None,
            plugin_version: None,
            plugin_root: None,
            collection_root: None,
            plugin_data: None,
            allowed_tools: None,
            license: None,
            compatibility: None,
            metadata: None,
            model: None,
            effort: None,
            user_invocable: true,
            disable_model_invocation: false,
            when_to_use: None,
            has_user_specified_description: false,
            paths: None,
            enabled: true,
            body: None,
        }
    }

    #[test]
    fn tool_metadata() {
        use crate::types::tool_metadata::ToolMetadata;
        let tool = SkillTool;
        assert_eq!(xai_tool_runtime::Tool::id(&tool).as_str(), "skill");
        assert!(matches!(tool.kind(), ToolKind::Skill));
        assert!(matches!(tool.tool_namespace(), ToolNamespace::OpenCode));
        let desc = tool.description_template();
        assert!(desc.contains("available_skills"));
        assert!(desc.contains("${%- for skill in skills %}"));
        assert!(desc.contains("${{ skill.name }}"));
    }

    #[test]
    fn find_skill_by_short_name() {
        let skills = vec![
            make_test_skill("commit", SkillScope::User, "/path/a"),
            make_test_skill("build", SkillScope::Local, "/path/b"),
        ];

        assert!(
            matches!(find_skill("commit", &skills), FindSkillResult::Found(s) if s.name == "commit")
        );
        assert!(
            matches!(find_skill("build", &skills), FindSkillResult::Found(s) if s.name == "build")
        );
        assert!(matches!(
            find_skill("nonexistent", &skills),
            FindSkillResult::NotFound
        ));
    }

    #[test]
    fn find_skill_excludes_disable_model_invocation() {
        let mut locked = make_test_skill("locked", SkillScope::Local, "/path/locked");
        locked.disable_model_invocation = true;
        let skills = vec![
            locked,
            make_test_skill("commit", SkillScope::User, "/path/a"),
        ];

        // A disable_model_invocation skill is not invocable via the model tool.
        assert!(matches!(
            find_skill("locked", &skills),
            FindSkillResult::NotFound
        ));
        assert!(matches!(
            find_skill("local:locked", &skills),
            FindSkillResult::NotFound
        ));
        // Unrelated eligible skills still resolve.
        assert!(matches!(
            find_skill("commit", &skills),
            FindSkillResult::Found(s) if s.name == "commit"
        ));
    }

    #[test]
    fn find_skill_keeps_user_invocable_false_eligible() {
        // `user_invocable = false` skills remain eligible for model invocation
        // (they are not shown in the skill tool listing, but the tool gate only
        // checks the native-model-invocable predicate).
        let mut auto = make_test_skill("auto", SkillScope::Local, "/path/auto");
        auto.user_invocable = false;
        let skills = vec![auto];
        assert!(
            matches!(find_skill("auto", &skills), FindSkillResult::Found(s) if s.name == "auto")
        );
    }

    #[test]
    fn find_skill_by_qualified_name() {
        let skills = vec![
            make_test_skill("commit", SkillScope::User, "/path/a"),
            make_test_skill("build", SkillScope::Local, "/path/b"),
        ];

        assert!(
            matches!(find_skill("user:commit", &skills), FindSkillResult::Found(s) if s.name == "commit")
        );
        assert!(
            matches!(find_skill("local:build", &skills), FindSkillResult::Found(s) if s.name == "build")
        );
    }

    #[test]
    fn find_skill_ambiguous_short_name_rejected() {
        let skills = vec![
            make_test_skill("commit", SkillScope::Local, "/path/local"),
            make_test_skill("commit", SkillScope::User, "/path/user"),
        ];

        // Short name "commit" matches two scopes -- should be Ambiguous.
        let result = find_skill("commit", &skills);
        match result {
            FindSkillResult::Ambiguous(qualified) => {
                assert!(qualified.contains(&"local:commit".to_string()));
                assert!(qualified.contains(&"user:commit".to_string()));
            }
            other => panic!("Expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn find_skill_qualified_resolves_despite_ambiguous_short_name() {
        let skills = vec![
            make_test_skill("commit", SkillScope::Local, "/path/local"),
            make_test_skill("commit", SkillScope::User, "/path/user"),
        ];

        // Qualified names still resolve unambiguously.
        assert!(matches!(
            find_skill("local:commit", &skills),
            FindSkillResult::Found(s) if s.path == "/path/local"
        ));
        assert!(matches!(
            find_skill("user:commit", &skills),
            FindSkillResult::Found(s) if s.path == "/path/user"
        ));
    }

    #[tokio::test]
    async fn skill_ambiguous_returns_error_output() {
        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![
            make_test_skill("commit", SkillScope::Local, "/path/local"),
            make_test_skill("commit", SkillScope::User, "/path/user"),
        ]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "commit".into(),
            },
        )
        .await
        .unwrap();

        assert!(!output.success);
        assert!(output.tool_result.contains("ambiguous"));
        assert!(output.tool_result.contains("local:commit"));
        assert!(output.tool_result.contains("user:commit"));
        assert!(output.error.is_some());
        assert!(output.skill_message.is_none());
    }

    #[tokio::test]
    async fn skill_not_found_returns_error_output() {
        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "commit",
            SkillScope::User,
            "/path/a",
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "nonexistent".into(),
            },
        )
        .await
        .unwrap();

        assert!(!output.success);
        assert!(output.tool_result.contains("not found"));
        assert!(output.tool_result.contains("commit"));
        assert!(output.error.is_some());
        assert!(output.skill_message.is_none());
    }

    #[tokio::test]
    async fn skill_not_found_with_no_skills_resource() {
        let resources = Resources::new();

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "commit".into(),
            },
        )
        .await
        .unwrap();

        assert!(!output.success);
        assert!(output.tool_result.contains("not found"));
        assert!(output.tool_result.contains("none"));
    }

    #[tokio::test]
    async fn skill_loads_content_from_file() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "test", "# Test Skill\n\nDo the thing.");

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "test",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "test".into(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        assert_eq!(output.skill_name, "test");
        assert_eq!(output.tool_result, "Loaded skill: test");

        let msg = output.skill_message.unwrap();
        assert!(msg.contains("<skill_content name=\"test\">"));
        assert!(msg.contains("# Skill: test"));
        assert!(msg.contains("# Test Skill"));
        assert!(msg.contains("Do the thing."));
        assert!(msg.contains("<skill_files>"));
        assert!(msg.contains("</skill_content>"));
    }

    /// Regression: the OpenCode skill tool's body must reach the model via the
    /// normal tool result (`to_prompt_format()`). It was previously dropped
    /// because OpenCode registers the skill tool without a follow-up handler.
    #[tokio::test]
    async fn skill_body_reaches_prompt_format() {
        use crate::types::output::ToolOutput;

        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "test", "# Test Skill\n\nDo the thing.");

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "test",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "test".into(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        let prompt = ToolOutput::Skill(output).to_prompt_format();
        assert!(
            prompt.contains("Do the thing."),
            "skill body must reach to_prompt_format(), got: {prompt}"
        );
        assert!(prompt.contains("<skill_content name=\"test\">"));
    }

    #[tokio::test]
    async fn skill_lists_bundled_files() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "test", "# Skill\n\nContent.");
        let skill_dir = skill_path.parent().unwrap();
        std::fs::write(skill_dir.join("helper.sh"), "#!/bin/bash").unwrap();
        std::fs::write(skill_dir.join("reference.md"), "# Ref").unwrap();

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "test",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "test".into(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        let msg = output.skill_message.unwrap();
        assert!(msg.contains("<file>"));
        // SKILL.md itself should NOT appear in the file list.
        assert!(!msg.contains("<file>") || !msg.contains("SKILL.md"));
    }

    #[tokio::test]
    async fn skill_file_not_found_returns_error() {
        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "missing",
            SkillScope::Local,
            "/nonexistent/path/SKILL.md",
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "missing".into(),
            },
        )
        .await
        .unwrap();

        assert!(!output.success);
        assert!(output.tool_result.contains("Failed to load"));
        assert!(output.error.is_some());
    }

    #[tokio::test]
    async fn works_through_erased_interface() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "hello", "# Hello\n\nContent.");

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "hello",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "hello".into(),
            },
        )
        .await
        .unwrap();

        assert!(result.success);
        assert_eq!(result.skill_name, "hello");
    }

    #[tokio::test]
    async fn frontmatter_stripping() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "deploy", "Run the deploy pipeline.");

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "deploy",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "deploy".into(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        let msg = output.skill_message.unwrap();
        // Body content is present.
        assert!(msg.contains("Run the deploy pipeline."));
        // Frontmatter delimiters and keys are stripped.
        assert!(!msg.contains("---"));
        assert!(!msg.contains("tags: [ops]"));
        assert!(!msg.contains("description: Deploy to prod"));
    }

    #[tokio::test]
    async fn skill_message_xml_structure() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "fmt", "Format the code.");
        let skill_dir = skill_path.parent().unwrap();
        std::fs::write(skill_dir.join("fmt.sh"), "#!/bin/bash").unwrap();

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "fmt",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput { name: "fmt".into() },
        )
        .await
        .unwrap();

        assert!(output.success);
        let msg = output.skill_message.unwrap();

        // Must open with <skill_content name="fmt">
        assert!(msg.starts_with("<skill_content name=\"fmt\">\n"));
        // Must have # Skill: fmt header
        assert!(msg.contains("# Skill: fmt\n"));
        // Must contain the body content
        assert!(msg.contains("Format the code."));
        // Must have Base directory line
        assert!(msg.contains(&format!(
            "Base directory for this skill: file://{}",
            skill_dir.display()
        )));
        // Must have <skill_files> block
        assert!(msg.contains("<skill_files>\n"));
        // Must list the bundled file
        assert!(msg.contains(&format!(
            "<file>{}</file>",
            skill_dir.join("fmt.sh").display()
        )));
        // Must close with </skill_content>
        assert!(msg.ends_with("</skill_content>"));
    }

    #[tokio::test]
    async fn base_directory_in_output() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-skills").join("linter");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, official_skill_md("linter", "Lint all files.")).unwrap();

        let mut skill = make_test_skill("linter", SkillScope::User, skill_path.to_str().unwrap());
        skill.collection_root = Some(tmp.path().join("my-skills").to_string_lossy().into_owned());
        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![skill]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "linter".into(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        let msg = output.skill_message.unwrap();
        let expected_base = format!(
            "Base directory for this skill: file://{}",
            skill_dir.display()
        );
        assert!(
            msg.contains(&expected_base),
            "Expected base dir line:\n  {expected_base}\nFull message:\n{msg}"
        );
        assert!(msg.contains("Relative paths in this skill"));
    }

    #[tokio::test]
    async fn skill_with_no_bundled_files() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "solo", "Solo skill, no extras.");

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "solo",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "solo".into(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        let msg = output.skill_message.unwrap();
        // <skill_files> block must be present but contain no <file> entries.
        assert!(msg.contains("<skill_files>"));
        assert!(msg.contains("</skill_files>"));
        assert!(!msg.contains("<file>"));
    }

    #[tokio::test]
    async fn empty_skill_content() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "empty", "");

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "empty",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "empty".into(),
            },
        )
        .await
        .unwrap();

        // Should succeed gracefully even with no body content.
        assert!(output.success);
        assert_eq!(output.tool_result, "Loaded skill: empty");
        assert!(output.error.is_none());

        let msg = output.skill_message.unwrap();
        assert!(msg.contains("<skill_content name=\"empty\">"));
        assert!(msg.contains("# Skill: empty"));
        assert!(msg.contains("</skill_content>"));
        // No frontmatter keys leaked into output.
        assert!(!msg.contains("description: nothing"));
    }

    #[tokio::test]
    async fn ten_file_cap() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_official_skill(tmp.path(), "bigskill", "Do stuff.");
        let skill_dir = skill_path.parent().unwrap();

        // Create 15 extra files in the skill directory.
        for i in 1..=15 {
            std::fs::write(
                skill_dir.join(format!("file{i:02}.txt")),
                format!("content {i}"),
            )
            .unwrap();
        }

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "bigskill",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "bigskill".into(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        let msg = output.skill_message.unwrap();
        let file_tag_count = msg.matches("<file>").count();
        assert!(
            file_tag_count <= 10,
            "Expected at most 10 <file> entries, got {file_tag_count}"
        );
    }

    #[tokio::test]
    async fn unofficial_skill_cannot_be_invoked() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("leaky");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_path,
            "---\nname: leaky\ndescription: A quarantined skill.\nwhen-to-use: secret-token\n---\nBody\n",
        )
        .unwrap();

        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            "leaky",
            SkillScope::Local,
            skill_path.to_str().unwrap(),
        )]));

        let tool = SkillTool;
        let output = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput {
                name: "leaky".into(),
            },
        )
        .await
        .unwrap();

        assert!(!output.success);
        assert!(output.tool_result.contains("Failed to load"));
        assert!(output.error.is_some());
        let dump = format!("{output:?}");
        assert!(!dump.contains("secret-token"));
    }

    fn write_tool_skill(root: &Path, name: &str, body: &str) -> PathBuf {
        let dir = root.join("skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("SKILL.md");
        std::fs::write(&path, official_skill_md(name, body)).unwrap();
        path
    }

    async fn invoke_skill(path: &Path, name: &str) -> SkillOutput {
        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![make_test_skill(
            name,
            SkillScope::Local,
            path.to_str().unwrap(),
        )]));
        let tool = SkillTool;
        xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput { name: name.into() },
        )
        .await
        .unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skill_tool_rejects_ancestor_dir_symlink_after_discovery() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_tool_skill(tmp.path(), "commit", "Trusted body.");
        let skills_dir = tmp.path().join("skills");
        let real = tmp.path().join("skills.real");
        std::fs::rename(&skills_dir, &real).unwrap();
        let evil_root = TempDir::new().unwrap();
        write_tool_skill(evil_root.path(), "commit", "EVIL_SECRET_BODY");
        std::os::unix::fs::symlink(evil_root.path().join("skills"), &skills_dir).unwrap();

        let output = invoke_skill(&skill_path, "commit").await;
        assert!(!output.success);
        assert!(output.skill_message.is_none());
        let dump = format!("{output:?}");
        assert!(!dump.contains("EVIL_SECRET_BODY"));
        assert!(
            output.tool_result.contains("symlink")
                || output.error.as_deref().unwrap_or("").contains("symlink")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skill_tool_rejects_leaf_symlink() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_tool_skill(tmp.path(), "commit", "Trusted body.");
        let evil = tmp.path().join("evil.md");
        std::fs::write(&evil, official_skill_md("commit", "EVIL_SECRET_BODY")).unwrap();
        std::fs::remove_file(&skill_path).unwrap();
        std::os::unix::fs::symlink(&evil, &skill_path).unwrap();

        let output = invoke_skill(&skill_path, "commit").await;
        assert!(!output.success);
        assert!(output.skill_message.is_none());
        let dump = format!("{output:?}");
        assert!(!dump.contains("EVIL_SECRET_BODY"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skill_tool_rejects_leaf_swap_between_revalidate_and_body() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_tool_skill(tmp.path(), "commit", "Trusted body.");
        let evil = tmp.path().join("evil.md");
        std::fs::write(&evil, official_skill_md("commit", "EVIL_SECRET_BODY")).unwrap();
        let swap_path = skill_path.clone();
        crate::implementations::skills::strict::set_after_nofollow_read_hook(move || {
            std::fs::remove_file(&swap_path).unwrap();
            std::os::unix::fs::symlink(&evil, &swap_path).unwrap();
        });

        let output = invoke_skill(&skill_path, "commit").await;
        assert!(!output.success);
        assert!(output.skill_message.is_none());
        let dump = format!("{output:?}");
        assert!(!dump.contains("EVIL_SECRET_BODY"));
        assert!(
            output.tool_result.contains("symlink")
                || output.error.as_deref().unwrap_or("").contains("symlink")
        );
    }

    fn write_deep_nested_tool_skill(root: &Path, name: &str, body: &str) -> PathBuf {
        let dir = root.join("skills").join("team").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("SKILL.md");
        std::fs::write(&path, official_skill_md(name, body)).unwrap();
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skill_tool_rejects_nested_ancestor_dir_symlink_after_discovery() {
        let tmp = TempDir::new().unwrap();
        let skill_path = write_deep_nested_tool_skill(tmp.path(), "infra", "Trusted body.");
        let skills_dir = tmp.path().join("skills");
        let real = tmp.path().join("skills.real");
        std::fs::rename(&skills_dir, &real).unwrap();
        let evil_root = TempDir::new().unwrap();
        write_deep_nested_tool_skill(evil_root.path(), "infra", "EVIL_SECRET_BODY");
        std::os::unix::fs::symlink(evil_root.path().join("skills"), &skills_dir).unwrap();

        let output = invoke_skill(&skill_path, "infra").await;
        assert!(!output.success);
        assert!(output.skill_message.is_none());
        let dump = format!("{output:?}");
        assert!(!dump.contains("EVIL_SECRET_BODY"));
        assert!(
            output.tool_result.contains("symlink")
                || output.error.as_deref().unwrap_or("").contains("symlink")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skill_tool_rejects_parent_of_skills_dir_symlink_after_discovery() {
        let tmp = TempDir::new().unwrap();
        let grok = tmp.path().join(".grok");
        let skill_path = write_tool_skill(&grok, "commit", "Trusted body.");
        let real = tmp.path().join(".grok.real");
        std::fs::rename(&grok, &real).unwrap();
        let evil_root = TempDir::new().unwrap();
        write_tool_skill(evil_root.path(), "commit", "EVIL_SECRET_BODY");
        std::os::unix::fs::symlink(evil_root.path(), &grok).unwrap();

        let output = invoke_skill(&skill_path, "commit").await;
        assert!(!output.success);
        assert!(output.skill_message.is_none());
        let dump = format!("{output:?}");
        assert!(!dump.contains("EVIL_SECRET_BODY"));
        assert!(
            output.tool_result.contains("symlink")
                || output.error.as_deref().unwrap_or("").contains("symlink")
        );
    }

    async fn invoke_skill_info(skill: SkillInfo) -> SkillOutput {
        let name = skill.name.clone();
        let mut resources = Resources::new();
        resources.insert(AvailableSkills(vec![skill]));
        let tool = SkillTool;
        xai_tool_runtime::Tool::run(
            &tool,
            test_ctx(resources.into_shared()),
            SkillInput { name },
        )
        .await
        .unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skill_tool_rejects_project_dir_symlink_after_discovery() {
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("proj");
        std::fs::create_dir_all(&proj).unwrap();
        let grok = proj.join(".grok");
        let skill_path = write_tool_skill(&grok, "commit", "Trusted body.");
        let real = tmp.path().join("proj.real");
        std::fs::rename(&proj, &real).unwrap();
        let evil_root = TempDir::new().unwrap();
        write_tool_skill(
            &evil_root.path().join(".grok"),
            "commit",
            "EVIL_SECRET_BODY",
        );
        std::os::unix::fs::symlink(evil_root.path(), &proj).unwrap();

        let output = invoke_skill(&skill_path, "commit").await;
        assert!(!output.success);
        assert!(output.skill_message.is_none());
        let dump = format!("{output:?}");
        assert!(!dump.contains("EVIL_SECRET_BODY"));
        assert!(
            output.tool_result.contains("symlink")
                || output.error.as_deref().unwrap_or("").contains("symlink")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skill_tool_rejects_nested_collection_root_symlink_after_discovery() {
        let tmp = TempDir::new().unwrap();
        let pack = tmp.path().join("pack");
        let dir = pack.join("team").join("infra");
        std::fs::create_dir_all(&dir).unwrap();
        let skill_path = dir.join("SKILL.md");
        std::fs::write(&skill_path, official_skill_md("infra", "Trusted body.")).unwrap();
        let mut skill = make_test_skill("infra", SkillScope::Local, skill_path.to_str().unwrap());
        skill.collection_root = Some(pack.to_string_lossy().into_owned());
        let ok = invoke_skill_info(skill.clone()).await;
        assert!(
            ok.success,
            "nested collection must load from the stamped root"
        );

        let real = tmp.path().join("pack.real");
        std::fs::rename(&pack, &real).unwrap();
        let evil_root = TempDir::new().unwrap();
        let evil_dir = evil_root.path().join("team").join("infra");
        std::fs::create_dir_all(&evil_dir).unwrap();
        std::fs::write(
            evil_dir.join("SKILL.md"),
            official_skill_md("infra", "EVIL_SECRET_BODY"),
        )
        .unwrap();
        std::os::unix::fs::symlink(evil_root.path(), &pack).unwrap();

        let output = invoke_skill_info(skill).await;
        assert!(!output.success);
        assert!(output.skill_message.is_none());
        let dump = format!("{output:?}");
        assert!(!dump.contains("EVIL_SECRET_BODY"));
        assert!(
            output.tool_result.contains("symlink")
                || output.error.as_deref().unwrap_or("").contains("symlink")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn skill_tool_rejects_stamped_skill_dir_grandparent_symlink_after_discovery() {
        let tmp = TempDir::new().unwrap();
        let pack = tmp.path().join("pack");
        let dir = pack.join("team").join("infra");
        std::fs::create_dir_all(&dir).unwrap();
        let skill_path = dir.join("SKILL.md");
        std::fs::write(&skill_path, official_skill_md("infra", "Trusted body.")).unwrap();
        let mut skill = make_test_skill("infra", SkillScope::Local, skill_path.to_str().unwrap());
        skill.collection_root = Some(dir.to_string_lossy().into_owned());
        let ok = invoke_skill_info(skill.clone()).await;
        assert!(
            ok.success,
            "stamped skill dir must load before the grandparent swap"
        );

        let real = tmp.path().join("pack.real");
        std::fs::rename(&pack, &real).unwrap();
        let evil_root = TempDir::new().unwrap();
        let evil_dir = evil_root.path().join("team").join("infra");
        std::fs::create_dir_all(&evil_dir).unwrap();
        std::fs::write(
            evil_dir.join("SKILL.md"),
            official_skill_md("infra", "EVIL_SECRET_BODY"),
        )
        .unwrap();
        std::os::unix::fs::symlink(evil_root.path(), &pack).unwrap();

        let output = invoke_skill_info(skill).await;
        assert!(!output.success);
        assert!(output.skill_message.is_none());
        let dump = format!("{output:?}");
        assert!(!dump.contains("EVIL_SECRET_BODY"));
        assert!(
            output.tool_result.contains("symlink")
                || output.error.as_deref().unwrap_or("").contains("symlink")
        );
    }
}
