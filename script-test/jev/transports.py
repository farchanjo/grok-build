"""Transport factory: one call shape, two wires.

The TypeSafe evaluation endpoint and OpenRouter's alpha decisions proxy speak
the same request and answer shape, and differ in exactly three things: base URL,
model string, and whether a ``provider`` routing block is accepted. The factory
keeps those three as data so a caller never branches on the transport, which is
the shape the Rust factory will want as well.

The OpenRouter path is what the repository uses today (``[compaction.jev]``); the
native path is the documented, stable one. Running the same case set over both
is how we find out whether the alpha proxy really passes ``choice`` and
``score`` through, since its docs only describe the native surface.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Any

import httpx

from . import keys, primitives

NATIVE = "native"
OPENROUTER = "openrouter"
LAYA = "laya"


@dataclass(frozen=True)
class Transport:
    """Everything that differs between the wires."""

    name: str
    endpoint: str
    model: str
    key_names: tuple[str, ...]
    sends_provider_block: bool
    notes: str = ""
    requires_auth: bool = True
    # Laya validates ``state`` as an object; a bare string is a 422. When this is
    # set, ``Client.ask`` wraps a string state as ``{"message": ...}``.
    wraps_text_state: bool = False
    retry_statuses: frozenset[int] = frozenset({429, 529})


TRANSPORTS: dict[str, Transport] = {
    NATIVE: Transport(
        name=NATIVE,
        endpoint="https://api.typesafe.ai/v1/systemone",
        model="jev-latest",
        key_names=("TYPESAFE_API_KEY", "TYPESAFE_AI_FABRICIO_KEY"),
        sends_provider_block=False,
        notes="documented endpoint; rejects unknown top-level fields",
    ),
    OPENROUTER: Transport(
        name=OPENROUTER,
        endpoint="https://openrouter.ai/api/alpha/decisions",
        model="~typesafe/jev-latest",
        key_names=("GROK_JEV_API_KEY", "OPENROUTER_API_KEY"),
        sends_provider_block=True,
        notes="alpha proxy; same as [compaction.jev] in the repository",
    ),
    LAYA: Transport(
        name=LAYA,
        endpoint="http://192.168.200.32:8803/v1/decide",
        model="typed-decisions",
        key_names=(),
        sends_provider_block=False,
        notes="self-hosted LAN, no auth; state must be an object; checkpoints differ by ~2x",
        requires_auth=False,
        wraps_text_state=True,
        # Single-shot and deterministic, so a 5xx retry cannot change the answer.
        retry_statuses=frozenset({429, 500, 502, 503, 504}),
    ),
}


def factory(name: str, *, endpoint: str | None = None, model: str | None = None) -> Transport:
    """Resolve a transport by name, with optional endpoint/model overrides."""
    try:
        base = TRANSPORTS[name]
    except KeyError:
        raise ValueError(f"unknown transport {name!r}; known: {', '.join(sorted(TRANSPORTS))}") from None
    if endpoint or model:
        return Transport(
            name=base.name,
            endpoint=endpoint or base.endpoint,
            model=model or base.model,
            key_names=base.key_names,
            sends_provider_block=base.sends_provider_block,
            notes=base.notes,
            requires_auth=base.requires_auth,
            wraps_text_state=base.wraps_text_state,
            retry_statuses=base.retry_statuses,
        )
    return base


class TransportError(RuntimeError):
    """A non-2xx response or a transport failure, with the status attached."""

    def __init__(self, message: str, status: int | None = None, body: str = "") -> None:
        super().__init__(message)
        self.status = status
        self.body = body


@dataclass
class Client:
    """A resolved transport plus one connection pool."""

    transport: Transport
    api_key: str = field(repr=False, default="")
    key_source: str = ""
    provider: dict[str, Any] | None = None
    timeout_s: float = 30.0
    _http: httpx.Client = field(init=False, repr=False)

    def __post_init__(self) -> None:
        headers = {"Content-Type": "application/json"}
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        self._http = httpx.Client(timeout=httpx.Timeout(self.timeout_s), headers=headers)

    @classmethod
    def build(
        cls,
        name: str,
        *,
        provider: dict[str, Any] | None = None,
        endpoint: str | None = None,
        model: str | None = None,
        **client_kwargs: Any,
    ) -> Client:
        """Resolve a transport, its credential, and a client in one step.

        ``endpoint``/``model`` are factory overrides; every other keyword goes to
        the client, so a caller never has to know which layer a knob belongs to.
        A transport with no ``key_names`` (Laya, on the LAN) resolves to no
        credential and sends no ``Authorization`` header.
        """
        transport = factory(name, endpoint=endpoint, model=model)
        if transport.key_names:
            key, source = keys.resolve(*transport.key_names)
        else:
            key, source = "", "none (lan)"
        return cls(transport=transport, api_key=key, key_source=source, provider=provider, **client_kwargs)

    def close(self) -> None:
        self._http.close()

    def __enter__(self) -> Client:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def _normalise_state(self, state: Any) -> Any:
        """Laya validates ``state`` as an object; Jev accepts a bare string too."""
        if self.transport.wraps_text_state and isinstance(state, str):
            return {"message": state}
        return state

    def ask(self, state: Any, questions: dict[str, Any], *, attempts: int = 3) -> primitives.Response:
        """One evaluation request, retrying 429/529 with backoff.

        Both endpoints document overload and rate-limit statuses, so a case set
        that dies on the first 429 would under-report. Raises TransportError on
        anything else, and on the last retryable failure.
        """
        body: dict[str, Any] = {
            "model": self.transport.model,
            "state": self._normalise_state(state),
            "questions": questions,
        }
        if self.transport.sends_provider_block and self.provider:
            body["provider"] = self.provider
        retryable = set(self.transport.retry_statuses)
        for attempt in range(1, attempts + 1):
            started = time.perf_counter()
            try:
                response = self._http.post(self.transport.endpoint, json=body)
            except httpx.HTTPError as error:
                if attempt == attempts:
                    raise TransportError(f"{type(error).__name__}: {error}") from error
                time.sleep(0.5 * 2 ** (attempt - 1))
                continue
            latency_ms = (time.perf_counter() - started) * 1000.0
            text = response.text
            if response.status_code == 200:
                break
            if response.status_code in retryable and attempt < attempts:
                time.sleep(0.5 * 2 ** (attempt - 1))
                continue
            raise TransportError(
                f"{self.transport.name} returned {response.status_code}",
                status=response.status_code,
                body=text[:400],
            )
        try:
            payload = response.json()
        except ValueError as error:
            raise TransportError(f"response was not JSON: {error}", body=text[:400]) from error
        raw_answers = payload.get("answers")
        if not isinstance(raw_answers, dict):
            raise TransportError(f"response has no `answers` object: {text[:400]}")
        usage = payload.get("usage") or {}
        return primitives.Response(
            answers=primitives.parse_answers(raw_answers),
            model=str(payload.get("model", self.transport.model)),
            input_tokens=int(usage.get("input_tokens") or 0),
            output_tokens=int(usage.get("output_tokens") or 0),
            latency_ms=latency_ms,
        )


def probe(name: str, *, timeout_s: float = 30.0) -> dict[str, Any]:
    """One cheap mixed-primitive call: does this transport answer at all?

    Exercises ``noul``, ``choice`` and ``score`` in a single request, so a
    transport that only supports a subset fails loudly here instead of midway
    through a case set.
    """
    with Client.build(name, timeout_s=timeout_s) as client:
        questions = {
            "is_urgent": primitives.noul(
                "Does this message express urgency?",
                true="Explicitly time-sensitive",
                false="No urgency expressed",
            ),
            "department": primitives.choice(
                "Which team should handle this?",
                {
                    "billing": "Payments, invoicing, refunds",
                    "technical": "Bugs, outages, integrations",
                },
            ),
            "frustration": primitives.score(
                "How frustrated is the customer?",
                ["Calm", "Frustrated", "Very angry"],
            ),
        }
        state = "Hi, I've been trying to connect my Stripe account for 3 days and it keeps failing."
        response = client.ask(state, questions)
        return {
            "transport": name,
            "endpoint": client.transport.endpoint,
            "model": response.model,
            "key_source": client.key_source,
            "noul": response.noul("is_urgent"),
            "choice": response.choice("department").choice,
            "confidence": response.choice("department").confidence,
            "score": response.score("frustration").score,
            "input_tokens": response.input_tokens,
            "latency_ms": round(response.latency_ms, 1),
        }