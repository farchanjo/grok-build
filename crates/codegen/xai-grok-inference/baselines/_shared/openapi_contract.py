#!/usr/bin/env python3
"""Shared OpenAPI → compact endpoint contract helpers.

Used by OpenAI and OpenRouter inventory generators. No network I/O.
"""

from __future__ import annotations

import re
from typing import Any

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

# Allowed transport labels in inventories (strict).
TRANSPORT_LABELS = frozenset(
    {
        "http_json",
        "http_sse",
        "http_multipart",
        "http_binary",
        "websocket",
        "unknown",
    }
)

# RFC6838-ish media type: type/subtype[;param=value]*
# Subtype may be `*` (e.g. audio/*) as used in some OpenAPI documents.
MEDIA_TYPE_RE = re.compile(
    r"^[a-zA-Z0-9!#$&\-\^_+.]+/(?:\*|[a-zA-Z0-9!#$&\-\^_+.]+)"
    r"(?:\s*;\s*[a-zA-Z0-9!#$&\-\^_+.]+=[\w\-\.+/*'\"%]+)*$"
)

BINARY_MEDIA_PREFIXES = (
    "audio/",
    "image/",
    "video/",
    "application/octet-stream",
    "application/pdf",
    "application/zip",
    "application/gzip",
)


def is_safe_path(path: str) -> bool:
    if not path or not path.startswith("/"):
        return False
    if path.startswith("//") or ".." in path or "://" in path:
        return False
    if "\0" in path or "\n" in path or "\r" in path:
        return False
    return True


def validate_media_type(mt: str) -> bool:
    s = mt.strip()
    if not s or len(s) > 256:
        return False
    if s == "*/*":
        return True
    return MEDIA_TYPE_RE.match(s) is not None


def is_binary_media(mt: str) -> bool:
    low = mt.lower().split(";", 1)[0].strip()
    return any(low.startswith(p) or low == p.rstrip("/") for p in BINARY_MEDIA_PREFIXES)


def collect_request_content_types(op: dict[str, Any]) -> list[str]:
    body = op.get("requestBody")
    if not isinstance(body, dict):
        return []
    content = body.get("content")
    if not isinstance(content, dict):
        return []
    out: list[str] = []
    for k in sorted(content.keys()):
        if not isinstance(k, str):
            raise SystemExit(f"non-string content type key: {k!r}")
        if not validate_media_type(k):
            raise SystemExit(f"malformed request content type: {k!r}")
        out.append(k)
    return out


def collect_response_content_types(op: dict[str, Any]) -> list[str]:
    responses = op.get("responses")
    if not isinstance(responses, dict):
        return []
    found: set[str] = set()
    for _code, resp in responses.items():
        if not isinstance(resp, dict):
            continue
        content = resp.get("content")
        if not isinstance(content, dict):
            continue
        for k in content.keys():
            if not isinstance(k, str):
                raise SystemExit(f"non-string response content type: {k!r}")
            if not validate_media_type(k):
                raise SystemExit(f"malformed response content type: {k!r}")
            found.add(k)
    return sorted(found)


def schema_has_stream_flag(schemas: dict[str, Any], schema_name: str | None) -> bool:
    if not schema_name or not isinstance(schemas, dict):
        return False
    s = schemas.get(schema_name)
    if not isinstance(s, dict):
        return False
    props = dict(s.get("properties") or {})
    for part in s.get("allOf") or []:
        if isinstance(part, dict) and isinstance(part.get("properties"), dict):
            props = {**props, **part["properties"]}
    return "stream" in props


def request_body_schema_name(op: dict[str, Any]) -> str | None:
    body = op.get("requestBody")
    if not isinstance(body, dict):
        return None
    content = body.get("content")
    if not isinstance(content, dict):
        return None
    for _ct, media in content.items():
        if not isinstance(media, dict):
            continue
        schema = media.get("schema")
        if not isinstance(schema, dict):
            continue
        ref = schema.get("$ref")
        if isinstance(ref, str) and ref.startswith("#/components/schemas/"):
            return ref.rsplit("/", 1)[-1]
    return None


