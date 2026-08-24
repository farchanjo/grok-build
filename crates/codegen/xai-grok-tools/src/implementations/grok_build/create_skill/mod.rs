//! Native `create_skill` tool — permission-aware publish of a project or user skill.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::implementations::skills::strict::{
    PublishError, PublishScope, SKILL_MD_FILE_NAME, dest_parent_for_scope,
    is_official_publishable_name, publish_skill_directory, render_skill_md,
};
use crate::types::requirements::Expr;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::util::grok_home::grok_home;

pub const CREATE_SKILL_TOOL_NAME: &str = "create_skill";

const DESCRIPTION: &str = r#"Create a new Agent Skill directory with a valid SKILL.md and publish it atomically.

Use this instead of writing SKILL.md by hand. Name must match official grammar (lowercase Unicode letters, digits, hyphens; 1-64 characters). Scope is `project` (repository `.grok/skills/`) or `user` (`$GROK_HOME/skills/`). Invalid skills are not published."#;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateSkillInput {
    #[schemars(description = "Official skill name. Must match the destination directory name.")]
    pub name: String,
    #[schemars(description = "Nonempty description (what the skill does and when to use it).")]
    pub description: String,
    #[serde(default)]
    #[schemars(description = "Markdown body. Optional; a heading is generated when empty.")]
    pub body: Option<String>,
    #[serde(default)]
    #[schemars(description = "project (repository) or user (Grok home). Defaults to project.")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateSkillOutput {
    pub name: String,
    pub created: bool,
    pub generation: u64,
    pub status: String,
}

impl From<CreateSkillInput> for crate::types::tool_io::ToolInput {
    fn from(value: CreateSkillInput) -> Self {
        crate::types::tool_io::ToolInput::Dynamic(
            serde_json::to_value(value).expect("CreateSkillInput serializes"),
        )
    }
}

impl TryFrom<crate::types::tool_io::ToolInput> for CreateSkillInput {
    type Error = String;
    fn try_from(value: crate::types::tool_io::ToolInput) -> Result<Self, Self::Error> {
        match value {
            crate::types::tool_io::ToolInput::Dynamic(v) => {
                serde_json::from_value(v).map_err(|e| format!("CreateSkillInput: {e}"))
            }
            _ => Err("expected Dynamic variant for CreateSkillInput".into()),
        }
    }
}

#[derive(Debug, Default)]
pub struct CreateSkillTool;

impl crate::types::tool_metadata::ToolMetadata for CreateSkillTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Write
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        DESCRIPTION
    }

    fn requires_expr(&self) -> Expr<crate::types::requirements::ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for CreateSkillTool {
    type Args = CreateSkillInput;
    type Output = crate::types::output::ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(CREATE_SKILL_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            CREATE_SKILL_TOOL_NAME,
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

    #[tracing::instrument(name = "new_tool.create_skill", skip_all, fields(name = %input.name))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: CreateSkillInput,
    ) -> Result<crate::types::output::ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::{resolve_cwd, shared_resources};
        let resources = shared_resources(&ctx)?;
        let cwd = resolve_cwd(&ctx, &resources).await?;
        let result = publish_from_fields(
            &cwd,
            &input.name,
            &input.description,
            input.body.as_deref().unwrap_or(""),
            input.scope.as_deref().unwrap_or("project"),
            None,
            0,
        )
        .map_err(|err| xai_tool_runtime::ToolError::invalid_arguments(err.message()))?;
        Ok(crate::types::output::ToolOutput::Text(
            crate::types::output::TextOutput::from(format!(
                "Published skill {} ({}) generation {}",
                result.name, result.status, result.generation
            )),
        ))
    }
}

/// True when `name` is a single `Normal` path component (not empty, `.`, `..`,
/// a separator, or an absolute path). Callers still apply official grammar.
fn is_single_normal_basename(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return false;
    }
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(os)), None) => os.to_str() == Some(name),
        _ => false,
    }
}

fn require_publishable_skill_name(name: &str) -> Result<(), PublishError> {
    if !is_single_normal_basename(name) {
        return Err(PublishError::PathEscape);
    }
    if !is_official_publishable_name(name) {
        return Err(PublishError::Quarantined);
    }
    Ok(())
}

