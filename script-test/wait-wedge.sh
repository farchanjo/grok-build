#!/usr/bin/env bash
# One-shot watcher for the wedged phase-2 agent.
#
# The skills agent is stuck inside an MCP call whose per-tool timeout is 6000s.
# Exit as soon as the situation changes, so the next decision is made on fresh
# facts instead of a timer: the agent writes again, the workflow agent finishes
# (making skills the only thing the batch waits on), or the run leaves `active`.
# Falls through on the timeout window itself.
set -u

RUN="/Users/farchanjo/.grok/sessions/%2FUsers%2Ffarchanjo%2Fdev%2Fgrok-build/01a0b71b-0bc1-7e92-b476-8829fe9c227c/workflows/wf_01a0b8a226947701b136a61492ea1698"
SKILLS="/Users/farchanjo/.grok/sessions/%2FUsers%2Ffarchanjo%2Fdev%2Fgrok-build/01a0b9a3-cbf6-71b2-be32-e3862a08f306/chat_history.jsonl"
BASE=$(stat -f%m "$SKILLS")
DEADLINE=$(( $(date +%s) + 3300 ))

while [ "$(date +%s)" -lt "$DEADLINE" ]; do
  if [ "$(stat -f%m "$SKILLS")" -gt "$BASE" ]; then
    echo "$(date +%H:%M:%S) SKILLS RETOMOU: chat_history voltou a crescer"
    exit 0
  fi
  read -r WF STATUS < <(python3 -c "
import json
s = json.load(open('$RUN/state.json'))['state']
wf = next((a['state'] for a in s['agents'] if a['label'] == 'agents-todo-workflow-goal'), '?')
print(wf, s['status'])")
  if [ "$STATUS" != "active" ]; then
    echo "$(date +%H:%M:%S) RUN SAIU DE active: $STATUS (workflow=$WF)"
    exit 0
  fi
  if [ "$WF" = "done" ]; then
    echo "$(date +%H:%M:%S) AGENTE DE WORKFLOW TERMINOU: skills vira a unica pendencia do lote"
    exit 0
  fi
  sleep 30
done
echo "$(date +%H:%M:%S) JANELA DE 100 MIN DO TOOL TIMEOUT ATINGIDA: rever o agente skills"