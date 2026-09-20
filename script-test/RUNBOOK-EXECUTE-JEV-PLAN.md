# Runbook — watching `execute-jev-plan-3` and recovering it

Operational counterpart to `execute-jev-plan.rhai`. The plan file is the durable
artifact; the run executes its own frozen copy at
`~/.grok/sessions/%2FUsers%2Ffarchanjo%2Fdev%2Fgrok-build/01a0b71b-…/workflows/wf_01a0b8a226947701b136a61492ea1698/script.rhai`,
so edits here reach the next run, not the live one.

## Live state

| Piece | Value |
| --- | --- |
| Run | `execute-jev-plan-3` (`wf_01a0b8a226947701b136a61492ea1698`) |
| Watcher | `python3 -u script-test/watch-run.py <run-dir>` — status, journal, agents, global stall, per-agent quiet |
| Decision channel | `python3 script-test/jev_ask.py …`, log at `script-test/jev-decisions.jsonl` |
| Guard snapshot | `/tmp/grok-guard-20260919-093641` (patch + copies of modified and untracked files) |
| Budget | 40 agents; 7 spent at phase 2 |

Phase indices for `args.only` / `args.start_at`: `0` preflight, `1` phase 0,
`2` phase 1, `3` phase 2, `4` phase 3, `5` phase 4, `6` verify, `7` TUI
validation. `Report` always runs.

## Reading the watcher

- `STATUS … -> active` — normal phase transition.
- `ALERT … -> paused|blocked|stopped|failed` — act; the line carries the last
  history detail, which is the pause message.
- `BLOCKED seq=… needs_human` — an agent returned `status=blocked` or
  `needs_human: true`. The run keeps going; read the entry, fix before Verify.
- `STALL nothing written for 10m` — no journal, state or agent file moved.
- `AGENT_QUIET <label> no writes for 15m` — one agent silent while peers write.
  A long build looks the same: check `pgrep -fl "cargo check|rustc"` before acting.
- `JOURNAL …` — every committed host call, including each agent's summary.

Raw reads, if the watcher is not enough:

```sh
python3 -c "import json;s=json.load(open('<run>/state.json'))['state'];print(s['status'],s['current_phase'],s['agents_used'])"
tail -3 <run>/journal.jsonl | python3 -c "import sys,json;[print(json.loads(l).get('result',{}).get('output',{}).get('summary','')) for l in sys.stdin]"
```

## Deciding with Jev

```sh
cd script-test
./.venv/bin/python jev_ask.py probe                                   # both transports answer
./.venv/bin/python jev_ask.py decide --state-file /tmp/jev-state.txt \
    --question "…" --options "$(printf '%s\n' 'a|what a does' 'b|what b does')" \
    --gate "Is the pick safe while agents hold the tree?"
./.venv/bin/python jev_ask.py gate --state-file /tmp/jev-state.txt --question "Is the tree consistent?"
```

`decide` ranks the options; `--gate` adds a yes/no soundness question on the
same call. Every call appends one line to `jev-decisions.jsonl`. Standing
decision of 2026-09-19 09:36 (three identical calls): `guard` 0.54, `brief`
0.41, `wait` 0.04, `sentinel` 0.01, with the safety gate failing (0.36) — so
interventions stay non-invasive while agents hold the tree.

## Findings fed back into the plan

Both came out of the live phase 2 and were validated by Jev
(`env_and_check` 0.53, gate "is the plan file the right level?" pass 0.67) before
being written into `execute-jev-plan.rhai`:

1. **The shell does not inherit the isolation.** The parent session runs with
   `GROK_HOME=$HOME/.grok`, so a helper called from an agent's shell refuses with
   `GROK_HOME must be unset or exactly …/.grokdev`. The memory agent lost four
   iterations to it. `ISOLATION` now tells every agent to `export` (or `unset`)
   the two variables in the command itself.
2. **"Do not build" was too absolute.** The same agent built anyway to verify
   itself and cargo answered `Blocking waiting for file lock on build directory`
   — the lock working, not a deadlock. `LOCK` now allows exactly one end-of-job
   helper per phase and keeps broad serial building for Verify.

