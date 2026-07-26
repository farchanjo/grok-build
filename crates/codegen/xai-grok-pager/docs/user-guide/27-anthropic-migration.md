# Migrating to the Anthropic Peer Provider

This guide is for operators who already use Claude models with Grok Build
through custom Messages TOML, environment variables, or OpenRouter, and want to
adopt the first-class Anthropic peer without losing existing setups.

**No destructive migration runs automatically.** Grok does not rewrite your
`config.toml`, does not move secrets into the vault without an explicit
`set-key` / `/providers` action, and does not delete custom Messages entries.

---

## What stays the same

| Existing setup | Behavior after the Anthropic peer lands |
|----------------|----------------------------------------|
| Hand-written `[model_providers.*]` with `api_backend = "messages"` | Continues to work as a **generic Messages** path |
| OpenRouter Claude (`openrouter:anthropic/...`) | Unchanged OpenRouter identity and billing |
| `env_key = "ANTHROPIC_API_KEY"` on custom models | Still resolved from the environment |
| xAI / OpenAI / OpenRouter vault entries | Untouched when you connect or disconnect Anthropic |
| Saved sessions | Preserve their model and execution backend metadata |

---

## What to prefer going forward

### Product Anthropic (API key peer)

For direct `api.anthropic.com` usage:

```bash
export ANTHROPIC_API_KEY="…"
grok provider set-key anthropic --from-env ANTHROPIC_API_KEY
```

Then pick a curated id such as `anthropic-claude-sonnet-5` (or a cached
`anthropic:<upstream-id>`) in `/model`. You do **not** need a hand-written
`[model_providers.grok_build_anthropic]` block — Grok installs it when the key
is configured.

### Move literal secrets out of TOML

If an older config still contains:

- literal `api_key = "…"` values
- `extra_headers` carrying `x-api-key` or other credentials
- checked-in sample keys

**Recommend** (manual):

1. Put the secret in the environment or a secret manager.
2. For product Anthropic: `grok provider set-key anthropic --from-env ANTHROPIC_API_KEY`.
3. For custom Messages gateways: set `env_key = "ANTHROPIC_API_KEY"` (or another
   env name) and remove the literal `api_key` / header secret.
4. Keep non-secret headers only (for example `anthropic-version = "2023-06-01"`).

Grok will **not** auto-migrate secrets from TOML into the vault. Leaving a
literal key in TOML is insecure and unsupported for the product Anthropic
workflow.

### Keep custom Messages when you need a gateway

Corporate proxies and third-party Messages-compatible endpoints should remain
`kind = "openai_compatible"` (or custom) with `api_backend = "messages"`. Do not
set `kind = "anthropic"` unless you intend the direct Anthropic peer identity.

---

## Sessions and execution modes

Sessions record their **execution backend** (native inference vs experimental
Claude Agent CLI, when that path is enabled).

| Change | Rule |
|--------|------|
| Switch among native models (xAI / OpenAI / Anthropic API / OpenRouter) | Allowed per existing model-switch rules |
| Switch **NativeInference ↔ Claude Agent CLI** after the first user turn | **Rejected** — start `/new` (or a new CLI session) |
| Resume a session | Restores the persisted backend; does not silently re-home to another mode |

There is no automatic conversion of a native Anthropic API session into a
Claude Agent CLI session (or the reverse).

---

## Claude Agent CLI (experimental)

Do **not** plan production rollout on the Claude Agent CLI integration until:

1. Legal/product authorization allows shipping `claude-cli-runtime` in your
   distribution channel, and
2. You deliberately build with that feature and set `GROK_CLAUDE_CLI_RUNTIME=1`,
   and
3. Operators understand subscription login, permission bridge, and limitations.

Ordinary release/default builds hide the subscription entry. The Anthropic
**API key** peer does not require that feature.

---

## OpenRouter and xAI defaults (regression expectations)

Connecting the Anthropic peer must not change:

- OpenRouter privacy defaults on connect (`data_collection = "deny"`,
  `require_parameters = true`, `X-OpenRouter-Title = "Grok Build"`)
- First-party xAI default model / principal auth behavior
- Custom Messages gateway configs already on disk

If any of those regress, treat it as a product bug independent of Anthropic
setup.

---

## Checklist

1. Inventory TOML for literal Anthropic-related secrets → move to env/vault.
2. Decide: product Anthropic peer vs custom Messages gateway vs OpenRouter.
3. Connect with `set-key` / `/providers` only when using the product peer.
4. Refresh models; pick an explicit Anthropic model for new work.
5. Start a **new session** when changing execution mode (API vs experimental CLI).
6. Disconnect Anthropic when the key should leave the machine:
   `grok provider disconnect anthropic`.

---

## Related docs

- [Anthropic provider (full)](26-anthropic-provider.md)
- [Custom models](11-custom-models.md)
- [Authentication](02-authentication.md)
- [Sessions](17-sessions.md)
- [Provider page](../providers/anthropic.md)
