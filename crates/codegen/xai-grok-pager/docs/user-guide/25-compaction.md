# Compaction Settings

Grok automatically summarizes conversation history to free up context window space. This page describes how to configure compaction behavior via `config.toml` and the TUI settings pane.

---

## The `/compact` Command

Manually compact the current conversation:

```
/compact
/compact [context]
```

The optional `context` argument provides additional instructions about what to preserve during compaction.

---

## Auto-Compaction

Grok automatically compacts the conversation when the context window approaches its limit. You will see a notification when auto-compaction triggers.

### When Auto-Compaction Fires

Auto-compaction triggers when:

- **Fixed policy**: Token usage reaches the configured threshold percentage (default 85%).
- **Dynamic policy**: Token usage reaches the earlier of the fixed threshold or the reserve-aware boundary. The session boundary reserves 32,768 tokens for the next model output and 8,192 tokens for request/tokenizer uncertainty.

These are conservative planning reserves, not extra context. They reduce the history budget available to compaction so the next request does not consume the model's entire window.

---

## Configuration

Configure compaction in `config.toml` under the `[compaction]` section:

```toml
[compaction]
strategy = "auto"              # auto | rolling | full_replace
trigger_policy = "fixed"         # fixed | dynamic
rolling_band_count = 4           # 3..=8, default 4

# Ordered list of compaction models (max 2)
models = ["@session"]          # @session or catalog model IDs
```

---

## Compaction Strategy

| Value | Description |
|-------|-------------|
| `auto` (default) | Uses rolling compaction when an automatic idle safe point has eligible cold history; otherwise it retains the compatible full-replace path. |
| `rolling` | Summarizes the oldest eligible logical band while preserving the fixed prefix and recent raw tail. |
| `full_replace` | Summarizes the mutable conversation into a replacement summary. |

### When Each Strategy Is Best

- **`auto`**: For most users. Grok prefers rolling automatic compaction while keeping compatibility with existing full-replace flows.
- **`rolling`**: For long sessions where recent context is critical (e.g., active coding, debugging). Preserves the newest turns in raw form.
- **`full_replace`**: For sessions where the full context has been summarized and you want maximum space savings.

---

## Trigger Policy

| Value | Description |
|-------|-------------|
| `fixed` (default) | Triggers compaction when token usage reaches a fixed percentage of the context window (default 85%). |
| `dynamic` | Triggers at the earlier of the fixed percentage and a reserve-aware boundary that leaves space for the next output and safety margins. |

### Auto-Compact Threshold

The default trigger percentage is 85%. Override via:

```toml
[session]
auto_compact_threshold_percent = 85
```

Or via environment variable: `GROK_AUTO_COMPACT_THRESHOLD_PERCENT=85`

---

## Rolling Band Count

When using `rolling` strategy, Grok divides the conversation into logical bands for summarization.

| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| `rolling_band_count` | 3–8 | 4 | Number of bands for rolling compaction. More bands preserve more recent context as raw history. |

### How Rolling Compaction Works

The bands are logical regions of one authoritative conversation, not separate buffers:

1. Grok preserves a fixed prefix containing system and project context.
2. Existing compaction summaries form a historical summary spine.
3. The oldest eligible raw band is selected without crossing tool-call/result boundaries.
4. That band is summarized and atomically replaced; the warm and hot raw tail remains in place.
5. If the compaction model has a smaller context window, Grok summarizes atomic subchunks and merges their summaries hierarchically.

Higher band counts produce smaller nominal bands. Before reserves, four nominal bands are 250,000 tokens for a 1,000,000-token window, 262,144 for a 1,048,576-token window, and 125,000 for a 500,000-token window. The fixed prefix, 32,768-token output reserve, and 8,192-token safety reserve reduce the runtime target.

The compactor request has its own budget: its context window minus a 32,768-token summary-output reserve, 8,192 tokens of instructions, and an 8,192-token tokenizer safety margin. When one logical band exceeds this usable request capacity, Grok uses atomic subchunks and hierarchical merging on the same route.

---

## Compaction Models

Grok uses one or two models for compaction summarization. Configure them as an ordered list:

```toml
[compaction]
models = ["grok-4", "@session"]    # Primary, then fallback
```

| Value | Meaning |
|-------|---------|
| `@session` | Use the active session's model (default behavior) |
| `<model-id>` | Any model in the catalog or a custom provider model |

### Model Resolution Order

1. **Primary model**: Always attempted first.
2. **Fallback model**: Attempted only after a retryable transport, provider, rate-limit, 5xx, timeout, empty-response, or degenerate-summary failure.

Authentication, configuration, privacy/policy, cancellation, unsupported deterministic failures, and context overflow do not switch routes. Context overflow instead reduces or subdivides the current route's input. If only one model is specified, there is no separate fallback route. If no models are specified, `@session` is used by default.

### Example Configurations

```toml
# Use session model (default)
[compaction]
models = ["@session"]

# Use a dedicated summarization model
[compaction]
models = ["grok-4"]

# Use grok-4 for primary, fall back to session model
[compaction]
models = ["grok-4", "@session"]

# Use a custom provider for compaction
[compaction]
models = ["my-summarizer"]

# Two custom models
[compaction]
models = ["my-summarizer", "my-backup-summarizer"]
```

---

## Fallback Behavior

When the primary compaction model fails, Grok tries the configured fallback only for failures where another route may help:

