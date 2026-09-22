# Laya tuning: what the parameters actually do — findings

Verified 2026-09-22 against the live `laya-decisions` container on
`192.168.200.32` (checkpoint `typed-decisions`, RTX 4090, `gpu_active: true`).
Companion to `HARNESS-INTEGRATION.md`; every number below is measured, and the
places where the brief's §4 does not transfer are called out.

Question asked: can Laya's parameters — `temperature` and others — be raised to
improve accuracy on the harness's workloads?

Short answer: **`temperature` cannot change an accuracy number, the budget knobs
are unreachable through the API and barely matter anyway, and the one lever that
does work is the payload shape.**

---

## 1. No parameter reaches the API today

The server's request model has exactly three fields:

```python
# /root/laya-verify/server.py
class DecideRequest(BaseModel):
    state: dict
    questions: dict
    model: str = 'auto'
```

and it forwards them to a signature with no tuning arguments:

```
Router.predict(state, questions, model=None, task=None, lang=None)
Agent.system_one(state, questions)
```

Pydantic drops unknown fields silently, so nine spellings of the obvious knobs —
`temperature`, `temperature` as a list, `temp`, `temperature_by_options`,
`max_len`, `head_max_len`, `cfg`, `options`, and a deliberate typo — all returned
**byte-identical** responses. The API accepts them, ignores them, and never
errors: there is no way to tell a working knob from a typo.

`temperature` and the budgets are reachable only by mutating the `Agent` object
(`agent.temperature = [...]`, `agent.cfg["head_max_len"] = ...`), which is what
the `sweep_*.py` scripts in `~/laya-verify` do — inside the container.

## 2. `temperature` changes confidence, never the pick

Swept in-container on the 66-case skill set, `typed-decisions`, bucketed
(`choice:11+`, which is the 172-option skill shape) and scalar:

| `choice:11+` | choice accuracy | mean confidence | distinct picks |
| --- | --- | --- | --- |
| 0.05 | 11/66 | 0.9614 | 36 |
| 0.25 | 11/66 | 0.6950 | 36 |
| **0.5 (effective shipped)** | **11/66** | **0.2994** | **36** |
| 1.0 | 11/66 | 0.0596 | 36 |
| 2.0 | 11/66 | 0.0124 | 36 |
| 5.0 | 11/66 | 0.0019 | 36 |

Forcing every bucket to a single scalar `t` ∈ {0.1, 0.5, 1, 2, 5} gives the same
11/66 and the same 36 distinct picks. The brief's reasoning is right and holds
even on a checkpoint that actually ships bucket overrides: `argmax(softmax(z/t))`
cannot reorder for positive `t`, so a choice label is temperature-invariant.

Three consequences the brief does not state:

- **Confidence moves 500×** across that range (0.0019 → 0.9614). A gate threshold
  is meaningless without pinning the temperature it was calibrated at, and the
  same is true in reverse.
- **The shipped value is not the effective one.** `typed-decisions` ships
  `choice:11+ = 0.1006`; the library clamps to `[0.5, 5]` and warns:
  *"clamping choice:11+=0.1006. Treat confidence from the affected buckets as
  uncalibrated."* So every wide-choice confidence this checkpoint reports — which
  is every skills and agents number — is flagged uncalibrated by the library
  itself.
- The brief's §4 sweep was run on `multilingual`, which ships `[1.0, 1.0, 1.0]`
  and an empty `temperature_by_options`. It concluded "nothing to tune there
  anyway" and generalised. `typed-decisions` is the counterexample: it ships six
  bucket overrides.

## 3. The budget knobs are a truncation bug, and widening them does not help

`build_sequence` gives every option an equal share of the head budget:

```python
opt_budget = head_max_len - sum(len(o) for o in opt_ids)
if opt_budget < 16:
    per = max(4, (head_max_len - 16) // max(1, len(opt_ids)))
    opt_ids = [o[:per] for o in opt_ids]
```

At the shipped `head_max_len=256` with the 172-skill roster that is
`max(4, 240 // 172) = 4`. **Every option is cut to four tokens** — the engine
reads skill names and almost none of the descriptions, which is where the
brief's §4 says the accuracy comes from. Token counts confirm it: at 172 options
a 10-character and a 240-character description both produce 704 input tokens.

Hard limits found on the way:

| Limit | Behaviour |
| --- | --- |
| > 256 options | HTTP 400 `question 'which' options exceed head_max_len=256` |
| sequence > `max_len` (1024) | `system_one` raises `options exceed head_max_len` |
| head budget overflow | silent equal-share truncation to as little as 4 tokens |

Widening both, on the flat 172-option list, makes accuracy **worse**:

| `max_len` | `head_max_len` | tokens per option | accuracy |
| --- | --- | --- | --- |
| 1024 | 256 | 4 | 9/66 |
| 2048 | 1024 | 5 | 8/66 |
| 4096 | 2048 | 11 | 3/66 |
| 8192 | 4096 | 23 | **0/66** |

More description text is actively harmful at that width.

## 4. The lever that works: chunk the option list

Split the roster, one `choice` question per chunk, then re-rank the survivors with
their full text. Two Laya calls per case, still ~60 ms and $0. Reproduced through
the plain API, no container access, no server patch:

| chunk | gold survived | accuracy | vs flat 172 (20.4%) |
| --- | --- | --- | --- |
| 6 | 37/66 | 24.2% | +3.8 |
| 8 | 35/66 | 24.2% | +3.8 |
| **12** | **30/66** | **30.3%** | **+9.9** |
| 16 | 28/66 | 24.2% | +3.8 |

Measured in-container at the same time: the shipped budget (1024/256) and a wide
budget (8192/4096) score within noise of each other at every chunk size. **So the
budget knobs are not worth patching the server for** — chunking, which works
through the API today, captures the whole effect.

A `noul` stage 1 ("does this chunk contain the right skill?") is much worse than a
`choice` stage 1: 5/66 against 19/66 at chunk 6. The filter saturates and the
recall ceiling drops from 56% to 32%.

## 5. Where this leaves the tier

Chunking lifts Laya on skills from 20.4% to 30.3%. Jev scores 70.4% on the same
set. So the tier's shape does not change — Jev stays the accurate engine and Laya
stays the cheap pre-filter — but the gate's meaning does:

- pin the checkpoint and the temperature before calibrating any threshold, or the
  number is not portable;
- treat wide-choice confidence from `typed-decisions` as uncalibrated (the
  library says so);
- `head_max_len` is a hard 256-option ceiling, so a roster above it must be
  chunked or the request 400s.

## 6. Not verified

- Whether raising `head_max_len` past 4096 helps; ModernBERT's context is the
  likely ceiling and the equal-share formula is the binding constraint long
  before it.
- Chunking with a *retrieval* stage (embedding shortlist) rather than fixed
  slices. Fixed chunks of 12 leave the gold outside the survivors 55% of the
  time, which is the current ceiling on the whole approach.
- Anything on `english`, which was not the checkpoint under test.
- Whether the harness's other experiments (agents at 28 options, memory, the
  4-option workflow) respond the same way; agents at 28 options already sits at
  `per = 8` and may not be truncated enough to matter.