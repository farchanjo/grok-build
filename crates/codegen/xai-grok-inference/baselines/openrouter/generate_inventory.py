#!/usr/bin/env python3
"""Deterministic OpenRouter OpenAPI → endpoint_inventory.json generator.

Verifies the exact input blob SHA/size before writing. No network I/O.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "_shared"))
from openapi_contract import enumerate_endpoints, schema_fields  # noqa: E402

FORMAT_VERSION = 2
PROVIDER = "openrouter"
SOURCE_URL = "https://openrouter.ai/openapi.json"
YAML_URL = "https://openrouter.ai/openapi.yaml"
DOCS_URL = "https://openrouter.ai/docs/api_reference/overview"

KEY_SCHEMAS: list[str] = [
    "ChatRequest",
    "ResponsesRequest",
    "ProviderPreferences",
    "ChatUsage",
    "ChatToolCall",
    "ChatStreamToolCall",
    "ChatReasoningDetails",
    "ChatStreamReasoningDetails",
    "ReasoningDetailText",
    "ReasoningDetailSummary",
    "ReasoningDetailEncrypted",
    "OpenAIResponsesUsage",
    "ModelsListResponse",
    "GenerationResponse",
]

PRIORITY_ENDPOINTS: list[str] = [
    "POST /chat/completions",
    "POST /responses",
    "POST /messages",
    "GET /models",
    "GET /key",
    "GET /credits",
    "GET /generation",
    "POST /embeddings",
]


def build_inventory(
    raw: bytes,
    fetched_at_utc: str,
    *,
    expect_sha256: str | None = None,
    expect_bytes: int | None = None,
) -> dict[str, Any]:
    sha = hashlib.sha256(raw).hexdigest()
    if expect_sha256 is not None and sha != expect_sha256:
        raise SystemExit(f"input sha mismatch: got {sha} expected {expect_sha256}")
    if expect_bytes is not None and len(raw) != expect_bytes:
        raise SystemExit(
            f"input size mismatch: got {len(raw)} expected {expect_bytes}"
        )

    data = json.loads(raw)
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

    return {
        "format_version": FORMAT_VERSION,
        "provider": PROVIDER,
        "baseline": {
            "title": info.get("title"),
            "version": info.get("version"),
            "openapi": data.get("openapi"),
            "source_url": SOURCE_URL,
            "yaml_url": YAML_URL,
            "docs_url": DOCS_URL,
            "fetched_at_utc": fetched_at_utc,
            "content_sha256": sha,
            "content_bytes": len(raw),
            "path_count": len(paths) if isinstance(paths, dict) else 0,
            "endpoint_count": len(endpoints),
            "schema_count": len(schemas),
        },
        "endpoints": endpoints,
        "coding_agent_schema_fields": field_inventory,
        "coding_agent_priority_endpoints": list(PRIORITY_ENDPOINTS),
        "notes": [
            "Compact inventory only; full OpenAPI is not vendored.",
            "transports is a multi-label set; stream-flag ops include http_json + http_sse.",
            "Binary response ops are never collapsed to sole http_json.",
            "OpenRouter is not a full OpenAI platform clone.",
            "Regenerate with generate_inventory.py from a local OpenAPI file.",
            "No network I/O in the generator or in unit tests.",
        ],
    }


def canonical_json(obj: Any) -> str:
    return json.dumps(obj, indent=2, ensure_ascii=False) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--fetched-at-utc", required=True)
    parser.add_argument(
        "--expect-source-sha256",
        help="When set, require the input blob SHA-256 to match exactly",
    )
    parser.add_argument(
        "--expect-source-bytes",
        type=int,
        help="When set, require the input blob size to match exactly",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)

    raw = args.input.read_bytes()
    inventory = build_inventory(
        raw,
        args.fetched_at_utc,
        expect_sha256=args.expect_source_sha256,
        expect_bytes=args.expect_source_bytes,
    )
    text = canonical_json(inventory)

    if args.check:
        existing = args.output.read_text(encoding="utf-8")
        if existing != text:
            print("inventory differs from generator output", file=sys.stderr)
            return 1
        print("OK: inventory matches generator for", args.input)
        return 0

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(text, encoding="utf-8")
    print(
        f"wrote {args.output} endpoints={inventory['baseline']['endpoint_count']} "
        f"sha256={inventory['baseline']['content_sha256']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
