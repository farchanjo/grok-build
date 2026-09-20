"""Credential resolution for the Jev harness.

The user keeps long-lived keys in ``~/.llm-key``, a shell-sourceable file of
``export NAME="value"`` lines. Environment wins over the file so a one-off
``TYPESAFE_API_KEY=... python run_eval.py`` can override without editing it.
"""

from __future__ import annotations

import os
import re
from pathlib import Path

LLM_KEY_FILE = Path(os.environ.get("LLM_KEY_FILE", "~/.llm-key")).expanduser()

_EXPORT = re.compile(r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$")


class MissingKey(RuntimeError):
    """No credential resolved from the environment or the key file."""


def load_key_file(path: Path = LLM_KEY_FILE) -> dict[str, str]:
    """Parse ``export NAME="value"`` lines into a mapping.

    Values may be double-quoted, single-quoted or bare. Unparseable lines are
    skipped rather than fatal: the file is hand-maintained and also holds
    unrelated exports.
    """
    keys: dict[str, str] = {}
    if not path.is_file():
        return keys
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.lstrip().startswith("#"):
            continue
        match = _EXPORT.match(line)
        if not match:
            continue
        name, raw = match.group(1), match.group(2)
        value = raw.strip().strip('"').strip("'").strip()
        if value:
            keys[name] = value
    return keys


def resolve(*names: str, file: Path = LLM_KEY_FILE) -> tuple[str, str]:
    """First non-empty credential among ``names``, from env then the key file.

    Returns ``(value, source)`` so a report can state where the key came from
    without ever printing it.
    """
    keys: dict[str, str] | None = None
    for name in names:
        value = os.environ.get(name, "").strip()
        if value:
            return value, f"env:{name}"
    keys = load_key_file(file)
    for name in names:
        value = keys.get(name, "").strip()
        if value:
            return value, f"file:{file.name}:{name}"
    raise MissingKey(f"none of {', '.join(names)} in env or {file}")


def mask(value: str) -> str:
    """A key rendered for logs: enough to identify, never enough to leak."""
    return value if len(value) <= 14 else f"{value[:8]}…{value[-4:]}"