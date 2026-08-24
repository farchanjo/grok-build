# Skills

Skills are reusable prompt packages that extend Grok with task-specific instructions. They let you capture a repeatable procedure once, instead of re-explaining it each session.

---

## What Are Skills?

A skill is a directory that contains a `SKILL.md` file. Its markdown body tells Grok how to handle a specific type of task: step-by-step instructions, conventions, and tool-usage patterns.

Use a skill for a repeatable procedure that's too specific for AGENTS.md but too long to retype. Grok activates a skill only when it applies to your current task.

---

## Skill Locations

Grok discovers skills from these directories, in priority order:

| Location | Scope | Priority | Notes |
|----------|-------|----------|-------|
| `./.grok/skills/`, `./.grok/commands/` | Local (CWD) | Highest | Current directory skills / legacy command markdown |
| `<repo_root>/.grok/skills/`, `…/commands/` | Repo | Medium | Shared across the repo |
| `~/.grok/skills/`, `~/.grok/commands/` | User | Lowest | Personal skills for all projects |
| `~/.claude/skills/`, `~/.claude/commands/` | User | Lowest | Claude Code compatibility (configurable) |
| `./.claude/skills/`, `./.claude/commands/` | Local / Repo | High | Project Claude skills and legacy custom slash commands |
| `~/.cursor/skills/` | User | Lowest | Cursor compatibility (configurable) |
| `./.cursor/skills/` | Local / Repo | High | Project Cursor skills (when cursor compat skills are enabled) |

Grok deduplicates skills by name -- a higher-priority location overrides a lower one. Grok also scans `.agents/skills/` (and `commands/`) at each tier (alongside `.grok/`) and walks every directory between your working directory and the repo root.

Flat `*.md` files under a `commands/` directory become user-invocable slash commands (filename stem = command name), matching Claude Code's legacy custom-command layout.

Skill and command discovery does **not** use `.gitignore`. Paths under known skill roots (`.grok/`, `.agents/`, `.claude/`, `.cursor/`) always load when present on disk — teams often ignore `.claude/**` as local-only config while still expecting `/frontend`-style project commands to work. To hide a skill, use `[skills] ignore` in config (not repo ignore rules).

Prime path-evidence scoring (workspace inventory used to rank skills) **does** honor repository ignore rules, but it does not read the developer global git excludes file, so ranking evidence stays host-stable. Discovery and path-evidence are therefore different: a gitignored `SKILL.md` still loads; a gitignored `src/` tree does not contribute path matches.