3. **A peer's check dies on someone else's error.** The permission-laziness agent finished with
   its own crate green, but its `xai-grok-shell` check exited 101 on an E0382 in
   `xai-workflow/src/validate.rs` that the workflow agent had just introduced — and fixed twenty
   minutes later. Jev split here: rule `consumer_check` 0.97, gate "is the plan file the right
   place?" fail 0.41. Applied narrowly, and `AGENTS.md` already carries the policy ("check direct
   consumers"), so the brief now points at it instead of restating it.

## Open decision 2.10 — settled

The memory agent left the rerank primitive unwired on purpose: the search path
already owned a rerank slot from phase 1.5. Jev chose `both_config` (0.60 over
`single_path` 0.37, gate "settle now vs leave to Verify" fail 0.46), so both
ends stay and one sits behind config:

- `[memory.gate] rerank` (default `false`) picks the owner of the search call.
  It works with `enabled = false` — it is not the append gate.
- `search::gate_rerank` is the twin of `remote_rerank`: same bounded prefix,
  same permutation validation, same fail-open; both share `rerank_prefix` and
  `apply_prefix_permutation`.
- Wired through `MemoryBackendParams::gate` / `MemoryBackendImpl::with_gate`,
  built in `spawn.rs` beside the retrieval facade.
- Covered by `search::tests::gate_rerank_reorders_the_prefix_and_keeps_the_order_without_a_gate`
  (applies a permutation, stays put with no gate, and works with the append
  gate off).

## Phase 2 salvage — `skills` (wedged since 09:45)

The agent is stuck inside one MCP call (`detent__service_logs`), so its summary
never arrived. Its work is on disk; this is what a follow-up needs.

| File | Δ |
| --- | --- |
| `xai-grok-tools/src/implementations/skills/strict/evals.rs` | +232 |
| `xai-grok-shell/src/session/prime/skills.rs` | +192 |
| `xai-grok-shell/src/session/prime/mod.rs` | +76 |
| `xai-grok-shell/src/session/prime/index.rs` | +47 |
| `xai-grok-shell/src/session/prime/agents.rs` | +17 |
| `xai-grok-tools/src/implementations/skills/strict/mod.rs` | +9 |
| `xai-grok-shell/src/session/prime/decide.rs` | new |

Its own last sanity (`final_sanity.py`, log at
`.detent/logs/skills-probe.stdout.latest.log`, seq 851-870) reported 40 skills
files with natural-language cases, 20 scratch scripts and no `MISS` line.

**Its code compiles.** Its own `check-skills` run died on the `xai-workflow`
E0382 a peer had introduced, so it never saw a green check of its own; the shell
ran one after cancelling it: `xai-grok-shell` Finished in 5m04s and
`xai-grok-tools` in 21.66s, both exit 0. What was lost with the cancellation is
the summary and the last verification read, not working code. Its items 2.1-2.7
in the plan remain the spec, and the phase-3 `correctness` agent is already
working over the same tree.

## Edits outside the repo: the user's live skills

The `skills` agent appended `should_trigger` cases to 40 of the 172 skills in
`~/.grok/skills/*/evals/cases.yaml` (47 `nl-<skill>` cases). That directory has
no git and the agent left no backup, and because the parent session runs with
`GROK_HOME=~/.grok`, these are the live production skills — the run's own Verify
phase never looks outside the repo, so nothing else covers them.

- Snapshot of all 40 files: `/tmp/grok-skills-nl-20260919-105553`.
- Revert: `python3 script-test/revert-skill-nl-cases.py` (dry run) then
  `--apply`; it removes only blocks whose id starts with `nl-`.
- Jev validated the path (gate yes 0.930). Scope question open: 40 of 172 —
  whether the plan meant "every skill with evals" or a subset.

## Known environment artifact: platform trust store

`./grok-test.sh -p xai-grok-shell -- laziness` fails 13 tests when the suite
runs at full parallelism, all in `laziness_integration_tests`, all panicking at
~15s in aws-smithy's rustls provider with `no valid root certificates parsed`.
Not a regression: the test file is unmodified, no modified file mentions
rustls, one test passes alone, and `-j1` runs 94/94 green. Fixed in
`.config/nextest.toml` with a `platform-trust` test group at `max-threads = 1`,
so the family is serialized and the rest stays parallel. Jev chose
`config_group` 0.78, gate pass 0.65.

