"""Two-tier decision policy: cheap engine first, paid engine on uncertainty.

Laya is a self-hosted LAN model (15 ms, $0 marginal); Jev is the TypeSafe
evaluation endpoint (~400 ms, paid). They speak the same wire contract, so the
tier is a policy layer over two interchangeable clients, not a new protocol.

The shape is the one the integration brief recommends: ask the cheap engine,
accept when it is sure, otherwise buy the second opinion. Two corrections come
from measuring the harness's own workloads rather than the brief's support-intent
payload:

1. **The gate is per question, not global.** ``confidence`` is normalised Shannon
   entropy, ``1 - H(p)/log(k)``, so its scale moves with the option count and with
   the checkpoint. A 2-option choice scoring 0.57/0.43 lands at confidence 0.013;
   a 172-option choice lands at 0.28..0.89 on the same engine. The brief's
   ``GATE = 0.70`` is right for its 6-option intent question and wrong everywhere
   else, so the threshold is data (``GATES``), not a constant.

2. **Escalation is the default, not the exception.** Where Laya trails Jev badly
   (wide, near-duplicate option sets), a low gate is what keeps the tier from
   *losing* accuracy. The verification run reports the net, per experiment.

A disagreement between the two engines is the escalation trigger: it marks a case
for review even though the tier still returns Jev's answer, because the tier has
no third engine to consult.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from . import primitives
from .transports import Client

# Measured on cases/ with model="typed-decisions", by sweeping a verification run
# offline (sweep_tier_gate.py). These values are set where the tier stops losing
# accuracy against Jev alone — deliberately conservative, and the price is that
# the gate almost never fires: 4% of paid calls avoided on skills, 0% on agents,
# 3% on memory. Lower them to trade accuracy for calls (see FINDINGS-LAYA-TIER.md
# §6). ``name`` is fitted to a case set whose gold sits at position 0 in 14 of 15
# cases, so it should be re-derived after shuffling.
# A question absent from this map uses ``default_gate``.
GATES: dict[str, float] = {
    # skills/agents share this id. 0.70 is set by skills (54 graded cases): 0.50
    # costs 7 points there, while 0.70 still ties agents and saves 4%.
    "which": 0.70,
    "should_store": 0.60,  # memory: no separation below 0.6, so escalate
    "name": 0.05,          # workflow: 4 options, whole distribution under 0.30
}
DEFAULT_GATE = 0.30


@dataclass
class TierStats:
    """Per-run counters, so a report can state what the gate actually did."""

    calls: int = 0
    accepted: int = 0
    escalated: int = 0
    agreed: int = 0
    disagreed: int = 0
    primary_ms: float = 0.0
    secondary_ms: float = 0.0

    @property
    def escalation_rate(self) -> float:
        return self.escalated / self.calls if self.calls else 0.0


@dataclass
class TieredClient:
    """A primary decision engine gated in front of a secondary one.

    ``ask`` mirrors ``transports.Client.ask`` so an existing caller — including
    ``run_eval.py`` — swaps one for the other with no other change.
    """

    primary: Client
    secondary: Client
    gates: dict[str, float] = field(default_factory=lambda: dict(GATES))
    default_gate: float = DEFAULT_GATE
    primary_question: str | None = None
    stats: TierStats = field(default_factory=TierStats)

    def __enter__(self) -> TieredClient:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def close(self) -> None:
        self.primary.close()
        self.secondary.close()

    @classmethod
    def build(
        cls,
        primary: str = "laya",
        secondary: str = "openrouter",
        *,
        gates: dict[str, float] | None = None,
        default_gate: float = DEFAULT_GATE,
        primary_question: str | None = None,
        primary_model: str | None = None,
        timeout_s: float = 60.0,
    ) -> TieredClient:
        return cls(
            primary=Client.build(primary, model=primary_model, timeout_s=timeout_s),
            secondary=Client.build(secondary, timeout_s=timeout_s),
            gates=dict(gates) if gates else dict(GATES),
            default_gate=default_gate,
            primary_question=primary_question,
        )

    def gate_for(self, key: str) -> float:
        return self.gates.get(key, self.default_gate)

    def _driver(self, questions: dict[str, Any]) -> str:
        """The question whose confidence decides whether Jev is bought at all."""
        if self.primary_question:
            return self.primary_question
        return sorted(questions)[0]

    def ask(self, state: Any, questions: dict[str, Any], *, attempts: int = 3) -> primitives.Response:
        """Cheap answer when it is confident, otherwise the paid one.

        Returns the primary's answers when the gate passes, or when both engines
        agree. Returns the secondary's answers on a split, since the secondary is
        the more accurate of the two on every workload measured so far.

        Provenance on the returned response:

        - ``source`` — the engine whose answer this is. On agreement that is the
          primary, because the two answers are identical and the primary's object
          is the one returned.
        - ``escalated`` — the secondary was consulted, i.e. a paid call happened.
        - ``agreement`` — ``None`` when no second opinion was bought, else
          ``True``/``False``. A ``False`` here is the "send it to a human" flag.
        """
        driver = self._driver(questions)
        first = self.primary.ask(state, questions, attempts=attempts)
        self.stats.calls += 1
        self.stats.primary_ms += first.latency_ms

        confidence = first.confidence(driver)
        if confidence >= self.gate_for(driver):
            self.stats.accepted += 1
            first.source = self.primary.transport.name
            first.primary_confidence = confidence
            return first

        second = self.secondary.ask(state, questions, attempts=attempts)
        self.stats.escalated += 1
        self.stats.secondary_ms += second.latency_ms

        agreed = first.pick(driver) == second.pick(driver)
        chosen = first if agreed else second
        if agreed:
            self.stats.agreed += 1
        else:
            self.stats.disagreed += 1

        chosen.source = self.primary.transport.name if agreed else self.secondary.transport.name
        chosen.primary_confidence = confidence
        chosen.agreement = agreed
        chosen.escalated = True
        chosen.secondary_model = second.model
        chosen.secondary_latency_ms = second.latency_ms
        return chosen