"""Typed Jev questions and answers.

Mirrors the wire format of the TypeSafe evaluation endpoint: three question
types (``noul``, ``choice``, ``score``) and their matching answers. Keeping the
builders and the parser in one module means a schema change lands in one place,
which is the property the Rust factory will need too.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

QuestionType = Literal["noul", "choice", "score"]


class BadAnswer(ValueError):
    """The response carried no answer, or an answer of the wrong shape."""


def noul(instructions: str, *, true: str | None = None, false: str | None = None) -> dict[str, Any]:
    """A yes/no question returning the probability that the answer is yes."""
    question: dict[str, Any] = {"type": "noul", "instructions": instructions}
    if true or false:
        question["criteria"] = {k: v for k, v in (("true", true), ("false", false)) if v}
    return question


def choice(instructions: str, criteria: dict[str, str | None]) -> dict[str, Any]:
    """Pick one option out of ``criteria``; option keys are the returned labels."""
    if len(criteria) < 2:
        raise ValueError("a choice question needs at least two options")
    return {"type": "choice", "instructions": instructions, "criteria": criteria}


def score(instructions: str, criteria: list[str]) -> dict[str, Any]:
    """Rate the state along an ordered rubric of at least two levels."""
    if len(criteria) < 2:
        raise ValueError("a score question needs at least two levels")
    return {"type": "score", "instructions": instructions, "criteria": criteria}


@dataclass(frozen=True)
class NoulAnswer:
    noul: float


@dataclass(frozen=True)
class ChoiceAnswer:
    choice: str
    probabilities: dict[str, float]
    confidence: float

    def ranked(self) -> list[tuple[str, float]]:
        """Options by descending probability, which is the ranking to read."""
        return sorted(self.probabilities.items(), key=lambda kv: -kv[1])


@dataclass(frozen=True)
class ScoreAnswer:
    score: float
    legend: dict[str, str]
    probabilities: dict[str, float]
    confidence: float


Answer = NoulAnswer | ChoiceAnswer | ScoreAnswer


@dataclass
class Response:
    """One request's answers, plus what it cost and how long it took."""

    answers: dict[str, Answer] = field(default_factory=dict)
    model: str = ""
    input_tokens: int = 0
    output_tokens: int = 0
    latency_ms: float = 0.0

    def noul(self, key: str) -> float:
        answer = self.answers.get(key)
        if not isinstance(answer, NoulAnswer):
            raise BadAnswer(f"{key!r} is {type(answer).__name__}, not a noul answer")
        return answer.noul

    def choice(self, key: str) -> ChoiceAnswer:
        answer = self.answers.get(key)
        if not isinstance(answer, ChoiceAnswer):
            raise BadAnswer(f"{key!r} is {type(answer).__name__}, not a choice answer")
        return answer

    def score(self, key: str) -> ScoreAnswer:
        answer = self.answers.get(key)
        if not isinstance(answer, ScoreAnswer):
            raise BadAnswer(f"{key!r} is {type(answer).__name__}, not a score answer")
        return answer


def _as_float(value: Any, where: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise BadAnswer(f"{where} is {value!r}, not a number")
    return float(value)


def parse_answers(raw: dict[str, Any]) -> dict[str, Answer]:
    """Turn the ``answers`` object into typed answers, keyed by question id."""
    parsed: dict[str, Answer] = {}
    for key, body in raw.items():
        if not isinstance(body, dict):
            raise BadAnswer(f"answer {key!r} is {type(body).__name__}, not an object")
        kind = body.get("type")
        if kind == "noul":
            parsed[key] = NoulAnswer(noul=_as_float(body.get("noul"), f"{key}.noul"))
        elif kind == "choice":
            probabilities = body.get("probabilities")
            if not isinstance(probabilities, dict):
                raise BadAnswer(f"{key}.probabilities is not an object")
            parsed[key] = ChoiceAnswer(
                choice=str(body.get("choice", "")),
                probabilities={k: _as_float(v, f"{key}.probabilities.{k}") for k, v in probabilities.items()},
                confidence=_as_float(body.get("confidence", 0.0), f"{key}.confidence"),
            )
        elif kind == "score":
            probabilities = body.get("probabilities") or {}
            legend = body.get("legend") or {}
            parsed[key] = ScoreAnswer(
                score=_as_float(body.get("score"), f"{key}.score"),
                legend={str(k): str(v) for k, v in legend.items()} if isinstance(legend, dict) else {},
                probabilities={k: _as_float(v, f"{key}.probabilities.{k}") for k, v in probabilities.items()}
                if isinstance(probabilities, dict)
                else {},
                confidence=_as_float(body.get("confidence", 0.0), f"{key}.confidence"),
            )
        else:
            raise BadAnswer(f"answer {key!r} has unknown type {kind!r}")
    return parsed