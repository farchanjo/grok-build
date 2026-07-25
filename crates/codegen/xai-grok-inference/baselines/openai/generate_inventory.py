#!/usr/bin/env python3
"""Deterministic OpenAI OpenAPI → endpoint_inventory.json generator.

Source pin is the official openai/openai-openapi YAML (converted to JSON for
this tool). The generator never performs network I/O.

Invocation:

  python3 generate_inventory.py \\
      --input /path/to/openai-openapi.json \\
      --output endpoint_inventory.json \\
      --fetched-at-utc 2026-07-25T17:00:00Z \\
      --source-sha256 <yaml_sha256> \\
      --source-bytes <yaml_bytes> \\
      --source-format yaml

Notes:
  - Endpoint enumeration: every path + HTTP method (skip x-*, parameters).
  - Priority endpoints are Grok coding-agent relevant and MUST exist.
  - content_sha256/content_bytes in baseline describe the *source document*
    pin (YAML blob when --source-format yaml), not the compact inventory.
  - OpenAI paths in this OpenAPI pin omit the `/v1` prefix (base URL carries it).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

# Coding-agent priority endpoints (must exist in the source OpenAPI).
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

# Schemas we inventory for later conformance (may be missing → skipped).
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

FORMAT_VERSION = 1
PROVIDER = "openai"
# Official public sources (documentation / OpenAPI, not a live inference call).
SOURCE_URL = (
    "https://raw.githubusercontent.com/openai/openai-openapi/master/openapi.yaml"
)
DOCS_URL = "https://platform.openai.com/docs/api-reference"
REPO_URL = "https://github.com/openai/openai-openapi"
LICENSE_NOTE = (
    "OpenAPI description from openai/openai-openapi (MIT). Compact inventory is "
    "derived metadata only; full OpenAPI is not vendored."
)

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


def request_content_types(op: dict[str, Any]) -> list[str]:
    body = op.get("requestBody")
    if not isinstance(body, dict):
        return []
    content = body.get("content")
    if not isinstance(content, dict):
        return []
    return sorted(str(k) for k in content.keys())


def infer_transport(method: str, content_types: list[str], op: dict[str, Any]) -> str:
    """Conservative transport label from OpenAPI shape only."""
    joined = " ".join(content_types).lower()
    if "multipart/" in joined:
        return "http_multipart"
    # Streaming is typically negotiated via request body flags, not path alone.
    # Mark unknown unless the summary/description clearly says SSE-only.
    summary = (op.get("summary") or "") + " " + (op.get("description") or "")
    if "server-sent" in summary.lower() or "sse" in summary.lower():
        return "http_sse"
    if method.upper() in {"GET", "DELETE", "HEAD", "OPTIONS"}:
        return "http_json"
    if content_types:
        return "http_json"
    return "unknown"


def build_inventory(
    data: dict[str, Any],
    *,
    fetched_at_utc: str,
    source_sha256: str,
    source_bytes: int,
    source_format: str,
) -> dict[str, Any]:
    info = data.get("info") or {}
    paths = data.get("paths") or {}
    schemas = (data.get("components") or {}).get("schemas") or {}
    if not isinstance(paths, dict) or not isinstance(schemas, dict):
        raise SystemExit("invalid OpenAPI: paths/schemas must be objects")

    endpoints: list[dict[str, Any]] = []
    for path in sorted(paths.keys()):
        if not isinstance(path, str) or not path.startswith("/"):
            raise SystemExit(f"unsafe or malformed path key: {path!r}")
        if ".." in path or path.startswith("//"):
            raise SystemExit(f"unsafe path key: {path!r}")
        methods = paths[path]
        if not isinstance(methods, dict):
            continue
        for method in sorted(methods.keys()):
            if method not in HTTP_METHODS:
                continue
            op = methods[method]
            if not isinstance(op, dict):
                continue
            cts = request_content_types(op)
            endpoints.append(
                {
                    "method": method.upper(),
                    "path": path,
                    "operation_id": op.get("operationId"),
                    "tags": list(op.get("tags") or []),
                    "summary": op.get("summary"),
                    "content_types": cts,
                    "transport": infer_transport(method, cts, op),
                }
            )

    endpoint_keys = {f"{e['method']} {e['path']}" for e in endpoints}
    missing = [p for p in PRIORITY_ENDPOINTS if p not in endpoint_keys]
    if missing:
        raise SystemExit(f"priority endpoints missing from source OpenAPI: {missing}")

    # Reject duplicate method+path identities.
    if len(endpoint_keys) != len(endpoints):
        raise SystemExit("duplicate method+path identities in OpenAPI paths")

    field_inventory = []
    for name in KEY_SCHEMAS:
        inv = schema_fields(schemas, name)
        if inv is not None:
            field_inventory.append(inv)

    inventory = {
        "format_version": FORMAT_VERSION,
        "provider": PROVIDER,
        "baseline": {
            "title": info.get("title"),
            "version": str(info.get("version")) if info.get("version") is not None else None,
            "openapi": data.get("openapi"),
            "source_url": SOURCE_URL,
            "docs_url": DOCS_URL,
            "repo_url": REPO_URL,
            "license_note": LICENSE_NOTE,
            "source_format": source_format,
            "fetched_at_utc": fetched_at_utc,
            "content_sha256": source_sha256,
            "content_bytes": source_bytes,
            "path_count": len(paths),
            "endpoint_count": len(endpoints),
            "schema_count": len(schemas),
        },
        "endpoints": endpoints,
        "coding_agent_schema_fields": field_inventory,
        "coding_agent_priority_endpoints": list(PRIORITY_ENDPOINTS),
        "notes": [
            "Compact inventory only; full OpenAPI is not vendored (~2.8 MiB YAML).",
            "OpenAI OpenAPI path templates omit the /v1 prefix (API base URL includes it).",
            "OpenRouter is NOT treated as a full OpenAI platform clone.",
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
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--fetched-at-utc", required=True)
    parser.add_argument(
        "--source-sha256",
        required=True,
        help="SHA-256 of the exact source blob being pinned (YAML preferred)",
    )
    parser.add_argument(
        "--source-bytes",
        required=True,
        type=int,
        help="Byte length of the exact source blob being pinned",
    )
    parser.add_argument(
        "--source-format",
        default="yaml",
        choices=["yaml", "json"],
        help="Format of the source blob described by --source-sha256",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)

    raw = args.input.read_bytes()
    # Input JSON is a conversion of the pin; integrity of the pin is the
    # explicit --source-sha256 of the original YAML (or JSON) blob.
    data = json.loads(raw)
    inventory = build_inventory(
        data,
        fetched_at_utc=args.fetched_at_utc,
        source_sha256=args.source_sha256,
        source_bytes=args.source_bytes,
        source_format=args.source_format,
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
        f"sha256={inventory['baseline']['content_sha256']} "
        f"bytes={inventory['baseline']['content_bytes']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
