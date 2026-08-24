# Strict Skills Migration

This guide is the release migration contract for strict Agent Skills, Prime
indexes, and local regression. It does **not** repair, rewrite, or bypass a
quarantined skill. Invalid `SKILL.md` files stay quarantined until **you**
edit them to the official contract plus namespaced Grok extensions.

See also [Skills](08-skills.md), [Plugins](09-plugins.md),
[Retrieval and Prime](30-retrieval-and-prime.md), and
[Memory](13-memory.md).

---

## What changed

Grok now validates every skill source with one strict contract:

- Official Agent Skills keys only at the top level: `name`, `description`,
  `license`, `compatibility`, `metadata`, and `allowed-tools`.
- Required nonempty `name` and `description`. `name` must match the parent
  directory and the official name grammar.
- `metadata` is a string-to-string map. Nested Grok extensions live under
  `metadata.grok` or dotted `metadata.grok.*` keys.
- Unknown top-level keys, YAML lists for `allowed-tools`, non-string metadata
  values, and malformed Grok extensions are **quarantine errors**.
- There is no normalization, coercion, body-derived fallback, or silent
  repair. A quarantined skill is never advertised, invoked, preloaded, or
  primed.

Flat `commands/*.md` files remain slash commands only. They cannot bypass
skill gates.

---

## Move Grok extensions under `metadata.grok.*`

These top-level keys are rejected. Move them under `metadata.grok`:

| Rejected top-level key | Strict location |
|------------------------|-----------------|
| `when-to-use` / `when_to_use` | `metadata.grok.when-to-use` |
| `argument-hint` | `metadata.grok.argument-hint` |
| `user-invocable` | `metadata.grok.user-invocable` |
| `disable-model-invocation` | `metadata.grok.disable-model-invocation` |
| `model` | `metadata.grok.model` |
| `effort` | `metadata.grok.effort` |
| `paths` | `metadata.grok.paths` |
| `short-description` | `metadata.grok.short-description` |

### Before (quarantined)

```markdown
---
name: commit
description: Create well-formatted git commits. Use when the user wants to commit.
when-to-use: commit changes
user-invocable: true
---

Review staged changes and create a conventional commit.
```

### After (valid)

```markdown
---
name: commit
description: Create well-formatted git commits. Use when the user wants to commit.
metadata:
  grok:
    when-to-use: commit changes
    argument-hint: commit message
    user-invocable: true
    disable-model-invocation: false
    paths:
      - src/**
---

Review staged changes and create a conventional commit.
```

Equivalent dotted keys such as `metadata.grok.when-to-use` are accepted.
Conflicting nested and dotted values for the same leaf are quarantined.

Boolean Grok fields must be YAML booleans (`true` / `false`), not `yes` /
`on`. `allowed-tools` must be a single space-separated string, not a YAML
list.

---

## Repair quarantined skills (manual only)

Quarantine is fail-closed. Grok never rewrites a `SKILL.md` file, never
infers `name` from the directory, and never fills `description` from the
body.

1. Open `/skills`. Quarantined rows are visible and never enableable.
2. Read the compact diagnostic code (for example `unexpected-top-level-key`
   or `name-directory-mismatch`). Diagnostics never include the raw value,
   the skill body, or an absolute path.
3. Edit the file yourself. Typical repairs:
   - Move Grok keys under `metadata.grok`.
   - Rename the parent directory so it matches `name`.
   - Add a nonempty `description` of at most 1024 characters.
   - Rename `skill.md` to `SKILL.md`.
   - Replace a YAML list `allowed-tools` with a space-separated string.
4. Save the file. Grok revalidates on reload. Opening `/skills` or searching
   does **not** start regression or network work.
5. Confirm the row is no longer quarantined, then run local regression if
   you maintain `evals/cases.yaml`.

Headless check:

```bash
grok skills validate path/to/skill --json
grok skills regress path/to/skill --json
```

Exit code `0` means valid (or local regression passed). Exit code `1` means
quarantined or failed. The JSON payload is versioned (`apiVersion: 1`) and
secret-free.

---

## Author `evals/cases.yaml`

Local regression is optional, offline, and deterministic. Put a bounded suite
next to `SKILL.md`:

```text
commit/
  SKILL.md
  evals/
    cases.yaml
```

Schema version `1`. At most 32 cases. Case ids use lowercase letters, digits,
and hyphens. Queries, paths, and resources are bounded strings. The runner
never contacts an embedding, reranking, or model provider and never stores
bodies, prompts, or absolute paths.

```yaml
version: 1
cases:
  - id: should-commit
    kind: should_trigger
    query: commit changes
    skill: commit
  - id: not-weather
    kind: should_not_trigger
    query: weather
    skill: commit
  - id: pin-commit
    kind: explicit_pin
    skill: commit
  - id: path-src
    kind: path_trigger
    path: src/lib.rs
    skill: commit
  - id: resource-git
    kind: resource
    resource: commit
    skill: commit
  - id: no-conflict
    kind: conflict
    query: commit changes
    peers: [commit, review]
```

Supported kinds:

