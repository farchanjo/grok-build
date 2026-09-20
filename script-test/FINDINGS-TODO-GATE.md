# Todo gate with Jev — complete findings and design

Consolidated 2026-09-19. Companion to the other `FINDINGS-*.md`; `FINDINGS.md` is
the umbrella.

Measured against `jev-1.13`, native transport, live dev profile read-only. No
Rust was changed.

## 1. Goal

The gate is deterministic by design, so the question is not whether to add a
model but whether there is any judgement left to make. There is one.

## 2. What exists today (verified)

`session/acp_session_impl/reminders.rs` (860 lines, of which the gate is four):

```rust
pub fn evaluate_todo_gate(input: &TodoGateInput<'_>) -> TodoGateDecision {
    if input.pending.is_empty() && input.in_progress_unbacked.is_empty() {
        return TodoGateDecision::Continue;
    }
    TodoGateDecision::Nudge { reminder: build_todo_gate_reminder(...), reason: InFlight }
}
```

- **No model call.** Pending todos, or in-progress ones with no backing
  task (`backing_task_count == 0`), produce a nudge.
- `build_todo_gate_reminder` **lists every item**, in insertion order, with **no
  cap**, then says: *"advance the next pending todo with the appropriate tool
  call NOW"*.
- `in_progress_unbacked` is the first N in-progress items in insertion order —
  a deterministic partition, documented as such.
- `max_fires_per_prompt` (default in `xai-grok-agent/src/system_reminder.rs`)
  caps how often it fires, not what it says.
- Precedence: CLI `--todo-gate` > remote settings > **built-in default, which is
  disabled**. `main.rs:1557` ships `todo_gate: false`.

**The gate is off by default and has no TUI surface.** `settings/defs.rs:1943`
says so explicitly: *"Only the CLI flag (`--todo-gate`) is wired.
Settings-modal entries for `[reminder.todo_gate]` are deferred — the modal
dispatcher requires per-key action arms … that don't yet have a place to land."*

## 3. Measured: "next" means insertion order

The reminder tells the agent to advance *the next* pending todo. Next is
insertion order, and nothing checks whether that item is **actionable** — it can
be blocked on a credential, on another item, or on an undecided choice while a
later one is ready.

18 fixtures where blocked items sit first, each with the first actionable item as
gold:

| measure | result |
| --- | --- |
| insertion order picks the actionable item | 11/18 (61%) |
| Jev picks the actionable item | **15/18 (83%)** |
| Jev against insertion order | **+4 gains, 0 losses** |

**Strictly better, never worse on this set.** And the three cases Jev missed are
cases where insertion order also missed — the errors are on the genuinely
ambiguous lists, not on the easy ones.

Caveat that matters: the fixtures were *designed* with blocked-first patterns, so
61% reflects the design, not a measured production base rate. What transfers is
the shape — insertion order has no actionability signal at all, so on any list
where the first pending item is blocked it is wrong by construction.

## 4. Measured: the reminder has no cap

`build_todo_gate_reminder` dumps every pending item:

| pending items | reminder size, full dump | capped at 3 |
| --- | --- | --- |
| 1 | 450 chars | 450 |
| 3 | 548 | 548 |
| 5 | 646 | 548 |
| 10 | 891 | 548 |
| 20 | **1,391** | **548** |

Linear and unbounded, for a text whose actionable content is one line — the item
to advance next. The other nineteen are context the agent already has in its own
todo list.

## 5. Design

**The gate's decision stays deterministic.** Jev does not decide whether to fire;
it decides what to name.

| stage | attachment | fail-open |
| --- | --- | --- |
| fire/no-fire | unchanged — the four-line branch | unchanged |
| item selection | one `choice` over `pending`, when the list is longer than a threshold | insertion order (today) |
| cap | name the chosen item, plus at most N others | full dump (today) |

```
select: one choice over the pending items,
        "Which single pending item should it advance next? Prefer one it can act
         on now; skip items blocked on a credential, on another item, or on a
         decision that has not been made."

Only call it when pending.len() > 3 — below that the dump is already cheap and
insertion order is as good as anything.
```

The cap is the bigger win and needs no model: naming three items instead of
twenty cuts a long reminder by 60% and removes the only part of it that grows.

## 6. TUI configuration

**What exists.** Nothing in the modal. The CLI flag `--todo-gate` is the only
switch, and `settings/defs.rs:1943` documents the deferral and its reason (the
modal dispatcher needs per-key action arms that have no place to land yet).

**What is missing.** The whole surface: on/off, the fire cap, and the item cap.

**Shape, following `/jev`.** `/jev on|off|status` dispatches a typed
`Action::SetCompactionJevEnabled`, the same action the settings row dispatches;
the value persists and the live session picks it up through the config reload
fan-out.

```
/todo-gate on|off|status
  status  -> enabled, fires used this prompt, pending count, the item named

settings rows: "Todo gate" bool, "Max fires per prompt" u32,
               "Max items named" u32 (default 3), "Pick the item with Jev" bool
               (default off, and only consulted when pending > 3)
```

This is the one target where the TUI work is larger than the logic work: the gate
is finished and off, and the reason it is off is that nobody built the rows.

## 7. Implementation order

1. **Turn it on.** A working backstop, disabled by default, with a documented
   reason that is purely about UI plumbing.
2. **Cap the item dump at 3.** No model, 60% smaller on long lists.
3. **Then, and only for lists longer than 3, let Jev pick the item.** +4/0 on the
   fixtures, and it costs one call only when the list is long.
4. **Build the settings rows** so the flag stops being the only switch.

## 8. Hazards

- **The gate fires at turn end, so a wrong item costs a whole turn.** Same
  asymmetry as the laziness nudge, and the reason to keep the decision cheap.
- **Insertion order is not wrong, it is uninformed.** On lists where the first
  item is actionable it is right, so any change must be measured against it, not
  against a strawman.
- **A cap loses context.** Three items is a guess; the agent's own todo list has
  the rest, but if a long list is common the cap should be revisited.
- **Jev is unavailable → fall back to insertion order**, never to "name nothing".

## 9. Not verified

- 18 fixtures, hand-built with blocked-first patterns; 61% is my design's base
  rate, not a measured one.
- The gate was never executed; `evaluate_todo_gate` and the reminder builder were
  ported by hand for the size measurement.
- Whether naming three items instead of twenty changes the agent's behaviour was
  not measured — only the size.
- The `in_progress_unbacked` partition and `max_fires_per_prompt` were read, not
  exercised.
- Nothing was measured inside the TUI.