#!/usr/bin/env python3
"""Deterministic OpenAI OpenAPI → endpoint_inventory.json generator.

Pins the official openai/openai-openapi tree at an immutable commit SHA.
Consumes either:
  - the exact pinned YAML blob (--source-yaml), preferred integrity path, or
  - a pre-converted JSON view (--input) plus verified source YAML metadata.

No network I/O.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import date, datetime
from pathlib import Path
from typing import Any

# Shared helpers live next to both generators.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "_shared"))
from openapi_contract import enumerate_endpoints, schema_fields  # noqa: E402

FORMAT_VERSION = 2
PROVIDER = "openai"

# Immutable pin (resolved 2026-07-25 against blob SHA-256 b58d6cd…).
SOURCE_REVISION = "5c044be3bf3a42854e99e34616564eeb2124a317"
SOURCE_URL = (
    f"https://raw.githubusercontent.com/openai/openai-openapi/"
    f"{SOURCE_REVISION}/openapi.yaml"
)
DOCS_URL = "https://platform.openai.com/docs/api-reference"
REPO_URL = "https://github.com/openai/openai-openapi"
LICENSE_NOTE = (
    "OpenAPI description from openai/openai-openapi (MIT) at the pinned commit. "
    "Compact inventory is derived public shape metadata only."
)

PRIORITY_ENDPOINTS: list[str] = [
    "POST /chat/completions",
    "POST /responses",
    "POST /embeddings",
    "GET /models",
    "POST /audio/speech",
    "POST /audio/transcriptions",
    "POST /files",
    "GET /files",
]

KEY_SCHEMAS: list[str] = [
    "CreateChatCompletionRequest",
    "CreateResponse",
    "CreateEmbeddingRequest",
    "CreateSpeechRequest",
    "CreateTranscriptionRequest",
    "CreateFileRequest",
    "ListModelsResponse",
    "Model",
    "ChatCompletionRequestMessage",
    "ChatCompletionTool",
]


def json_default(o: Any) -> Any:
    if isinstance(o, (date, datetime)):
        return o.isoformat()
    raise TypeError(type(o))


def load_yaml_as_data(yaml_bytes: bytes) -> dict[str, Any]:
    try:
        import yaml  # type: ignore
    except ImportError as e:
        raise SystemExit(
            "PyYAML required to parse --source-yaml; install pyyaml or pass --input JSON"
        ) from e
    data = yaml.safe_load(yaml_bytes)
    if not isinstance(data, dict):
        raise SystemExit("OpenAPI root must be an object")
    # Round-trip through JSON to normalize dates like generators consuming --input.
    return json.loads(json.dumps(data, default=json_default))


def build_inventory(
    data: dict[str, Any],
    *,
    fetched_at_utc: str,
    source_sha256: str,
    source_bytes: int,
    source_format: str,
    converted_json_sha256: str | None,
) -> dict[str, Any]:
    info = data.get("info") or {}
    paths = data.get("paths") or {}
    endpoints, schemas = enumerate_endpoints(data)

    endpoint_keys = {f"{e['method']} {e['path']}" for e in endpoints}
    if len(endpoint_keys) != len(endpoints):
        raise SystemExit("duplicate method+path identities")
    missing = [p for p in PRIORITY_ENDPOINTS if p not in endpoint_keys]
    if missing:
        raise SystemExit(f"priority endpoints missing: {missing}")

    field_inventory = []
    for name in KEY_SCHEMAS:
        inv = schema_fields(schemas, name)
        if inv is not None:
            field_inventory.append(inv)

    baseline: dict[str, Any] = {
        "title": info.get("title"),
        "version": str(info.get("version")) if info.get("version") is not None else None,
        "openapi": data.get("openapi"),
        "source_url": SOURCE_URL,
        "source_revision": SOURCE_REVISION,
        "docs_url": DOCS_URL,
        "repo_url": REPO_URL,
        "license_note": LICENSE_NOTE,
        "source_format": source_format,
        "fetched_at_utc": fetched_at_utc,
        "content_sha256": source_sha256,
        "content_bytes": source_bytes,
        "path_count": len(paths) if isinstance(paths, dict) else 0,
        "endpoint_count": len(endpoints),
        "schema_count": len(schemas),
    }
    if converted_json_sha256:
        baseline["converted_json_sha256"] = converted_json_sha256

    return {
        "format_version": FORMAT_VERSION,
        "provider": PROVIDER,
        "baseline": baseline,
        "endpoints": endpoints,
        "coding_agent_schema_fields": field_inventory,
        "coding_agent_priority_endpoints": list(PRIORITY_ENDPOINTS),
        "notes": [
            "Compact inventory only; full OpenAPI YAML is not vendored.",
            "source_url is commit-addressed (immutable); source_revision is the full git SHA.",
            "OpenAI path templates omit the /v1 prefix (API base URL includes it).",
            "transports is a multi-label set; stream-flag ops include http_json + http_sse.",
            "Binary response ops are never collapsed to sole http_json.",
            "OpenRouter is not the full OpenAI platform.",
            "No network I/O in the generator or unit tests.",
        ],
    }


def canonical_json(obj: Any) -> str:
    return json.dumps(obj, indent=2, ensure_ascii=False) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-yaml",
        type=Path,
        help="Exact pinned OpenAPI YAML blob (integrity verified via --expect-source-sha256)",
    )
    parser.add_argument(
        "--input",
        type=Path,
        help="Pre-converted OpenAPI JSON (must pair with --source-yaml for full chain)",
    )
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--fetched-at-utc", required=True)
    parser.add_argument(
        "--expect-source-sha256",
        required=True,
        help="Expected SHA-256 of the immutable source YAML (or JSON if only --input)",
    )
    parser.add_argument(
        "--expect-source-bytes",
        required=True,
        type=int,
        help="Expected byte length of the immutable source blob",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)

    converted_json_sha256: str | None = None

    if args.source_yaml:
        yaml_bytes = args.source_yaml.read_bytes()
        sha = hashlib.sha256(yaml_bytes).hexdigest()
        if sha != args.expect_source_sha256:
            print(
                f"source YAML sha mismatch: got {sha} expected {args.expect_source_sha256}",
                file=sys.stderr,
            )
            return 1
        if len(yaml_bytes) != args.expect_source_bytes:
            print(
                f"source YAML size mismatch: got {len(yaml_bytes)} "
                f"expected {args.expect_source_bytes}",
                file=sys.stderr,
            )
            return 1
        data = load_yaml_as_data(yaml_bytes)
        # Digested converted JSON view for the chain.
        converted = json.dumps(data, default=json_default, sort_keys=True).encode("utf-8")
        converted_json_sha256 = hashlib.sha256(converted).hexdigest()
        if args.input:
            # Optional consistency: input JSON must match conversion of YAML.
            input_data = json.loads(args.input.read_text(encoding="utf-8"))
            # Compare structural JSON (keys normalized).
            a = json.dumps(data, sort_keys=True, default=json_default)
            b = json.dumps(input_data, sort_keys=True, default=json_default)
            if a != b:
                print(
                    "ERROR: --input JSON does not match conversion of --source-yaml",
                    file=sys.stderr,
                )
                return 1
        source_format = "yaml"
        source_sha = sha
        source_bytes = len(yaml_bytes)
    elif args.input:
        # JSON-only path: pin is the JSON blob itself (discouraged for OpenAI).
        raw = args.input.read_bytes()
        sha = hashlib.sha256(raw).hexdigest()
        if sha != args.expect_source_sha256:
            print(
                f"source JSON sha mismatch: got {sha} expected {args.expect_source_sha256}",
                file=sys.stderr,
            )
            return 1
        if len(raw) != args.expect_source_bytes:
            print("source JSON size mismatch", file=sys.stderr)
            return 1
        data = json.loads(raw)
        source_format = "json"
        source_sha = sha
        source_bytes = len(raw)
    else:
        print("error: provide --source-yaml and/or --input", file=sys.stderr)
        return 2

    inventory = build_inventory(
        data,
        fetched_at_utc=args.fetched_at_utc,
        source_sha256=source_sha,
        source_bytes=source_bytes,
        source_format=source_format,
        converted_json_sha256=converted_json_sha256,
    )
    text = canonical_json(inventory)

    if args.check:
        existing = args.output.read_text(encoding="utf-8")
        if existing != text:
            print("inventory differs from generator output", file=sys.stderr)
            return 1
        print("OK: inventory matches generator")
        return 0

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(text, encoding="utf-8")
    print(
        f"wrote {args.output} endpoints={inventory['baseline']['endpoint_count']} "
        f"sha256={inventory['baseline']['content_sha256']} "
        f"revision={SOURCE_REVISION}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