| Kind | Required fields | Passes when |
|------|-----------------|-------------|
| `should_trigger` | `query`, `skill` | Local metadata matches the query |
| `should_not_trigger` | `query`, `skill` | Local metadata does not match |
| `explicit_pin` | `skill` | The subject skill name matches |
| `path_trigger` | `path`, `skill` | A `metadata.grok.paths` glob matches |
| `resource` | `resource`, `skill` | Name, description, or when-to-use contains the resource token |
| `conflict` | `query`, at least two `peers` | At most one peer matches the query |

The runner executes the suite twice and requires stable ordering. Status
vocabulary is `valid-pass`, `failed`, `quarantined`, `stale`, and
`untested`. Results go stale when the inventory fingerprint or the live
`evals/cases.yaml` fingerprint changes. Missing or unreadable cases mark the
row stale, not passing.

---

## Local versus configured-profile regression

- **Local regression** is the default. It is offline, cancellable,
  generation-aware, and non-destructive. It never sends skill bodies.
- **Configured-profile regression** uses the shipped retrieval route only
  after an explicit confirmation. The UI shows the configured **profile id**,
  never an endpoint or credential. Unconfirmed calls stay local and report
  `confirm_required`.
- Mixed-version ACP defaults fail closed: new methods require `apiVersion: 1`.
  Legacy `x.ai/skills/list` without a version keeps the historical
  `{ skills: [...] }` shape and does not expose enablement of quarantined
  rows.

---

## Index operations

Prime metadata lives at `$GROK_HOME/indexes/prime/<workspace-identity>/metadata.sqlite`.
It is independent of the Memory index at
`$GROK_HOME/memory/<workspace-identity>/index.sqlite`.

| Operation | Effect |
|-----------|--------|
| Status | Generation/fingerprint-preconditioned refresh. Unchanged snapshots are compact. |
| Backfill | Missing-only vectors for the pinned embedding space. |
| Rebuild | Full restage of one collection (`skills` or `agents`) or both. |
| Cancel | Cooperative cancel of a running job. Idle cancel is a no-op. |

Skills and callable-agent collections are independent: rebuilding one never
drops or blocks the other. Saving retrieval configuration is not the same as
rebuilding the index. Configured-profile backfill and rebuild require
confirmation and display the configured route.

`/skills` is the compact health, search, diagnostic, and regression control
center. `/agents` shows only compact agent index state. Retrieval Settings
owns the independent Skills/Agents index controls.

---

## Local-only fallback

When semantic routes are unavailable, Smart search and Prime degrade to the
exact deterministic local order (when-to-use, path, and inventory evidence)
and report a secret-free degradation category. Local-only fallback:

- preserves explicit slash pins first, without duplication;
- requires positive local evidence for automatic candidates;
- never installs stale vectors from a previous embedding space;
- never sends bodies, prompts, or absolute paths.

A hard semantic failure occurs **before** user-turn insertion. Soft failure
keeps the local ranking.

---

## Rollback

On rollback:

1. Stop index backfill, rebuild, and configured-profile regression.
2. Retain last-known-good published retrieval graphs, skill inventories, and
   index files. Do not hand-edit fingerprint or schema rows.
3. Keep FTS available. Vector tables may be empty; local ranking still works.
4. Leave quarantined skills quarantined. Rollback is not a repair bypass.
5. After re-upgrade, rerun `grok skills validate` / `grok inspect --json` and
   confirm inspect, `/context`, and `/session-info` still redact secrets.

A newer Prime schema is fail-closed read-only for older binaries. Do not
delete `$GROK_HOME/indexes/prime/` to "fix" that; wait for a matching binary
or rebuild from a compatible version.

---

## Privacy

Prime and inspect surfaces accept only bounded, non-secret metadata:

- Indexed fields: strict name, frontmatter description, bounded
  `grok.when-to-use` / `grok.paths`, and a safe scope label under opaque ids.
- Never stored or transmitted: skill bodies, prompts, credentials, absolute
  paths, session history, raw provider errors, full fingerprints, or vector
  dumps.
- `/context`, `/session-info`, and `grok inspect` report names, counts,
  generations, truncated fingerprints, and degradation labels only.
- Memory and Prime databases remain isolated. A Memory rebuild cannot write
  Prime rows, and a Prime rebuild cannot write Memory chunks.

Do not put secrets in skill bodies or descriptions. Selected Prime reminders
are bounded snippets of **selected** bodies only.

---

## Bundled and plugin authors

Bundled and plugin skills use the same strict validator. A bad remote bundle
candidate cannot replace last-known-good. Invalid plugin skill components are
quarantined while other plugin components may remain.

Before publishing:

```bash
grok skills validate path/to/skill --json
grok skills regress path/to/skill --json   # when evals/cases.yaml exists
```

Hermetic repository tests walk bundled and plugin author fixtures with no
network. Release gates fail if a bundled/plugin fixture is quarantined, if
legacy top-level Grok keys reappear, or if diagnostics leak bodies, paths, or
credential-like values.

See [Plugins](09-plugins.md) for install and trust rules.