The Skills tab in the TUI is one list with one Local/Smart field. See [04-slash-commands.md](04-slash-commands.md#skills) for `/`, `m`, `f`, `n`, and `g`.

Grok scans the Claude and Cursor skill directories by default. To stop scanning a vendor, set its `skills` cell to `false` under `[compat.cursor]` or `[compat.claude]` in `~/.grok/config.toml`, or set the `GROK_CURSOR_SKILLS_ENABLED` or `GROK_CLAUDE_SKILLS_ENABLED` environment variable to `false`. See [Configuration](05-configuration.md#harness-compatibility) for details. Grok always filters out known vendor-shipped default skills (such as Cursor's `shell`, `canvas`, and `statusline`), regardless of these settings.

### Additional Skill Directories

Add directories, exclude paths, or disable individual skills via `[skills]` in `~/.grok/config.toml`:

```toml
[skills]
paths = ["~/my-team-skills"]          # Additional directories to scan
ignore = ["~/my-team-skills/wip"]     # Paths to exclude (hidden entirely)
disabled = ["wip-skill"]              # Skill names to keep listed but inactive
```

Each entry in `paths` is a `SKILL.md` file or a directory that Grok walks recursively. `ignore` hides a skill completely; `disabled` keeps it in the list but excludes it from the system prompt and from invocation. `paths` and `ignore` take filesystem paths and support `~` expansion; `disabled` takes skill names.

---

## Creating a Skill

### Directory Structure

Each skill lives in its own directory with a `SKILL.md` file:

```
~/.grok/skills/
  commit/
    SKILL.md
  review-pr/
    SKILL.md
  deploy/
    SKILL.md
```

### SKILL.md Format

A skill file has YAML frontmatter followed by markdown instructions:

```markdown
---
name: commit
description: Create well-formatted git commits following conventional commit standards. Use when the user wants to commit changes or asks for /commit.
---

# Git Commit Skill

Review staged changes and create a commit with a clear, conventional message.

## Steps

1. Run `git diff --staged` to see changes
2. Summarize what changed and why
3. Create commit message following conventional commits format
4. Run `git commit -m "..."` with the message
```

### Core Frontmatter Fields

Official Agent Skills keys live at the top level. Invalid `SKILL.md` files are quarantined and are not advertised, invoked, preloaded, or primed.

| Field | Description |
|-------|-------------|
| `name` | Skill identifier. Must match the parent directory name. Use lowercase letters, digits, and hyphens, up to 64 characters. Repair and fallback are not applied. |
| `description` | What the skill does and when to use it. Required, nonempty, up to 1024 characters. Body-derived fallback is not applied. |

Write a specific `description`. It determines when Grok invokes the skill automatically. Name the trigger phrases and use cases.

### Optional Official Fields

| Field | Description |
|-------|-------------|
| `license` | License identifier (for example, `Apache-2.0`). |
| `compatibility` | Environment requirements (for example, `Requires git, docker, jq`), up to 500 characters. |
| `allowed-tools` | Space-separated tool list as a string. YAML lists are rejected. |
| `metadata` | String-to-string map. Nested Grok extensions live under `metadata.grok` or dotted `metadata.grok.*` keys. |

### Grok Extensions (`metadata.grok.*`)

| Field | Description |
|-------|-------------|
| `metadata.grok.when-to-use` | Trigger phrases for automatic invocation, kept separate from `description`. |
| `metadata.grok.argument-hint` | Hint text shown in the slash-command autocomplete (for example, `commit message`). |
| `metadata.grok.user-invocable` | Whether you can run the skill as a slash command. Defaults to `true` when omitted; set `false` to hide it from slash commands. |
| `metadata.grok.disable-model-invocation` | When `true`, only your slash command runs the skill -- the model cannot invoke it automatically. |
| `metadata.grok.model` | Model override for running the skill. |
| `metadata.grok.effort` | Reasoning-effort override. |
| `metadata.grok.paths` | Glob patterns that gate when the skill is listed. |
| `metadata.grok.short-description` | Compact UI description. |

Top-level Grok keys such as `when-to-use` or `user-invocable` are rejected. Move them under `metadata.grok`. Disabled skills use a stable qualified identity (`local:commit`, `plugin:name`) in `[skills].disabled`. Flat `commands/*.md` files remain slash commands only and cannot bypass skill gates.

Grok never repairs a quarantined skill. See [Strict Skills Migration](31-strict-skills-migration.md) for the field map, `evals/cases.yaml`, index operations, local-only fallback, rollback, and privacy rules.

---

## Creating Skills with /create-skill

The `/create-skill` command walks you through building a new skill interactively. Grok asks what you want, drafts the files, and writes them to disk.

### How It Works

When you run `/create-skill`, Grok:

1. **Gathers requirements.** Grok asks for the skill name, the scope to save it under, and a description of the workflow you want to capture. Use a name with lowercase letters, digits, and hyphens (2–64 characters, starting and ending with a letter or digit).

2. **Drafts the description.** Grok writes a `description` that states what the skill does, the phrases that trigger it, and the slash command name. You approve or edit the draft before continuing.

3. **Creates the skill directory.** Grok creates the `<scope>/.grok/skills/<name>/` directory, plus `scripts/` or `references/` subdirectories when the skill needs them.

4. **Writes SKILL.md.** Grok writes the frontmatter (`name` and `description`) and a markdown body of instructions, along with any supporting files.

5. **Verifies and confirms.** Grok reads the file back, confirms it wrote correctly, and tells you how to run the skill.

### Choosing a Scope

Grok asks where to save the skill:

- **Project** (`<repo_root>/.grok/skills/<name>/`) -- available only in this repository and shareable with teammates through version control. Grok recommends this scope inside a git repository.
- **User** (`~/.grok/skills/<name>/`) -- available across all your projects.

The new skill appears in the slash menu within a few seconds, because Grok reloads skills when files change on disk.

---

## Using Skills

### Run a Skill by Name

Each skill is a slash command named after the skill. Run one by typing its name:

```
/commit              # Runs the "commit" skill
/review-pr           # Runs the "review-pr" skill
```

Running a skill loads its instructions into the conversation and directs the model to follow them. To pass arguments, type them after the name:

```
/commit fix the build
```

To browse your skills, type `/` to open the slash-command menu. Grok lists every built-in command and skill and filters them as you type. To list skills from the command line instead, run `grok inspect` (see [Viewing Skill Details](#viewing-skill-details)).

### Qualified Names

When a skill's name collides with another skill or a built-in command, Grok advertises a qualified name prefixed by the skill's scope -- `local:`, `repo:`, `user:`, or the plugin name. Use the qualified form to choose a specific skill:

```
/local:commit        # The "commit" skill from ./.grok/skills/
/user:commit         # The "commit" skill from ~/.grok/skills/
```

### Automatic Invocation

Grok can invoke a skill on its own when it recognizes a relevant task. Grok matches your prompt against the skill's `description` and `metadata.grok.when-to-use` fields, so write both to describe the triggering situation.

For example, if a skill's description says "Use when the user wants to commit changes," then saying "commit my changes" can trigger that skill automatically. To require an explicit slash command and prevent automatic invocation, set `metadata.grok.disable-model-invocation: true`.

---

## Viewing Skill Details

Run `grok inspect` to see every skill Grok discovers, along with the rest of your configuration:

```bash
grok inspect          # Human-readable summary
grok inspect --json   # Machine-readable report
```

In the human-readable output, the Skills section lists each skill's name and its source -- `project`, `user`, `bundled`, `config` (a `[skills].paths` entry), `server` (skills synced from the skill store in managed workspaces), or `plugin: <name>`. Grok tags any skill disabled via `[skills].disabled` or from a disabled vendor surface with `[disabled]`.

The report honors your `[skills]` config the same way a live session does: skills from `paths` are listed, skills under an `ignore` prefix are hidden, and skills named in `disabled` stay listed but tagged `[disabled]`.

The `--json` report includes the full detail for each skill: its `name`, `description`, `source` (with the path to the SKILL.md file), and `userInvocable` flag.

---

## Bundled and Plugin Skills

Grok distributes platform skills separately from your personal skills. Bundled skills are cached under `~/.grok/bundled/skills/`; Grok never writes them into `~/.grok/skills/`. A same-named local, repo, or user skill overrides the bundled copy. `grok inspect` labels each definition by its actual source. (A plugin skill of the same name does not override a native skill; it stays available under its qualified `plugin:name` form.)

Skills can also come from plugins. When you install a plugin that includes skills, they appear alongside your user and project skills. `grok inspect` labels each plugin-provided skill with its source as `plugin: <name>`.

Bundled and plugin authors must pass the same strict validator. A quarantined bundled candidate cannot replace last-known-good. See [Strict Skills Migration](31-strict-skills-migration.md#bundled-and-plugin-authors) and the [Plugins guide](09-plugins.md).

---

## Skills, retrieval, and prime

Skills participate in **prime** — the bounded injection of retrieved context
before an eligible turn — under strict privacy rules:

- **Native-only and real-user-turn gating.** Prime runs only for an eligible
  real native user turn. Synthetic or external turns do not prime.
- **Metadata-only remote selection.** Skill selection over remote/retrieved
  sources uses metadata (name and description) only. Skill **bodies** are
  loaded only after a bounded selection and are never exposed through
  `grok inspect` or `/context`.
- **Hidden bounded reminder.** When a skill is selected by prime, Grok may
  send the model a hidden reminder containing bounded, budget-truncated
  snippets from the selected skill bodies. Unselected bodies are not included.
  The wrapper adds no credentials, but sensitive text stored inside a selected
  skill becomes model-visible; do not put secrets in skill bodies.
- **Explicit slash pin.** Pinning a skill explicitly via `/<skill>` (or
  `/<scope>:<skill>`) overrides automatic selection for that turn; the pinned
  skill is the one used.

Safe inspect/context disclosure covers profile/generation/status/name/count
categories only — never skill bodies or prompt content. See
[Retrieval and Prime](30-retrieval-and-prime.md) and
[Configuration](05-configuration.md).

---

## Best Practices

1. **Write specific descriptions.** The description drives automatic invocation. "Create git commits" is too vague; "Create well-formatted git commits following conventional commit standards. Use when the user wants to commit changes or asks for /commit." works better.

2. **Include concrete steps.** Skills work best when they give Grok a clear, ordered procedure to follow.

3. **Reference tools by name.** When a skill relies on specific tools (such as `run_terminal_command` or `search_replace`), name them so the model knows what to use.

4. **Keep skills focused.** Write one skill per workflow. A "deploy" skill and a "rollback" skill work better than a single "deploy-and-rollback" skill.

5. **Version-control project skills.** Commit `.grok/skills/` to your repository so the whole team benefits. User skills in `~/.grok/skills/` stay personal and unshared.

6. **Test by running it.** Invoke `/name` and confirm the skill works before you rely on automatic invocation.