## TUI drive-through (phase 7 fallback)

The frozen copy spawns the TUI-validation agent with `agent_type: "explore"` and
`capability_mode: "execute"`; `explore` is read-only, so the intersection drops
the shell and the agent cannot build or drive tmux. `script-test/tui-verify.sh`
is that drive-through, run by hand from the shell; captures land in
`/tmp/tui-verify`.

Verified on 2026-09-19 15:34 against the phase-4 tree:

| Check | Evidence |
| --- | --- |
| Console starts clean | `01-start.txt` |
| New control commands answer | `02-memory-gate.txt`, `02-laziness.txt`, `02-prime.txt` |
| New settings row and its default | `04-settings-filter.txt` — `▸ Jev transport … OpenRouter ›` |
| Value command applies and the pane follows | `manual-status.txt` — `tool-search: bm25` after `/tool-search bm25` |

Four gotchas the script now handles, each found by failing once:

1. `unset GROK_HOME GROK_LEADER_SOCKET` before `source ./grok-dev-env.sh`, or the
   helper refuses the inherited `~/.grok` (the same trap the brief now teaches).
2. The console opens on the resume picker, which can sit on "loading sessions…"
   and ignores Escape while loading — retry the dismissal until it is gone.
3. In the settings modal, `Esc` **clears the filter**; a second `Esc` closes it.
   Confirm the modal is gone before typing, or the command lands in its search.
4. `tool-search` is a value command (`fused | bm25 | dense`), not `on | off`;
   the `on|off|status` family is the control commands.

## Ambient env vs. env-sensitive tests (the 425-failure family)

`cargo nextest run -p xai-grok-shell` reported 7627 tests, **425 failed**, all at
~15s. Not a regression and not concurrency: this machine's shell is a
grok-agent shell, so it exports `GROK_EXTERNAL_OTEL=1` and `OTEL_EXPORTER_OTLP_*`,
and `EndpointsConfig::default()` reads them through `env_string`/`env_bool`,
where blank means unset. Hundreds of tests assert against a clean environment.

Reproduced with four tests alone (all four failed in 7s), then fixed by blanking
the seven vars with `force = true` in `.cargo/config.toml [env]`: two of the four
flipped to green with no `env -u` in the command. Jev chose `repo_config` (0.69)
with the placement gate failing (0.41), so the trade-off is documented instead
of hidden:

- `[env]` with `force` overrides even an explicit shell value, so `cargo run`
  loses the ambient OTEL exporter. Running the built binary directly
  (`./target-dev/debug/xai-grok-pager`) is unaffected, and the config file can
  re-enable the exporter for a dev run.
- Two tests still fail for a different reason — `agent::models` catalog-cache
  shape (`rollout-off must prune additional-account catalog keys`) and the
  enterprise endpoint test — and are tracked here as open.

**Its own suite found one real gap, and it was fixed.** Two tests it wrote
(`semantic_fill_rerank_docs_include_bodies…`) failed: the rerank call site in
`prime/skills.rs` still sent `skill_decision_text` — the 200-byte chooser line —
while the helper it wrote for the job, `index::skill_rerank_document`, sat
unused by that path (it was already used for the index build and in
`quality.rs`, and its three own tests passed). One-line fix at the call site;
both tests now pass (2/2), Jev gate yes 0.900. Its checks never ran because the
cancel killed them, so it never saw this.

## Open: the ten `session::workflow::manager` timeouts

Deterministic, not load: two of them time out alone at 300s. The stack shows the
tokio runtime parked in the time driver with no ready task, while the test
awaits a spawn event from the host service that never arrives (the host service
is a `tokio::spawn`ed task, so a silent exit looks exactly like this).

Ruled out:

- the new `WorkflowHostRequest::Decide` arm — neutralised to a synchronous
  reply, test still times out; arm restored;
- the four workflow-area diffs read line by line (`manager.rs` +7 additive field,
  `acp_session_impl/workflow.rs` +38 catalog refresh, `mod.rs` +62 catalog fn and
  tests, `host_service.rs` Decide arm).