- network and transport failures;
- provider or service unavailability;
- HTTP 408, 429, and 5xx responses;
- stream idle or wall-clock timeouts;
- empty or degenerate summaries.

Grok does **not** switch routes for authentication, invalid configuration, privacy or provider-policy denial, cancellation, unsupported deterministic errors, or context-window overflow. A context overflow is handled on the same route by reducing or subdividing the input. If no fallback is configured or both routes fail, the current compaction attempt fails according to the normal retry and suppression policy.

---

## External Provider Privacy

When using external providers (non-xAI models) for compaction:

- **Model access**: The conversation history is sent to the provider for summarization
- **Data retention**: Provider's privacy policy applies to the summarization request
- **Credentials**: Stored in `auth.json` under the provider's section; never in `config.toml`
- **Recommended**: Review the selected provider's retention and privacy terms before using it for compaction.

### Privacy Considerations

- `@session` means “use the active session route”; it does not imply a particular provider.
- External providers see the conversation portion being summarized, which may include source code, tool output, paths, and user instructions.
- The TUI warns when a selected dedicated compaction route may send history to an external provider.

---

## Durability and Replay

Compaction checkpoints enable session rewind across compaction boundaries.

### Checkpoint Storage

When compaction completes, Grok saves a checkpoint containing the exact compacted conversation:

```
$GROK_HOME/sessions/<encoded-cwd>/<session-id>/compaction_checkpoints/
  <checkpoint-id>.json
```

The checkpoint and replacement history are written before a durable marker is appended to `updates.jsonl`. The marker is the commit record; an unreferenced checkpoint file is ignored by replay.

If marker durability cannot be determined, Grok fail-stops the live session before accepting another conversation mutation. Restarting the session invokes journal recovery, which selects the committed or previous base deterministically and preserves any later append-only tail. Recovery fails closed if the history matches neither recorded base.

### Rewinding Past Compaction

Use `/rewind` to restore a previous state. The append-only update history reconstructs pre-compaction turns when rewinding before the marker. For targets at or after the compaction boundary, the committed checkpoint restores the exact compacted model view without running compaction again.

### Two-Pass Compaction

When two-pass compaction is enabled (`[features] two_pass_compaction = true`):

1. **Pass 1 (prefire)**: Background summarization of history prefix
2. **Pass 2 (apply)**: Combines NOTE₁ (pass 1 output) with recent tail for final summary

This keeps summarizer latency off the critical path. The prefire cache is invalidated on:
- Model switch
- Conversation edits
- Rewind operations

---

## TUI Compaction Settings

Access compaction settings from the settings pane:

1. Press `/` to open the command palette
2. Type `/settings` or navigate to **Settings** → **Compaction**

### Available Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| Compaction strategy | enum | Auto | How history is summarized |
| Compaction trigger | enum | Fixed | When auto-compaction fires |
| Rolling band count | int | 4 | Bands for rolling compaction (3-8) |
| Primary compaction model | model | (session) | Model for summarization |
| Fallback compaction model | model | (empty) | Model when primary fails |
| Compaction status | status | Idle | Read-only live status; shows Compacting while automatic compaction runs |

Changes made in the TUI settings pane are persisted to `config.toml` under `[compaction]`.

---

## Migration and Default Behavior

### Legacy Configuration

Before the new compaction configuration, automatic compaction used the fixed 85% threshold, the active session model, and the full-replace implementation. The new absent-config default keeps the fixed threshold and `@session`, while `strategy = "auto"` permits rolling automatic compaction when eligible.

### Backward Compatibility

If your `config.toml` doesn't include `[compaction]`:

| Setting | Default Value |
|---------|---------------|
| `strategy` | `auto` |
| `trigger_policy` | `fixed` |
| `rolling_band_count` | `4` |
| `models` | `["@session"]` |

Existing configurations without the `[compaction]` section will continue to work with these defaults.

---

## Related Settings

Other configuration options that affect compaction:

```toml
[features]
two_pass_compaction = false    # Enable background prefire pass-1

[session]
auto_compact_threshold_percent = 85    # Trigger percentage

[compaction.memory_flush]
enabled = false    # Flush memory before compaction
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GROK_AUTO_COMPACT_THRESHOLD_PERCENT` | Override auto-compact threshold percentage |
| `GROK_COMPACTION_MODE` | Override compaction mode (`summary`, `transcript`, `segments`) |
| `GROK_COMPACTION_DETAIL` | Override segment detail level (`none`, `minimal`, `balanced`, `verbose`) |

---

## Troubleshooting

### "Compaction failed: out of credits"

- Add credits to your account
- Configure a fallback model under `[compaction] models`
- Use `/providers` to check account status

### "Compaction failed: context too large"

- The selected atomic tool-call/result group may be larger than the compaction model's usable input capacity.
- Grok automatically subdivides at safe atomic boundaries when possible.
- If subdivision cannot make progress, select a compaction model with a larger context window or use `full_replace` with a suitable route.

### Auto-compaction not triggering

- Check `[session] auto_compact_threshold_percent` is set appropriately
- Verify the model's context window in the model configuration
- Use `/session-info` to see current token usage

---

## See Also

- [Configuration](05-configuration.md) — Full `config.toml` reference
- [Session Management](17-sessions.md) — Session persistence and rewind
- [Memory](13-memory.md) — Pre-compaction memory flush
- [Slash Commands](04-slash-commands.md) — `/compact`, `/rewind`, `/session-info`