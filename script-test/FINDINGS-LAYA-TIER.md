# Two-tier decision policy: integration and verification — findings

Built and measured 2026-09-22 against the live `laya-decisions` container and the
TypeSafe endpoint, on the harness's own case sets. Companion to
`HARNESS-INTEGRATION.md` (the brief) and `FINDINGS-LAYA-PARAMETERS.md` (the tuning
question). Everything below is measured; the places where the brief does not
transfer are called out explicitly.

## 1. What was built

| Piece | Where | What it does |
| --- | --- | --- |
| `laya` transport | `jev/transports.py` | LAN endpoint, no credential, dict-only `state`, 5xx retry |
| Tier policy | `jev/tier.py` | Laya first, per-question gate, Jev on fallthrough, split flagged |
| Provenance | `jev/primitives.py` | `Response.source` / `escalated` / `agreement` / `primary_confidence` |
| CLI | `jev_ask.py --transport tiered` | one-command tier call |
| Eval | `run_eval.py --transport tiered` | the tier as a first-class transport |
| Verification | `verify_tier.py` | three arms — Laya, Jev, tier — on the same cases |
| Gate sweep | `sweep_tier_gate.py` | offline threshold sweep from a verification run |
| Position control | `control_workflow_position.py` | shuffles candidates to break the case set's defect |

The tier mirrors `Client.ask`, so every existing experiment runs against it
unchanged. `Response.source` names the engine whose answer was returned,
`escalated` means a paid call happened, and `agreement is False` is the
send-it-to-a-human flag.

## 2. Structural property that governs everything

The tier returns Jev's answer on every escalated case and Laya's on every
gate-passed case. When the engines agree the answers are identical either way. So:

> **The tier can only differ from Jev alone on the gate-passed subset.**

This makes the "gained/lost vs Jev" table nearly tautological — 0 gained and 0 lost
is the expected outcome of a well-set gate, not a triumph. The numbers that matter
are the size of the gate-passed subset (paid calls avoided) and its accuracy.

## 3. Measured result

Three repeats per arm; the arms run concurrently on the same cases.

| experiment | laya | jev | tier | paid calls | avoided | gate-passed |
| --- | --- | --- | --- | --- | --- | --- |
| skills | 20.4% | 68.5–70.4% | 68.5–70.4% | 52/54 | **4%** | 2/66 (100% right) |
| agents | 36.0% | 80.0% | 80.0% | 25/25 | **0%** | 0/25 |
| memory | 53.3% | 70.0–73.3% | 70.0–73.3% | 29/30 | **3%** | 1/30 (right) |
| workflow | 60.0% | 40.0–46.7% | 60.0% | 5/15 | **67%** | 10/15 (60% right) |

Read it as: **on three of four experiments the tier is Jev with 0–4% of the paid
calls removed — effectively a no-op for the cost it adds.** On workflow it is
materially different and better, but that is a case-set artefact (below).

Laya itself is fully deterministic across repeats (20.4%, 53.3%, 60.0% with zero
spread). Jev is **not**: 68.5–70.4% on skills and 40.0–46.7% on workflow across
three runs of the same cases, with the resolved model id pinned
(`typesafe/jev-1.13-20260917`). The brief states Jev is deterministic for the same
resolved build; measured, it is not, and one skill case flipped between
`ripgrep-search` and `grep-search` on consecutive runs. Any gate calibrated
against a single Jev run is calibrated against noise of the same order as the
effect being measured.

## 4. The workflow result is an artefact, and the control shows it

The tier "beats Jev by 13–20 points" on workflow naming only because it stops
escalating there. That looked like Laya being good at naming. It is mostly the
case set:

- `workflow_cases.json` puts the gold at candidate position 0 in **14 of 15** cases
  (the harness README flags this as an unfixed defect).
- Laya picks position 0 in 9 of 15 cases, and **all 9 of its hits are cases where
  the gold sits at position 0**. Jev picks position 0 only 5 of 15.
- Shuffling candidates per case, four seeds: **Laya 45.0% (sd 6.4), Jev 55.0%
  (sd 6.4)** — the ranking reverses, and Laya's position histogram stays skewed
  (22/60 picks at position 0) while Jev's flattens.

So on a fair read of workflow, Jev is ahead and Laya leans on position. The
`name` gate of 0.05 is tuned on the confounded ordering.

## 5. Where the brief does not transfer

| Brief says | Measured |
| --- | --- |
| `state` may be a string, dict or list | Laya **422s on a bare string**; only an object works. `laya` transport wraps it. |
| `GATE = 0.70`, a single constant | Per-question. `confidence` is `1 - H(p)/log(k)`, so it scales with the option count and the checkpoint: a 2-option choice at 0.57/0.43 scores **0.013**, a 172-option choice scores 0.28–0.89 on the same engine. A global 0.70 accepts nothing on a 2-option question. |
| `temperature` inert | True for accuracy (11/66 at every t in 0.05–5.0, identical pick distribution) but **not** for confidence, which moves 500× (0.0019→0.9614). And it is not reachable through the API at all. |
| "escalate to the big LLM" on disagreement | There is no third engine in this harness, so a split returns Jev's answer and sets `agreement=False` for a human. |
| Laya is the cheap first pass for the easy majority | True in shape, but Laya is 20–53% accurate on three of four workloads against Jev's 70–80%, so the "easy majority" is small. To match Jev's accuracy the gate must escalate 96–100% of cases. |

## 6. What would make the tier worth running

The tier's value is entirely in the gate-passed subset, so the question is how to
grow it without losing accuracy. In order of measured promise:

1. **Fix the skills payload first.** Chunking the 172-option list takes Laya from
   20.4% to 30.3% (`rescue_chunked.py`), which raises the confidence of correct
   answers and should widen the gate-passed subset. Not yet re-measured with the
   tier.
2. **Loosen the gate and accept a measured loss.** The offline sweep shows skills
   at T=0.50 → 63.0% (7 points down) for ~26% of paid calls removed, and agents at
   T=0.40 → 76% (4 points down) for 24%. The current gates were set to *tie*, not
   to trade.
3. **Drop the tier where it does nothing.** Agents saved 0% of paid calls; the
   honest configuration for agents is Jev alone, or a much lower gate.
4. **Re-tune `name` after shuffling the case set**, since the current value is fit
   to a position-biased ordering.

## 7. Not verified

- Whether chunking actually widens the gate-passed subset; measured separately,
  never composed with the tier.
- Cost of the Laya-first leg in wall time: p50 for the tier tracks Jev (364 ms vs
  383 ms on skills), so the 15 ms Laya leg is invisible when it escalates — but on
  workflow it drops the tier to 54 ms.
- The `agents` gate: 0 of 25 cases passed at 0.70, so that threshold is set by
  nothing but the sweep's tie.
- Anything about a third engine. The brief's escalation path assumes one exists.
- Concurrency beyond the harness's 8 workers; the brief claims 8 parallel calls
  succeed and this run is consistent with that but does not test a ceiling.