def infer_transports(
    method: str,
    request_cts: list[str],
    response_cts: list[str],
    op: dict[str, Any],
    schemas: dict[str, Any],
) -> list[str]:
    """Return sorted unique transport labels. Prefer multi-label sets over collapse."""
    transports: set[str] = set()
    text = f"{op.get('summary') or ''} {op.get('description') or ''}".lower()
    path = ""  # caller may not pass path; description-based only

    req_has_json = any("json" in c.lower() for c in request_cts)
    req_has_multipart = any("multipart/" in c.lower() for c in request_cts)
    req_has_form = any(
        "application/x-www-form-urlencoded" in c.lower() for c in request_cts
    )
    resp_has_sse = any(
        "text/event-stream" in c.lower() or c.lower().startswith("text/event-stream")
        for c in response_cts
    )
    resp_has_json = any("json" in c.lower() for c in response_cts)
    resp_has_binary = any(is_binary_media(c) for c in response_cts)

    if req_has_multipart or req_has_form:
        transports.add("http_multipart")
    if resp_has_sse or "server-sent" in text or " text/event-stream" in text:
        transports.add("http_sse")
    if "websocket" in text or " web socket" in text:
        transports.add("websocket")

    # Stream-flag request schemas (Chat Completions / Responses style).
    schema_name = request_body_schema_name(op)
    if schema_has_stream_flag(schemas, schema_name):
        transports.add("http_json")
        transports.add("http_sse")

    if resp_has_binary:
        transports.add("http_binary")
        # Binary download with JSON request body: keep request transport separate.
        if req_has_json:
            transports.add("http_json")

    # Pure JSON request/response without binary.
    if req_has_json and not resp_has_binary:
        transports.add("http_json")
    if not request_cts and method.upper() in {"GET", "DELETE", "HEAD", "OPTIONS"}:
        if resp_has_binary:
            transports.add("http_binary")
        elif resp_has_json or not response_cts:
            transports.add("http_json")

    # Response-only JSON (e.g. GET list).
    if resp_has_json and not resp_has_binary and method.upper() == "GET":
        transports.add("http_json")

    if not transports:
        transports.add("unknown")

    # Never claim only http_json when binary responses exist.
    if resp_has_binary and transports == {"http_json"}:
        transports = {"http_binary", "http_json"}

    ordered = sorted(transports)
    for t in ordered:
        if t not in TRANSPORT_LABELS:
            raise SystemExit(f"invalid transport label produced: {t}")
    return ordered


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


def enumerate_endpoints(
    data: dict[str, Any],
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    paths = data.get("paths") or {}
    schemas = (data.get("components") or {}).get("schemas") or {}
    if not isinstance(paths, dict) or not isinstance(schemas, dict):
        raise SystemExit("invalid OpenAPI: paths/schemas must be objects")

    endpoints: list[dict[str, Any]] = []
    for path in sorted(paths.keys()):
        if not isinstance(path, str) or not is_safe_path(path):
            raise SystemExit(f"unsafe or malformed path key: {path!r}")
        methods = paths[path]
        if not isinstance(methods, dict):
            continue
        for method in sorted(methods.keys()):
            if method not in HTTP_METHODS:
                continue
            op = methods[method]
            if not isinstance(op, dict):
                continue
            req_cts = collect_request_content_types(op)
            resp_cts = collect_response_content_types(op)
            transports = infer_transports(method, req_cts, resp_cts, op, schemas)
            endpoints.append(
                {
                    "method": method.upper(),
                    "path": path,
                    "operation_id": op.get("operationId"),
                    "tags": list(op.get("tags") or []),
                    "summary": op.get("summary"),
                    "request_content_types": req_cts,
                    "response_content_types": resp_cts,
                    "transports": transports,
                }
            )
    return endpoints, schemas
