# Agent Mode (ACP) — removed

ACP agent mode (`grok agent stdio` and `grok agent serve`) has been **removed**
from the product surface.

## What to use instead

| Goal | Command |
| --- | --- |
| Interactive coding | `grok` (TUI) |
| Single-turn / scripts / CI | `grok -p "…"` (headless) |
| Shared leader for TUI reconnect | `grok agent leader` (internal; TUI may spawn this) |

## Why it was removed

Grok Build no longer ships as an ACP stdio/WebSocket server for IDE embedding.
Provider work (including ChatGPT OAuth) runs inside the normal TUI and headless
inference paths.

If an older script still calls `grok agent stdio` or `grok agent serve`, the
binary exits with a short error pointing at the TUI or `grok -p`.