Blocked comparison: `git stash push -- crates/` to reach HEAD fails because
**HEAD's `xai-grok-shell` lib-test target does not compile** (E0559 with the
note "all struct fields are already assigned" — a literal that lists every field
and still adds `..Default::default()`; our tree's new field silences it). So a
clean before/after needs HEAD plus that fix.

Next step: bisect the four files plus their untracked companions as one unit, or
reproduce at HEAD once the E0559 is fixed.

## Verify drift — fixed and proven

The Verify closed with `needs_human` on five pager failures, all coherence
guards pointing at the run's own new surface. Each is now green, verified one by
one, then as a family (390/390 in
`settings::tests`, `settings_modal::tests`, `slash::commands::tests`,
`dispatch::tests::settings`, `retrieval_settings_modal::tests`):

| Guard | Fix |
| --- | --- |
| `every_setting_has_action_for_reset_arm` | one generic arm for every shell-store control row (`Action::SetControl`, already routed) plus an arm for `compaction_jev_transport`; the test helper `move_setting_away_from_default` needed its own generic arm, which the first failure did not reveal |
| `shell_collision_contract_covers_every_pager_command_and_alias` | the 8 control keys added to `SHELL_RESERVED`, in place |
| `rows_contain_categories_and_settings_through_pr_14` | the `Session` category is emitted now that `[memory.gate]` rows exist; the expected key list gained the 31 new keys |
| `protocol_field_recovers_from_an_unknown_value` | `cycle_protocol` restarts at the first entry on an unknown value, matching its own comment (was advancing to the second) |

Two more guards were checked and were already green: `every_setting_has_dispatch_arm`
and `every_setting_has_action_for_bool_arm`.

## Recovery

**Compile failure (Verify pauses).** The gate is `pause("verification", …)`, and
a `pause` in a result-derived branch re-fires on every resume. Fix the code,
then resume; if it pauses again on the same message, relaunch a continuation
instead of fighting the gate:

```
workflow(script_path: "script-test/execute-jev-plan.rhai", args: #{ only: [6, 7] })
```

**A phase returns `blocked`.** The run continues by design. Fix the cause before
Verify, and keep the agent's `blocked_on` text as the record.

**TUI validation loses its shell.** The frozen copy spawns that step with
`agent_type: "explore"` + `capability_mode: "execute"`, and `explore` is
read-only, so the intersection drops the shell — the step needs tmux. Either
drive the tmux checks by hand (see `AGENTS.md`, "Interactive TUI Verification
via tmux") or relaunch with `only: [7]`. The plan file already drops the
`agent_type` there.

**A concurrent commit sweeps our lines.** Restore from the newest guard
snapshot. `git apply --check` against the live tree will report conflicts once
agents edit the same files again — that is expected; test the patch against the
index (still at HEAD, nothing staged) instead, and use the copies for a single
file:

```sh
git apply --check --cached /tmp/grok-guard-<ts>/tracked.patch   # patch vs HEAD
git apply          --cached /tmp/grok-guard-<ts>/tracked.patch   # reconstruct, keeps the live tree
rsync -a /tmp/grok-guard-<ts>/worktree/<path> <path>             # single file back
```

Snapshots: `093641` (34 modified, 118 untracked) and `095342` (71 modified, 476
untracked, patch 274KB) — the second supersedes the first.

## `job-sentinel`

`script-test/job-sentinel.rhai` is the agentic second layer: read-only, never
takes the `target-dev` lock. Two defects found and fixed on 2026-09-19 — it
paused with the invalid kind `Observe` (now `user`) and called `sleep()`, which
throws in workflow scripts (now removed; the observe agent's own runtime paces
the loop). The file was renamed from `monitor-run.rhai` because the tool
requires the filename to equal `meta.name`; `gen_monitor_workflow.py` now writes
`<chosen-name>.rhai`. Both smoke checks pass. Not launched: Jev ranks it last
while the shell watcher already reports.

```
workflow(script_path: "script-test/job-sentinel.rhai",
         args: #{ objective: "<run-dir and what progress looks like>", poll_seconds: 60, max_polls: 90 })
```