/// Shared by the native tool, ACP publish, and the create wizard.
pub fn publish_from_fields(
    cwd: &Path,
    name: &str,
    description: &str,
    body: &str,
    scope: &str,
    expected_generation: Option<u64>,
    current_generation: u64,
) -> Result<CreateSkillOutput, PublishError> {
    require_publishable_skill_name(name)?;
    let scope = PublishScope::parse(scope)?;
    if name.chars().count() > 64 || description.chars().count() > 1024 || body.len() > 64 * 1024 {
        return Err(PublishError::FileTooLarge);
    }
    let staging = tempfile::Builder::new()
        .prefix("grok-skill-")
        .tempdir()
        .map_err(|_| PublishError::Staging)?;
    let skill_dir = staging.path().join(name);
    std::fs::create_dir_all(&skill_dir).map_err(|_| PublishError::Staging)?;
    std::fs::write(
        skill_dir.join(SKILL_MD_FILE_NAME),
        render_skill_md(name, description, body),
    )
    .map_err(|_| PublishError::Io)?;
    let dest_parent = dest_parent_for_scope(scope, cwd, &grok_home())?;
    let result = publish_skill_directory(
        &skill_dir,
        &dest_parent,
        scope,
        expected_generation,
        current_generation,
    )?;
    Ok(CreateSkillOutput {
        name: result.identity.parent_dir_name,
        created: result.created,
        generation: result.generation,
        status: result.status.as_str().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_project_skill_atomically() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let out = publish_from_fields(
            tmp.path(),
            "commit",
            "Create well-formatted git commits.",
            "# Commit\n",
            "project",
            Some(2),
            2,
        )
        .unwrap();
        assert!(out.created);
        assert_eq!(out.generation, 3);
        assert!(tmp.path().join(".grok/skills/commit/SKILL.md").is_file());
        assert!(!tmp.path().join(".grok/skills/commit/.staging").exists());
    }

    #[test]
    fn rejects_invalid_name_without_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let err = publish_from_fields(
            tmp.path(),
            "Bad_Name",
            "Create well-formatted git commits.",
            "",
            "project",
            None,
            0,
        )
        .unwrap_err();
        assert_eq!(err, PublishError::Quarantined);
        assert!(!tmp.path().join(".grok/skills/Bad_Name").exists());
    }

    #[test]
    fn rejects_path_escape_names_without_writing_outside_staging() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let description = "Create well-formatted git commits.";

        let sibling = tmp
            .path()
            .parent()
            .unwrap()
            .join("escaped")
            .join("SKILL.md");
        let sibling_existed = sibling.exists();
        let err = publish_from_fields(
            tmp.path(),
            "../escaped",
            description,
            "",
            "project",
            None,
            0,
        )
        .unwrap_err();
        assert_eq!(err, PublishError::PathEscape);
        assert_eq!(
            sibling.exists(),
            sibling_existed,
            "../escaped must not write SKILL.md beside the destination tempdir"
        );
        assert!(!tmp.path().join(".grok/skills/escaped").exists());
        assert!(!tmp.path().join(".grok/skills/../escaped/SKILL.md").exists());

        let abs = Path::new("/tmp/escaped");
        let abs_skill = abs.join("SKILL.md");
        let abs_existed = abs_skill.exists();
        let err = publish_from_fields(
            tmp.path(),
            "/tmp/escaped",
            description,
            "",
            "project",
            None,
            0,
        )
        .unwrap_err();
        assert_eq!(err, PublishError::PathEscape);
        assert_eq!(
            abs_skill.exists(),
            abs_existed,
            "/tmp/escaped must not receive SKILL.md"
        );
        assert!(!tmp.path().join(".grok/skills/escaped").exists());
    }

    #[test]
    fn publish_from_fields_accepts_official_unicode_name() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let out = publish_from_fields(
            tmp.path(),
            "навык",
            "A skill with Russian lowercase name used in official fixtures.",
            "# Body\n",
            "project",
            None,
            0,
        )
        .unwrap();
        assert!(out.created);
        assert!(
            tmp.path().join(".grok/skills/навык/SKILL.md").is_file(),
            "official Unicode names must publish"
        );
    }
}
