#!/usr/bin/env python3
"""Deterministic OpenRouter OpenAPI → endpoint_inventory.json generator.

Invocation (exact):

  python3 generate_inventory.py \\
      --input /path/to/openrouter-openapi.json \\
      --output endpoint_inventory.json \\
      --fetched-at-utc 2026-07-24T22:27:00Z

Rules are pinned in this file:
  - endpoint enumeration: every path + HTTP method (skip x-*, parameters)
  - coding-agent schema allowlist (KEY_SCHEMAS)
  - coding-agent priority endpoints (PRIORITY_ENDPOINTS) — must exist in source
  - stable ordering (sorted paths/methods/fields)
  - canonical JSON (indent=2, trailing newline, sorted object keys where applicable)
  - content_sha256 + content_bytes of the exact input blob

No network I/O. Tests pass --input to a local file (full pin or mini fixture).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

# Pinned coding-agent schema allowlist (order preserved in output).
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

# Priority endpoints for Grok coding-agent coverage. Every entry MUST exist
# in the source OpenAPI or generation fails.
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

FORMAT_VERSION = 1
PROVIDER = "openrouter"
SOURCE_URL = "https://openrouter.ai/openapi.json"
YAML_URL = "https://openrouter.ai/openapi.yaml"
DOCS_URL = "https://openrouter.ai/docs/api_reference/overview"

HTTP_METHODS = {
    "get",
    "put",
    "post",
    "delete",
    "options",
    "head",
    "patch",
    "trace",
}


def schema_fields(schemas: dict[str, Any], name: str) -> dict[str, Any] | None:
    s = schemas.get(name)
    if not s or not isinstance(s, dict):
        return None
    props: dict[str, Any] = dict(s.get("properties") or {})
    required = set(s.get("required") or [])
    for part in s.get("allOf") or []:
        if isinstance(part, dict) and "properties" in part:
            props = {**props, **part["properties"]}
            required |= set(part.get("required") or [])
    fields: list[dict[str, Any]] = []
    for fname in sorted(props.keys()):
        fdef = props[fname]
        entry: dict[str, Any] = {"name": fname, "required": fname in required}
        if isinstance(fdef, dict):
            if "type" in fdef:
                entry["type"] = fdef["type"]
            if "$ref" in fdef and isinstance(fdef["$ref"], str):
                entry["ref"] = fdef["$ref"].rsplit("/", 1)[-1]
            if "anyOf" in fdef or "oneOf" in fdef:
                entry["union"] = True
            if "enum" in fdef:
                entry["enum"] = fdef["enum"]
        fields.append(entry)
    return {"schema": name, "field_count": len(fields), "fields": fields}


def build_inventory(raw: bytes, fetched_at_utc: str) -> dict[str, Any]:
    data = json.loads(raw)
    info = data.get("info") or {}
    paths = data.get("paths") or {}
    schemas = (data.get("components") or {}).get("schemas") or {}
    if not isinstance(paths, dict) or not isinstance(schemas, dict):
        raise SystemExit("invalid OpenAPI: paths/schemas must be objects")

    endpoints: list[dict[str, Any]] = []
    for path in sorted(paths.keys()):
        methods = paths[path]
        if not isinstance(methods, dict):
            continue
        for method in sorted(methods.keys()):
            if method not in HTTP_METHODS:
                continue
            op = methods[method]
            if not isinstance(op, dict):
                continue
            endpoints.append(
                {
                    "method": method.upper(),
                    "path": path,
                    "operation_id": op.get("operationId"),
                    "tags": list(op.get("tags") or []),
                    "summary": op.get("summary"),
                }
            )

    endpoint_keys = {f"{e['method']} {e['path']}" for e in endpoints}
    missing = [p for p in PRIORITY_ENDPOINTS if p not in endpoint_keys]
    if missing:
        raise SystemExit(f"priority endpoints missing from source OpenAPI: {missing}")

    field_inventory = []
    for name in KEY_SCHEMAS:
        inv = schema_fields(schemas, name)
        if inv is not None:
            field_inventory.append(inv)

    sha = hashlib.sha256(raw).hexdigest()
    inventory = {
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
            "path_count": len(paths),
            "endpoint_count": len(endpoints),
            "schema_count": len(schemas),
        },
        "endpoints": endpoints,
        "coding_agent_schema_fields": field_inventory,
        "coding_agent_priority_endpoints": list(PRIORITY_ENDPOINTS),
        "notes": [
            "Compact inventory only; full OpenAPI is not vendored.",
            "Regenerate with generate_inventory.py from a local OpenAPI file.",
            "No network I/O in the generator or in unit tests.",
        ],
    }
    return inventory


def canonical_json(obj: Any) -> str:
    return json.dumps(obj, indent=2, ensure_ascii=False) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path, help="Local OpenAPI JSON path")
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
        help="Destination endpoint_inventory.json path",
    )
    parser.add_argument(
        "--fetched-at-utc",
        required=True,
        help="ISO-8601 UTC timestamp recorded in baseline.fetched_at_utc",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Compare generated inventory to --output and exit 1 on diff",
    )
    args = parser.parse_args(argv)

    raw = args.input.read_bytes()
    inventory = build_inventory(raw, args.fetched_at_utc)
    text = canonical_json(inventory)

    if args.check:
        existing = args.output.read_text(encoding="utf-8")
        if existing != text:
            print(
                "inventory differs from generator output; re-run without --check to refresh",
                file=sys.stderr,
            )
            return 1
        print("OK: inventory matches generator for", args.input)
        return 0

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(text, encoding="utf-8")
    print(
        f"wrote {args.output} endpoints={inventory['baseline']['endpoint_count']} "
        f"sha256={inventory['baseline']['content_sha256']} "
        f"bytes={inventory['baseline']['content_bytes']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
