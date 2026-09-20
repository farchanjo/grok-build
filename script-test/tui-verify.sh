#!/usr/bin/env bash
# Interactive TUI verification for the phase-4 surface.
#
# The phase-7 agent in the frozen copy spawns with `agent_type: "explore"` and
# `capability_mode: "execute"`; `explore` is read-only, so the intersection
# drops the shell and the agent cannot build or drive tmux. This script is that
# drive-through, ready to run by hand from the shell.
#
#   bash script-test/tui-verify.sh
#
# Captures land in /tmp/tui-verify and are ground truth: read them before
# concluding anything. Never `tmux attach`.
set -u

OUT=/tmp/tui-verify
BIN=./target-dev/debug/xai-grok-pager
mkdir -p "$OUT"

# One session name, reused everywhere and in the cleanup. Five sessions already
# exist on this box, so pick a free name.
SESSION=grok-dev
if tmux ls 2>/dev/null | grep -q "^grok-dev:"; then SESSION=grok-dev-jev; fi
tmux kill-session -t "$SESSION" 2>/dev/null

echo "== build =="
# The inherited GROK_HOME is the parent session's ~/.grok; the helper refuses it.
bash -c 'unset GROK_HOME GROK_LEADER_SOCKET; source ./grok-dev-env.sh && cargo build -p xai-grok-pager-bin' >"$OUT/00-build.log" 2>&1
echo "build_exit=$?  session=$SESSION"

echo "== session =="
tmux new-session -d -s "$SESSION" -x 200 -y 50 \
  "export GROK_HOME=\"\$HOME/.grokdev\" \
   GROK_LEADER_SOCKET=\"\$HOME/.grokdev/leader.sock\" \
   GROK_DISABLE_AUTOUPDATER=1 \
   GROK_CURSOR_SKILLS_ENABLED=0 GROK_CURSOR_RULES_ENABLED=0 GROK_CURSOR_AGENTS_ENABLED=0 \
   GROK_CURSOR_MCPS_ENABLED=0 GROK_CURSOR_HOOKS_ENABLED=0 GROK_CURSOR_SESSIONS_ENABLED=0 \
   GROK_CLAUDE_SKILLS_ENABLED=0 GROK_CLAUDE_RULES_ENABLED=0 GROK_CLAUDE_AGENTS_ENABLED=0 \
   GROK_CLAUDE_MCPS_ENABLED=0 GROK_CLAUDE_HOOKS_ENABLED=0 GROK_CLAUDE_SESSIONS_ENABLED=0 \
   GROK_CODEX_SKILLS_ENABLED=0 GROK_CODEX_RULES_ENABLED=0 GROK_CODEX_AGENTS_ENABLED=0 \
   GROK_CODEX_MCPS_ENABLED=0 GROK_CODEX_HOOKS_ENABLED=0 GROK_CODEX_SESSIONS_ENABLED=0; \
   $BIN --no-leader --no-auto-update; exec zsh"
sleep 6
# The dev profile has sessions, so the console opens on the resume picker, where
# anything typed lands in its search box. It can sit on "loading sessions…" for a
# while and ignores Escape during that; so retry the dismissal until it is gone
# instead of guessing the timing.
for _ in $(seq 1 30); do
  tmux capture-pane -p -t "$SESSION" >"$OUT/00-boot.txt"
  grep -q "Resume session" "$OUT/00-boot.txt" || break
  tmux send-keys -t "$SESSION" Escape
  sleep 3
done
tmux capture-pane -p -t "$SESSION" >"$OUT/01-start.txt"
grep -q "Resume session" "$OUT/01-start.txt" && echo "AVISO: picker ainda na tela em 01-start"

echo "== new control commands =="
for cmd in memory-gate laziness prime tool-search; do
  tmux send-keys -t "$SESSION" "/$cmd status" Enter
  sleep 2
  tmux capture-pane -p -t "$SESSION" >"$OUT/02-$cmd.txt"
done

echo "== new settings row =="
tmux send-keys -t "$SESSION" "/settings" Enter
sleep 3
tmux capture-pane -p -t "$SESSION" >"$OUT/03-settings.txt"
# The modal filters on `/` first, then the text.
tmux send-keys -t "$SESSION" "/"
sleep 1
tmux send-keys -t "$SESSION" "transport"
sleep 2
tmux capture-pane -p -t "$SESSION" >"$OUT/04-settings-filter.txt"
# In the modal, Esc clears the filter; a second Esc closes it. Confirm it is
# gone before typing anything else, or the next command lands in its search box.
tmux send-keys -t "$SESSION" Escape
sleep 1
for _ in $(seq 1 5); do
  grep -q "─ Settings ─" <(tmux capture-pane -p -t "$SESSION") || break
  tmux send-keys -t "$SESSION" Escape
  sleep 1
done
tmux capture-pane -p -t "$SESSION" >"$OUT/04b-modal-closed.txt"
grep -q "─ Settings ─" "$OUT/04b-modal-closed.txt" && echo "AVISO: modal ainda aberto"

echo "== switch one value and read the line back =="
# `tool-search` takes a value, not on/off: fused | bm25 | dense.
tmux send-keys -t "$SESSION" "/tool-search bm25" Enter
sleep 2
tmux capture-pane -p -t "$SESSION" >"$OUT/05-switch-bm25.txt"
tmux send-keys -t "$SESSION" "/tool-search fused" Enter
sleep 2
tmux capture-pane -p -t "$SESSION" >"$OUT/06-switch-back.txt"
grep -h "tool-search:" "$OUT/05-switch-bm25.txt" "$OUT/06-switch-back.txt" | head -2

echo "== cleanup =="
tmux send-keys -t "$SESSION" "/quit" Enter
sleep 3
tmux kill-session -t "$SESSION" 2>/dev/null
ls -la "$OUT"
echo "captures em $OUT"