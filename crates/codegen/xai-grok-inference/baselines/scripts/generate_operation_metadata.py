#!/usr/bin/env python3
"""Unify OpenAI/OpenRouter operation metadata from inventories + actual ops.

Authoritative inputs (never circular):
  - baselines/{openai,openrouter}/endpoint_inventory.json
  - src/openai_platform/generated/*_ops.rs (parsed method specs)

Derived caches (regenerated, never used as inputs):
  - baselines/operation_table.json
  - baselines/operation_bindings_report.json
  - src/openai_platform/generated/bindings.rs
  - xai-grok-shell/src/cli/generated_ops.rs
  - xai-grok-shell/src/cli/typed_dispatch_runtime.rs (dispatch arms only)

Also repairs known transport gaps in generated ops/types so inventory
transports (skill zip binary, SSE companions) match actual client methods.

No network I/O. No credentials.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, asdict, field
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[5]
INFERENCE = REPO / "crates/codegen/xai-grok-inference"
SHELL_CLI = REPO / "crates/codegen/xai-grok-shell/src/cli"
GEN = INFERENCE / "src/openai_platform/generated"
BASELINES = INFERENCE / "baselines"
RUSTFMT_TOML = REPO / "rustfmt.toml"

OPENAI_INV = BASELINES / "openai/endpoint_inventory.json"
OPENROUTER_INV = BASELINES / "openrouter/endpoint_inventory.json"

OPS_FILES = {
    "openai": GEN / "openai_ops.rs",
    "openai_admin": GEN / "openai_admin_ops.rs",
    "openrouter": GEN / "openrouter_ops.rs",
}
TYPES_FILES = {
    "openai": GEN / "openai_types.rs",
    "openai_admin": GEN / "openai_admin_types.rs",
    "openrouter": GEN / "openrouter_types.rs",
}
CLIENT_TYPES = {
    "openai": "OpenAiClient",
    "openai_admin": "OpenAiAdminClient",
    "openrouter": "OpenRouterClient",
}
TYPES_MODS = {
    "openai": "openai_types",
    "openai_admin": "openai_admin_types",
    "openrouter": "openrouter_types",
}

# OpenRouter paths that require the management/admin credential. GET /key is
# application-scoped (current key metadata); organization/keys/BYOK/workspaces/
# guardrails/observability management surfaces are admin.
OPENROUTER_ADMIN_PREFIXES = (
    "/keys",
    "/auth/keys",
    "/byok",
    "/workspaces",
    "/guardrails",
    "/observability",
    "/organization",
)

# Inference-style POSTs that intentionally do not require --yes. Everything
# else that is not GET/HEAD fails closed when metadata is missing or mutating.
SAFE_NO_CONFIRM_OPERATION_IDS = frozenset(
    {
        # OpenAI inference / media generation
        "createChatCompletion",
        "createChatCompletion_stream",
        "createCompletion",
        "createCompletion_stream",
        "createResponse",
        "createResponse_stream",
        "beta_createResponse",
        "beta_createResponse_stream",
        "createEmbedding",
        "createSpeech",
        "createSpeech_stream",
        "createTranscription",
        "createTranscription_stream",
        "createTranslation",
        "createImage",
        "createImage_stream",
        "createImageEdit",
        "createImageEdit_stream",
        "createImageVariation",
        "createVideo",
        "createModeration",
        "createThreadAndRun",
        "createThreadAndRun_stream",
        "createRun",
        "createRun_stream",
        "submitToolOuputsToRun",
        "submitToolOuputsToRun_stream",
        # OpenRouter inference
        "sendChatCompletionRequest",
        "sendChatCompletionRequest_stream",
        "createEmbeddings",
        "createEmbeddings_stream",
        "createImages",
        "createImages_stream",
        "createMessages",
        "createMessages_stream",
        "createResponses",
        "createResponses_stream",
        "createRerank",
        "createRerank_stream",
        "createAudioSpeech",
        "createVideos",
        "createPresetsChatCompletions",
        "createPresetsChatCompletions_stream",
        "createPresetsMessages",
        "createPresetsMessages_stream",
        "createPresetsResponses",
        "createPresetsResponses_stream",
    }
)


@dataclass
class InventoryEndpoint:
    provider: str  # openai | openrouter (inventory provider)
    method: str
    path: str
    operation_id: str
    transports: list[str]
    is_deprecated: bool
    summary: str = ""


@dataclass
class ParsedOp:
    namespace: str
    client_method: str
    operation_id: str  # from doc / HttpRequestSpec
    method: str
    path: str
    mode: str  # json | sse | binary | multipart | websocket
    request_type: str
    response_type: str
    credential: str  # Application | Admin
    transports_comment: list[str]
    has_files: bool
    has_sink: bool
    body: str  # full method source including signature


@dataclass
class MetaRow:
    provider: str
    operation_id: str
    method: str
    path: str
    client_type: str
    client_method: str
    request_type: str
    response_type: str
    body_type: str | None
    cli_route: str
    transports: list[str]
    is_admin: bool
    is_deprecated: bool
    is_multipart: bool
    is_sse: bool
    is_binary: bool
    is_websocket: bool
    is_primary: bool
    typed_request: bool
    typed_response: bool
    generic_value_body: bool
    requires_confirmation: bool
    credential_class: str  # application | admin
    mode: str


def load_inventory(path: Path, provider: str) -> list[InventoryEndpoint]:
    data = json.loads(path.read_text())
    out: list[InventoryEndpoint] = []
    for ep in data["endpoints"]:
        op_id = ep.get("operation_id") or f'{ep["method"]}_{ep["path"]}'
        transports = list(ep.get("transports") or ["http_json"])
        is_deprecated = "assistants" in ep["path"] or "threads" in ep["path"]
        out.append(
            InventoryEndpoint(
                provider=provider,
                method=ep["method"].upper(),
                path=ep["path"],
                operation_id=op_id,
                transports=transports,
                is_deprecated=is_deprecated,
                summary=ep.get("summary") or "",
            )
        )
    return out


def is_openai_admin_path(path: str) -> bool:
    return path.startswith("/organization") or path.startswith("/dashboard")


def is_openrouter_admin_path(path: str) -> bool:
    if path == "/key" or path.startswith("/key?"):
        return False
    for prefix in OPENROUTER_ADMIN_PREFIXES:
        if path == prefix or path.startswith(prefix + "/"):
            return True
    return False


def namespace_for(provider: str, path: str) -> str:
    if provider == "openrouter":
        return "openrouter"
    if is_openai_admin_path(path):
        return "openai_admin"
    return "openai"


def camel_to_snake(name: str) -> str:
    if not name:
        return "op"
    s1 = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)
    s2 = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", s1)
    s3 = re.sub(r"[^a-zA-Z0-9_]+", "_", s2)
    s3 = re.sub(r"_+", "_", s3).strip("_").lower()
    if not s3:
        s3 = "op"
    if s3[0].isdigit():
        s3 = f"op_{s3}"
    keywords = {
        "type",
        "match",
        "move",
        "ref",
        "self",
        "super",
        "crate",
        "async",
        "await",
        "dyn",
        "where",
        "as",
        "break",
        "const",
        "continue",
        "else",
        "enum",
        "extern",
        "false",
        "fn",
        "for",
        "if",
        "impl",
        "in",
        "let",
        "loop",
        "mod",
        "mut",
        "pub",
        "return",
        "static",
        "struct",
        "trait",
        "true",
        "unsafe",
        "use",
        "while",
        "try",
        "gen",
        "box",
    }
    if s3 in keywords:
        s3 = f"{s3}_"
    return s3


def pascal(name: str) -> str:
    parts = re.sub(r"[^a-zA-Z0-9]+", "_", name).split("_")
    base = "".join(p[:1].upper() + p[1:] for p in parts if p)
    if not base:
        base = "Type"
    if base[0].isdigit():
        base = f"T{base}"
    return base


METHOD_RE = re.compile(
    r"(?ms)^    /// `([A-Z]+) ([^`]+)` — `([^`]+)` \(([^)]+)\)\.\n"
    r"    /// Transports: ([^\n]+)\.\n"
    r"    pub async fn ([a-zA-Z0-9_]+)\(\n?"
    r"\s*&self,?\n?"
    r"(.*?)"
    r"\) -> PlatformResult<([A-Za-z0-9_]+)>\s*\{"
    r"(.*?)^    \}\n",
)


def parse_ops_file(namespace: str, path: Path) -> list[ParsedOp]:
    text = path.read_text()
    ops: list[ParsedOp] = []
    for m in METHOD_RE.finditer(text):
        method = m.group(1)
        op_path = m.group(2)
        op_id_doc = m.group(3)
        mode = m.group(4)
        transports_s = m.group(5)
        fn_name = m.group(6)
        resp_ty = m.group(8)
        full = m.group(0)
        # Prefer operation_id from HttpRequestSpec when present.
        spec_id = re.search(r'operation_id:\s*"([^"]+)"', full)
        operation_id = spec_id.group(1) if spec_id else op_id_doc
        # Stream companions keep the _stream suffix in catalog ids.
        if mode == "sse" and fn_name.endswith("_stream"):
            base_id = (
                operation_id[: -len("_stream")]
                if operation_id.endswith("_stream")
                else operation_id
            )
            catalog_id = base_id + "_stream"
        elif fn_name.endswith("_stream") and not operation_id.endswith("_stream"):
            catalog_id = operation_id + "_stream"
        else:
            catalog_id = operation_id

        req_m = re.search(r"request:\s*([A-Za-z0-9_]+)", full)
        request_type = req_m.group(1) if req_m else "()"
        cred_m = re.search(r"credential:\s*CredentialKind::(Application|Admin)", full)
        credential = cred_m.group(1) if cred_m else "Application"
        transports = [t.strip() for t in transports_s.split(",") if t.strip()]
        has_files = "files: MultipartFiles" in full
        has_sink = "sink: Option<&std::path::Path>" in full
        ops.append(
            ParsedOp(
                namespace=namespace,
                client_method=fn_name,
                operation_id=catalog_id,
                method=method,
                path=op_path,
                mode=mode,
                request_type=request_type,
                response_type=resp_ty,
                credential=credential,
                transports_comment=transports,
                has_files=has_files,
                has_sink=has_sink,
                body=full,
            )
        )
    return ops


def requires_confirmation(method: str, operation_id: str) -> bool:
    if method in ("GET", "HEAD", "OPTIONS"):
        return False
    if operation_id in SAFE_NO_CONFIRM_OPERATION_IDS:
        return False
    # Fail closed for mutations and unknown ids.
    return True


def primary_mode_from_transports(transports: list[str]) -> str:
    t = set(transports)
    if "http_multipart" in t:
        return "multipart"
    if "http_binary" in t and "http_json" not in t and "http_sse" not in t:
        return "binary"
    if "http_binary" in t and "http_json" not in t:
        return "binary"
    if "http_binary" in t and "http_sse" not in t and "http_json" in t:
        # Prefer binary when both binary and json are present (downloads).
        # createSpeech is special: primary is binary with sse companion.
        return "binary"
    if "websocket" in t and "http_json" not in t:
        return "websocket"
    if "http_sse" in t and "http_json" not in t and "http_binary" not in t:
        return "sse"
    return "json"


def ensure_sse_type(types_path: Path, type_name: str, op_id: str) -> None:
    text = types_path.read_text()
    if f"pub struct {type_name}" in text:
        return
    block = f'''
/// SSE event stream for `{op_id}` (all frames preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct {type_name} {{
    pub events: Vec<SseEvent>,
}}
'''
    types_path.write_text(text.rstrip() + "\n" + block + "\n")


def ensure_binary_type(types_path: Path, type_name: str, op_id: str) -> None:
    text = types_path.read_text()
    # Replace JSON-shaped result with binary bytes when present as json wrapper.
    bin_struct = f'''/// Binary result for `{op_id}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct {type_name} {{
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}}
'''
    pattern = re.compile(
        rf"///[^\n]*\n#\[derive\([^\]]*\)\]\npub struct {type_name} \{{.*?\n\}}\n",
        re.S,
    )
    if pattern.search(text):
        text = pattern.sub(bin_struct + "\n", text, count=1)
        types_path.write_text(text)
        return
    if f"pub struct {type_name}" not in text:
        types_path.write_text(text.rstrip() + "\n" + bin_struct + "\n")


def rewrite_method_to_binary(body: str, op_id: str, resp_ty: str) -> str:
    # Convert execute_json method to execute_binary with sink.
    body = re.sub(
        r"pub async fn ([a-zA-Z0-9_]+)\(&self,\n?(\s*)request: ([A-Za-z0-9_]+),\n?(\s*)\) -> PlatformResult<",
        r"pub async fn \1(&self,\n\2request: \3,\n\2sink: Option<&std::path::Path>,\n\2) -> PlatformResult<",
        body,
        count=1,
    )
    # If already has sink, leave signature alone.
    if "sink: Option<&std::path::Path>" not in body:
        body = re.sub(
            r"(pub async fn [a-zA-Z0-9_]+\(&self, request: [A-Za-z0-9_]+),",
            r"\1, sink: Option<&std::path::Path>,",
            body,
            count=1,
        )
        body = re.sub(
            r"(pub async fn [a-zA-Z0-9_]+\(&self,\n\s*request: [A-Za-z0-9_]+,)\n(\s*)\)",
            r"\1\n\2sink: Option<&std::path::Path>,\n\2)",
            body,
            count=1,
        )
    body = re.sub(r"\((json|sse|multipart|websocket)\)\.", "(binary).", body, count=1)
    # Fix transports line to include http_binary.
    def fix_transports(m: re.Match[str]) -> str:
        parts = [p.strip() for p in m.group(1).split(",")]
        if "http_binary" not in parts:
            parts = sorted(set(parts + ["http_binary"]))
        # Prefer inventory-style binary primary.
        return f"    /// Transports: {', '.join(parts)}."

    body = re.sub(r"    /// Transports: ([^\n]+)\.", fix_transports, body, count=1)
    body = re.sub(
        r"expect_binary: false,",
        "expect_binary: true,",
        body,
    )
    body = re.sub(
        r"expect_sse: true,",
        "expect_sse: false,",
        body,
    )
    body = re.sub(
        r"let raw = self\.transport\.execute_json\(spec\)\.await\?;\n\s*serde_json::from_value\(raw\)\.map_err\(\|e\| PlatformError::Decode\(e\.to_string\(\)\)\)",
        f"let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;\n        Ok({resp_ty} {{ bytes, content_type }})",
        body,
    )
    body = re.sub(
        r"let events = self\.transport\.execute_sse\(spec\)\.await\?;\n\s*Ok\(\w+ \{ events \}\)",
        f"let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;\n        Ok({resp_ty} {{ bytes, content_type }})",
        body,
    )
    return body


def make_sse_companion(primary: ParsedOp, inventory_transports: list[str]) -> str:
    fn = primary.client_method if primary.client_method.endswith("_stream") else primary.client_method + "_stream"
    if primary.client_method.endswith("_stream"):
        fn = primary.client_method
    else:
        fn = primary.client_method + "_stream"
    base_op = primary.operation_id.replace("_stream", "")
    resp_ty = pascal(base_op) + "SseResult"
    if primary.namespace == "openrouter" and not resp_ty.startswith("Or"):
        # OpenRouter types are not always Or-prefixed for Params/Result.
        resp_ty = pascal(base_op) + "SseResult"
    transports = sorted(set(inventory_transports) | {"http_sse"})
    # Clone primary body structure into sse form.
    path = primary.path
    method = primary.method
    req_ty = primary.request_type
    # Build a standard SSE method from primary fields by templating.
    # Extract path/query/body construction from primary when possible.
    path_line = re.search(r'let (?:mut )?path = String::from\("([^"]+)"\);', primary.body)
    path_lit = path_line.group(1) if path_line else path
    path_repls = re.findall(
        r'path = path\.replace\(\s*"\{([^}]+)\}",\s*&crate::openai_platform::url_policy::encode_path_segment\(&request\.([a-zA-Z0-9_]+)\),\s*\);',
        primary.body,
    )
    query_inserts = re.findall(
        r'(if let Some\(v\) = request\.([a-zA-Z0-9_]+)\.as_ref\(\) \{\s*query\.insert\("([^"]+)"\.into\(\), query_value\(v\)\);\s*\}|query\.insert\("([^"]+)"\.into\(\), query_value\(&request\.([a-zA-Z0-9_]+)\)\);)',
        primary.body,
        re.S,
    )
    has_body = "let body = Some(" in primary.body
    lines = [
        f"    /// `{method} {path}` — `{base_op}` (sse).",
        f"    /// Transports: {', '.join(transports)}.",
        f"    pub async fn {fn}(",
        "        &self,",
        f"        request: {req_ty},",
        f"    ) -> PlatformResult<{resp_ty}> {{",
    ]
    if path_repls:
        lines.append(f'        let mut path = String::from("{path_lit}");')
        for brace, field in path_repls:
            lines.append(
                f'        path = path.replace(\n            "{{{brace}}}",\n            &crate::openai_platform::url_policy::encode_path_segment(&request.{field}),\n        );'
            )
    else:
        lines.append(f'        let path = String::from("{path_lit}");')
    # Query
    if "let mut query" in primary.body:
        lines.append("        let mut query: BTreeMap<String, String> = BTreeMap::new();")
        # Re-extract optional and required query lines simply
        for qm in re.finditer(
            r"if let Some\(v\) = request\.([a-zA-Z0-9_]+)\.as_ref\(\) \{\n\s*query\.insert\(\"([^\"]+)\"\.into\(\), query_value\(v\)\);\n\s*\}",
            primary.body,
        ):
            lines.append(
                f'        if let Some(v) = request.{qm.group(1)}.as_ref() {{\n            query.insert("{qm.group(2)}".into(), query_value(v));\n        }}'
            )
        for qm in re.finditer(
            r'query\.insert\("([^"]+)"\.into\(\), query_value\(&request\.([a-zA-Z0-9_]+)\)\);',
            primary.body,
        ):
            # only required inserts not inside if-let already covered loosely
            if f"request.{qm.group(2)}.as_ref()" not in primary.body:
                lines.append(
                    f'        query.insert("{qm.group(1)}".into(), query_value(&request.{qm.group(2)}));'
                )
    else:
        lines.append("        let query: BTreeMap<String, String> = BTreeMap::new();")
    if has_body:
        lines.append(
            "        let body = Some(\n            serde_json::to_value(&request.body)\n                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,\n        );"
        )
    else:
        lines.append("        let body: Option<serde_json::Value> = None;")
    cred = primary.credential
    # For openrouter admin paths force Admin.
    if primary.namespace == "openrouter" and is_openrouter_admin_path(path):
        cred = "Admin"
    lines += [
        "        let spec = HttpRequestSpec {",
        f'            method: "{method}",',
        "            path,",
        "            query,",
        "            body,",
        f"            credential: CredentialKind::{cred},",
        "            expect_sse: true,",
        "            expect_binary: false,",
        "            multipart: false,",
        f'            operation_id: "{base_op}",',
        f"            idempotent: {str(method in ('GET', 'HEAD')).lower()},",
        "        };",
        "        let events = self.transport.execute_sse(spec).await?;",
        f"        Ok({resp_ty} {{ events }})",
        "    }",
        "",
    ]
    return "\n".join(lines)


def repair_ops_and_types(
    inventories: dict[str, list[InventoryEndpoint]],
    parsed: dict[str, list[ParsedOp]],
) -> None:
    """Patch generated ops/types for inventory transport fidelity."""
    # Index inventory by (method, path) and operation_id.
    inv_by_key: dict[tuple[str, str, str], InventoryEndpoint] = {}
    for prov, eps in inventories.items():
        for ep in eps:
            inv_by_key[(prov, ep.method, ep.path)] = ep

    # --- OpenAI skill zip binary ---
    openai_ops_path = OPS_FILES["openai"]
    openai_types_path = TYPES_FILES["openai"]
    text = openai_ops_path.read_text()
    by_fn = {op.client_method: op for op in parsed["openai"]}
    for fn, op_id in (
        ("get_skill_content", "GetSkillContent"),
        ("get_skill_version_content", "GetSkillVersionContent"),
    ):
        if fn not in by_fn:
            continue
        op = by_fn[fn]
        if op.mode == "binary":
            continue
        resp_ty = op.response_type
        ensure_binary_type(openai_types_path, resp_ty, op_id)
        new_body = rewrite_method_to_binary(op.body, op_id, resp_ty)
        # Also force transports comment to inventory.
        inv = inv_by_key.get(("openai", op.method, op.path))
        if inv:
            new_body = re.sub(
                r"    /// Transports: [^\n]+\.",
                f"    /// Transports: {', '.join(inv.transports)}.",
                new_body,
                count=1,
            )
        text = text.replace(op.body, new_body)
    openai_ops_path.write_text(text)

    # Re-parse openai after binary fix.
    parsed["openai"] = parse_ops_file("openai", openai_ops_path)

    # --- Missing SSE companions for inventory http_sse primaries ---
    for prov, inv_provider in (("openai", "openai"), ("openrouter", "openrouter")):
        ops_path = OPS_FILES[prov]
        types_path = TYPES_FILES[prov]
        ops_list = parsed[prov]
        by_catalog = {o.operation_id: o for o in ops_list}
        by_path_method_primary = {
            (o.method, o.path): o
            for o in ops_list
            if not o.operation_id.endswith("_stream") and o.mode != "sse"
        }
        additions: list[str] = []
        for ep in inventories[inv_provider]:
            if prov == "openai" and is_openai_admin_path(ep.path):
                continue
            if "http_sse" not in ep.transports:
                continue
            stream_id = ep.operation_id + "_stream"
            if stream_id in by_catalog:
                continue
            # Need a primary to clone.
            primary = by_catalog.get(ep.operation_id) or by_path_method_primary.get(
                (ep.method, ep.path)
            )
            if not primary:
                print(f"warning: no primary for SSE companion {stream_id}", file=sys.stderr)
                continue
            # Primary may already be sse-only; still ok.
            if primary.mode == "sse" and primary.operation_id.endswith("_stream"):
                continue
            base = ep.operation_id
            sse_ty = pascal(base) + "SseResult"
            ensure_sse_type(types_path, sse_ty, base)
            additions.append(make_sse_companion(primary, ep.transports))
            # Also ensure primary transport comment includes http_sse.
        if additions:
            t = ops_path.read_text()
            # Insert before final closing brace of impl.
            insert_at = t.rfind("\n}")
            if insert_at < 0:
                raise SystemExit(f"cannot find impl end in {ops_path}")
            t = t[:insert_at] + "\n" + "\n".join(additions) + t[insert_at:]
            ops_path.write_text(t)
            parsed[prov] = parse_ops_file(prov, ops_path)

    # --- OpenRouter admin credential kinds ---
    or_path = OPS_FILES["openrouter"]
    t = or_path.read_text()
    changed = False
    for op in parsed["openrouter"]:
        want_admin = is_openrouter_admin_path(op.path)
        has_admin = op.credential == "Admin"
        if want_admin and not has_admin:
            # Replace only within this method body.
            new_body = op.body.replace(
                "credential: CredentialKind::Application,",
                "credential: CredentialKind::Admin,",
            )
            if new_body != op.body:
                t = t.replace(op.body, new_body)
                changed = True
        elif not want_admin and has_admin and op.path == "/key":
            new_body = op.body.replace(
                "credential: CredentialKind::Admin,",
                "credential: CredentialKind::Application,",
            )
            t = t.replace(op.body, new_body)
            changed = True
    if changed:
        or_path.write_text(t)

    # Ensure credential is evaluated before path is moved into HttpRequestSpec.
    t = or_path.read_text()
    t2 = t.replace(
        "            path,\n            query,\n            body,\n            credential: openrouter_credential(&path),",
        "            credential: openrouter_credential(&path),\n            path,\n            query,\n            body,",
    )
    if t2 != t:
        or_path.write_text(t2)
        changed = True
    if changed:
        parsed["openrouter"] = parse_ops_file("openrouter", or_path)

    # Align primary transport comments with inventory for SSE-capable ops.
    for prov, inv_provider in (("openai", "openai"), ("openrouter", "openrouter")):
        ops_path = OPS_FILES[prov]
        t = ops_path.read_text()
        for op in parsed[prov]:
            if op.operation_id.endswith("_stream"):
                continue
            key_prov = inv_provider
            inv = inv_by_key.get((key_prov, op.method, op.path))
            if not inv:
                continue
            want = ", ".join(inv.transports)
            cur = ", ".join(op.transports_comment)
            if want != cur and set(inv.transports) != set(op.transports_comment):
                # Only expand, never shrink to unknown.
                merged = sorted(set(op.transports_comment) | set(inv.transports))
                new_body = re.sub(
                    r"    /// Transports: [^\n]+\.",
                    f"    /// Transports: {', '.join(merged)}.",
                    op.body,
                    count=1,
                )
                if new_body != op.body:
                    t = t.replace(op.body, new_body)
        ops_path.write_text(t)
        parsed[prov] = parse_ops_file(prov, ops_path)


def build_meta_rows(
    inventories: dict[str, list[InventoryEndpoint]],
    parsed: dict[str, list[ParsedOp]],
) -> list[MetaRow]:
    rows: list[MetaRow] = []
    # Index ops by (namespace, catalog operation_id) and (namespace, method, path, mode).
    ops_by_id: dict[tuple[str, str], ParsedOp] = {}
    for ns, ops in parsed.items():
        for op in ops:
            ops_by_id[(ns, op.operation_id)] = op

    # Primaries from inventory.
    for inv_provider, eps in inventories.items():
        for ep in eps:
            ns = namespace_for(inv_provider, ep.path)
            op = ops_by_id.get((ns, ep.operation_id))
            if op is None:
                # Try matching by method+path primary (non-stream).
                candidates = [
                    o
                    for o in parsed.get(ns, [])
                    if o.method == ep.method
                    and o.path == ep.path
                    and not o.operation_id.endswith("_stream")
                ]
                op = candidates[0] if candidates else None
            if op is None:
                raise SystemExit(
                    f"missing primary op for inventory {inv_provider} {ep.method} {ep.path} ({ep.operation_id})"
                )
            is_admin = (
                ns == "openai_admin"
                or op.credential == "Admin"
                or (ns == "openrouter" and is_openrouter_admin_path(ep.path))
            )
            transports = list(ep.transports)
            mode = op.mode
            # Mode flags come from the actual ops.rs method (authoritative).
            is_binary = mode == "binary"
            is_sse = mode == "sse"
            is_multipart = mode == "multipart"
            is_websocket = mode == "websocket"
            rows.append(
                MetaRow(
                    provider=ns,
                    operation_id=ep.operation_id,
                    method=ep.method,
                    path=ep.path,
                    client_type=CLIENT_TYPES[ns],
                    client_method=op.client_method,
                    request_type=op.request_type,
                    response_type=op.response_type,
                    body_type=None,
                    cli_route=f"{ns}.{op.client_method}",
                    transports=transports,
                    is_admin=is_admin,
                    is_deprecated=ep.is_deprecated,
                    is_multipart=is_multipart,
                    is_sse=is_sse,
                    is_binary=is_binary,
                    is_websocket=is_websocket,
                    is_primary=True,
                    typed_request=True,
                    typed_response=True,
                    generic_value_body=False,
                    requires_confirmation=requires_confirmation(ep.method, ep.operation_id),
                    credential_class="admin" if is_admin else "application",
                    mode=mode,
                )
            )
            # SSE companion when inventory has http_sse and primary is not pure-sse.
            if "http_sse" in transports and mode != "sse":
                stream_id = ep.operation_id + "_stream"
                stream_op = ops_by_id.get((ns, stream_id))
                if stream_op is None:
                    raise SystemExit(f"missing SSE companion op {ns}::{stream_id}")
                rows.append(
                    MetaRow(
                        provider=ns,
                        operation_id=stream_id,
                        method=ep.method,
                        path=ep.path,
                        client_type=CLIENT_TYPES[ns],
                        client_method=stream_op.client_method,
                        request_type=stream_op.request_type,
                        response_type=stream_op.response_type,
                        body_type=None,
                        cli_route=f"{ns}.{stream_op.client_method}",
                        transports=["http_sse"],
                        is_admin=is_admin,
                        is_deprecated=ep.is_deprecated,
                        is_multipart=False,
                        is_sse=True,
                        is_binary=False,
                        is_websocket=False,
                        is_primary=False,
                        typed_request=True,
                        typed_response=True,
                        generic_value_body=False,
                        requires_confirmation=requires_confirmation(ep.method, stream_id),
                        credential_class="admin" if is_admin else "application",
                        mode="sse",
                    )
                )

    # Detect extras in ops not covered by inventory primaries or companions.
    expected_ids = {(r.provider, r.operation_id) for r in rows}
    for ns, ops in parsed.items():
        for op in ops:
            key = (ns, op.operation_id)
            if key in expected_ids:
                continue
            # Allow admin namespace openai methods only if inventory had them under openai.
            # Extra ops are errors for --check; for generate we surface them.
            raise SystemExit(
                f"extra primary/companion op not in inventory-derived set: {ns}::{op.operation_id} {op.method} {op.path}"
            )
    return rows


def validate_rows(rows: list[MetaRow], inventories: dict[str, list[InventoryEndpoint]]) -> list[str]:
    errors: list[str] = []
    openai_primaries = [r for r in rows if r.provider in ("openai", "openai_admin") and r.is_primary]
    or_primaries = [r for r in rows if r.provider == "openrouter" and r.is_primary]
    sse = [r for r in rows if r.is_sse]
    binary = [r for r in rows if r.is_binary and r.is_primary]

    if len(openai_primaries) != 287:
        errors.append(f"openai primaries {len(openai_primaries)} != 287")
    if len(or_primaries) != 89:
        errors.append(f"openrouter primaries {len(or_primaries)} != 89")
    if len(sse) != 20:
        errors.append(f"sse companions/bindings {len(sse)} != 20")
    if len(binary) != 7:
        errors.append(f"binary primaries {len(binary)} != 7")

    # Inventory coverage
    inv_openai = {(e.method, e.path) for e in inventories["openai"]}
    inv_or = {(e.method, e.path) for e in inventories["openrouter"]}
    bound_openai = {(r.method, r.path) for r in openai_primaries}
    bound_or = {(r.method, r.path) for r in or_primaries}
    missing_o = sorted(inv_openai - bound_openai)
    extra_o = sorted(bound_openai - inv_openai)
    missing_r = sorted(inv_or - bound_or)
    extra_r = sorted(bound_or - inv_or)
    if missing_o:
        errors.append(f"missing openai primaries: {missing_o[:5]}...")
    if extra_o:
        errors.append(f"extra openai primaries: {extra_o[:5]}...")
    if missing_r:
        errors.append(f"missing openrouter primaries: {missing_r[:5]}...")
    if extra_r:
        errors.append(f"extra openrouter primaries: {extra_r[:5]}...")

    # Duplicates / collisions
    seen_ids: dict[tuple[str, str], int] = defaultdict(int)
    seen_methods: dict[tuple[str, str], int] = defaultdict(int)
    for r in rows:
        seen_ids[(r.provider, r.operation_id)] += 1
        seen_methods[(r.provider, r.client_method)] += 1
    for k, n in seen_ids.items():
        if n > 1:
            errors.append(f"duplicate operation_id {k}")
    for k, n in seen_methods.items():
        if n > 1:
            errors.append(f"colliding client_method {k}")

    # Skill zip binary
    for oid in ("GetSkillContent", "GetSkillVersionContent"):
        hit = next((r for r in rows if r.operation_id == oid), None)
        if hit is None or not hit.is_binary:
            errors.append(f"{oid} must be typed binary primary")

    # OpenRouter GET /key application
    key = next((r for r in rows if r.provider == "openrouter" and r.path == "/key" and r.method == "GET"), None)
    if key is None:
        errors.append("missing openrouter GET /key")
    elif key.is_admin or key.credential_class != "application":
        errors.append("openrouter GET /key must be application credential")

    # OpenRouter admin surfaces
    for r in rows:
        if r.provider != "openrouter" or not r.is_primary:
            continue
        want = is_openrouter_admin_path(r.path)
        if want and not r.is_admin:
            errors.append(f"openrouter {r.operation_id} path {r.path} must be admin")
        if not want and r.is_admin and r.path != "/key":
            # non-admin paths should not be admin
            if not is_openrouter_admin_path(r.path):
                errors.append(f"openrouter {r.operation_id} path {r.path} must not be admin")

    # Fail-closed confirmation for unknown-like: every non-safe mutation must require.
    for r in rows:
        expect = requires_confirmation(r.method, r.operation_id)
        if r.requires_confirmation != expect:
            errors.append(f"confirmation mismatch {r.operation_id}")

    # Inventory transport vs actual op-mode fidelity (authoritative inventories).
    inv_by_key: dict[tuple[str, str, str], InventoryEndpoint] = {}
    for prov, eps in inventories.items():
        for ep in eps:
            inv_by_key[(prov, ep.method, ep.path)] = ep
    for r in rows:
        inv_provider = "openrouter" if r.provider == "openrouter" else "openai"
        inv = inv_by_key.get((inv_provider, r.method, r.path))
        if inv is None:
            continue
        inv_t = set(inv.transports)
        if r.is_primary:
            if r.is_binary and "http_binary" not in inv_t:
                errors.append(
                    f"binary mode without inventory http_binary: {r.provider}::{r.operation_id}"
                )
            if r.is_multipart and "http_multipart" not in inv_t:
                errors.append(
                    f"multipart mode without inventory http_multipart: {r.provider}::{r.operation_id}"
                )
            if r.is_websocket and "websocket" not in inv_t:
                errors.append(
                    f"websocket mode without inventory websocket: {r.provider}::{r.operation_id}"
                )
            # Sole-binary inventory must not be typed as JSON primary.
            if inv_t == {"http_binary"} and not r.is_binary:
                errors.append(
                    f"inventory sole http_binary but op not binary: {r.provider}::{r.operation_id}"
                )
            # SSE inventory requires a stream companion when primary is not pure SSE.
            if "http_sse" in inv_t and r.mode != "sse":
                stream_id = r.operation_id + "_stream"
                if not any(
                    x.provider == r.provider and x.operation_id == stream_id for x in rows
                ):
                    errors.append(
                        f"inventory http_sse missing companion: {r.provider}::{stream_id}"
                    )
        else:
            # Companions are SSE-only arms.
            if r.is_sse and r.transports != ["http_sse"]:
                errors.append(
                    f"sse companion transports must be [http_sse]: {r.provider}::{r.operation_id}"
                )
            if not r.is_sse:
                errors.append(
                    f"non-primary non-sse binding: {r.provider}::{r.operation_id}"
                )

    return errors


def emit_bindings_rs(rows: list[MetaRow]) -> str:
    lines = [
        "//! Operation bindings (generated).",
        "//! DO NOT EDIT BY HAND. Source: baselines/scripts/generate_operation_metadata.py",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct OperationBinding {",
        "    pub provider: &'static str,",
        "    pub operation_id: &'static str,",
        "    pub method: &'static str,",
        "    pub path: &'static str,",
        "    pub client_type: &'static str,",
        "    pub client_method: &'static str,",
        "    pub request_type: &'static str,",
        "    pub response_type: &'static str,",
        "    pub cli_route: &'static str,",
        "    pub transports: &'static [&'static str],",
        "    pub is_admin: bool,",
        "    pub is_deprecated: bool,",
        "    pub is_multipart: bool,",
        "    pub is_sse: bool,",
        "    pub is_binary: bool,",
        "    pub is_websocket: bool,",
        "    pub is_primary: bool,",
        "    pub typed_request: bool,",
        "    pub typed_response: bool,",
        "    pub generic_value_body: bool,",
        "    pub requires_confirmation: bool,",
        "    pub credential_class: &'static str,",
        "}",
        "",
        "pub static OPERATION_BINDINGS: &[OperationBinding] = &[",
    ]
    for b in rows:
        transports = ", ".join(f'"{t}"' for t in b.transports)
        lines.append("    OperationBinding {")
        for f in (
            "provider",
            "operation_id",
            "method",
            "path",
            "client_type",
            "client_method",
            "request_type",
            "response_type",
            "cli_route",
        ):
            lines.append(f'        {f}: {json.dumps(getattr(b, f))},')
        lines.append(f"        transports: &[{transports}],")
        for f in (
            "is_admin",
            "is_deprecated",
            "is_multipart",
            "is_sse",
            "is_binary",
            "is_websocket",
            "is_primary",
            "typed_request",
            "typed_response",
            "generic_value_body",
            "requires_confirmation",
        ):
            lines.append(f"        {f}: {str(getattr(b, f)).lower()},")
        lines.append(f'        credential_class: {json.dumps(b.credential_class)},')
        lines.append("    },")
    lines.append("];")
    lines.append(
        f"pub const OPENAI_APP_BINDING_COUNT: usize = {sum(1 for b in rows if b.provider == 'openai')};"
    )
    lines.append(
        f"pub const OPENAI_ADMIN_BINDING_COUNT: usize = {sum(1 for b in rows if b.provider == 'openai_admin')};"
    )
    lines.append(
        f"pub const OPENROUTER_BINDING_COUNT: usize = {sum(1 for b in rows if b.provider == 'openrouter')};"
    )
    lines.append(f"pub const TOTAL_BINDING_COUNT: usize = {len(rows)};")
    lines.append(
        f"pub const OPENAI_PRIMARY_COUNT: usize = {sum(1 for b in rows if b.provider in ('openai','openai_admin') and b.is_primary)};"
    )
    lines.append(
        f"pub const OPENROUTER_PRIMARY_COUNT: usize = {sum(1 for b in rows if b.provider == 'openrouter' and b.is_primary)};"
    )
    lines.append(
        f"pub const SSE_COMPANION_COUNT: usize = {sum(1 for b in rows if b.is_sse)};"
    )
    lines.append(
        f"pub const BINARY_PRIMARY_COUNT: usize = {sum(1 for b in rows if b.is_binary and b.is_primary)};"
    )
    lines.append("")
    lines.append(
        "pub fn find_binding(provider: &str, operation_id: &str) -> Option<&'static OperationBinding> {"
    )
    lines.append(
        "    OPERATION_BINDINGS\n        .iter()\n        .find(|b| b.provider == provider && b.operation_id == operation_id)"
    )
    lines.append("}")
    lines.append("")
    lines.append("/// OpenRouter management path classification (provider-native).")
    lines.append("pub fn openrouter_path_is_admin(path: &str) -> bool {")
    lines.append("    const ADMIN_PREFIXES: &[&str] = &[")
    lines.append('        "/keys",')
    lines.append('        "/auth/keys",')
    lines.append('        "/byok",')
    lines.append('        "/workspaces",')
    lines.append('        "/guardrails",')
    lines.append('        "/observability",')
    lines.append('        "/organization",')
    lines.append("    ];")
    lines.append("    for prefix in ADMIN_PREFIXES {")
    lines.append(
        '        if path == *prefix || path.starts_with(&format!("{prefix}/")) {'
    )
    lines.append("            return true;")
    lines.append("        }")
    lines.append("    }")
    lines.append("    false")
    lines.append("}")
    lines.append("")
    lines.append("/// Fail-closed mutation confirmation for a known method/operation_id pair.")
    lines.append(
        "pub fn operation_requires_confirmation(method: &str, operation_id: &str) -> bool {"
    )
    lines.append('    if matches!(method, "GET" | "HEAD" | "OPTIONS") {')
    lines.append("        return false;")
    lines.append("    }")
    # Emit allowlist from SAFE_NO_CONFIRM_OPERATION_IDS
    lines.append("    const SAFE: &[&str] = &[")
    for oid in sorted(SAFE_NO_CONFIRM_OPERATION_IDS):
        lines.append(f'        "{oid}",')
    lines.append("    ];")
    lines.append("    if SAFE.contains(&operation_id) {")
    lines.append("        return false;")
    lines.append("    }")
    lines.append("    true")
    lines.append("}")
    lines.append("")
    return "\n".join(lines) + "\n"


def emit_cli_ops(rows: list[MetaRow]) -> str:
    lines = [
        "//! Generated CLI catalog.",
        "//! DO NOT EDIT BY HAND. Source: baselines/scripts/generate_operation_metadata.py",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct CliOperation {",
        "    pub provider_namespace: &'static str,",
        "    pub operation_id: &'static str,",
        "    pub method: &'static str,",
        "    pub path: &'static str,",
        "    pub client_method: &'static str,",
        "    pub request_type: &'static str,",
        "    pub response_type: &'static str,",
        "    pub cli_route: &'static str,",
        "    pub transports: &'static [&'static str],",
        "    pub is_admin: bool,",
        "    pub is_deprecated: bool,",
        "    pub is_multipart: bool,",
        "    pub is_sse: bool,",
        "    pub is_binary: bool,",
        "    pub is_websocket: bool,",
        "    pub is_primary: bool,",
        "    pub typed_request: bool,",
        "    pub requires_confirmation: bool,",
        "    pub credential_class: &'static str,",
        "}",
        "pub static CLI_OPERATIONS: &[CliOperation] = &[",
    ]
    for b in rows:
        transports = ", ".join(f'"{t}"' for t in b.transports)
        lines.append("    CliOperation {")
        lines.append(f'        provider_namespace: {json.dumps(b.provider)},')
        for f in (
            "operation_id",
            "method",
            "path",
            "client_method",
            "request_type",
            "response_type",
            "cli_route",
        ):
            lines.append(f"        {f}: {json.dumps(getattr(b, f))},")
        lines.append(f"        transports: &[{transports}],")
        for f in (
            "is_admin",
            "is_deprecated",
            "is_multipart",
            "is_sse",
            "is_binary",
            "is_websocket",
            "is_primary",
            "typed_request",
            "requires_confirmation",
        ):
            lines.append(f"        {f}: {str(getattr(b, f)).lower()},")
        lines.append(f'        credential_class: {json.dumps(b.credential_class)},')
        lines.append("    },")
    lines.append("];")
    lines.append(f"pub const CLI_OPERATION_COUNT: usize = {len(rows)};")
    lines.append(
        "pub fn find_cli_operation(namespace: &str, operation_id: &str) -> Option<&'static CliOperation> {"
    )
    lines.append(
        "    CLI_OPERATIONS\n        .iter()\n        .find(|op| op.provider_namespace == namespace && op.operation_id == operation_id)"
    )
    lines.append("}")
    lines.append(
        "pub fn operations_for_namespace(namespace: &str) -> impl Iterator<Item = &'static CliOperation> {"
    )
    lines.append(
        "    CLI_OPERATIONS\n        .iter()\n        .filter(move |op| op.provider_namespace == namespace)"
    )
    lines.append("}")
    lines.append("")
    lines.append("/// Fail-closed mutation gate for CLI call paths.")
    lines.append(
        "pub fn operation_requires_confirmation(namespace: &str, operation_id: &str) -> bool {"
    )
    lines.append("    match find_cli_operation(namespace, operation_id) {")
    lines.append("        Some(op) => op.requires_confirmation,")
    lines.append("        // Unknown/missing metadata fails closed.")
    lines.append("        None => true,")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    return "\n".join(lines) + "\n"


def emit_dispatch_arm(row: MetaRow) -> str:
    ns = row.provider
    types = TYPES_MODS[ns]
    oid = row.operation_id
    req = row.request_type
    fn = row.client_method
    lines = [f'        "{oid}" => {{']
    lines.append(f"            let req: {types}::{req} = decode_params(merged)?;")
    if row.is_multipart:
        lines.append("            let files = multipart_from(multipart_files);")
        lines.append(
            f"            let resp = client.{fn}(req, files).await.map_err(|e| e.to_string())?;"
        )
        lines.append("            write_json(&resp).map_err(|e| e.to_string())?;")
    elif row.is_binary:
        lines.append(
            f"            let resp = client.{fn}(req, output).await.map_err(|e| e.to_string())?;"
        )
        lines.append("            if output.is_some() {")
        lines.append(
            '                write_json(&json!({"ok": true, "bytes": resp.bytes.len()})).map_err(|e| e.to_string())?;'
        )
        lines.append("            } else {")
        lines.append(
            "                return write_binary(&resp.bytes, None).map_err(|e| e.to_string());"
        )
        lines.append("            }")
    elif row.is_sse:
        lines.append(
            f"            let resp = client.{fn}(req).await.map_err(|e| e.to_string())?;"
        )
        lines.append("            if stream {")
        lines.append("                for ev in &resp.events {")
        lines.append(
            "                    write_ndjson_line(ev).map_err(|e| e.to_string())?;"
        )
        lines.append("                }")
        lines.append("            } else {")
        lines.append("                write_json(&resp).map_err(|e| e.to_string())?;")
        lines.append("            }")
    else:
        lines.append(
            f"            let resp = client.{fn}(req).await.map_err(|e| e.to_string())?;"
        )
        lines.append("            if stream {")
        lines.append(
            "                write_ndjson_line(&resp).map_err(|e| e.to_string())?;"
        )
        lines.append("            } else {")
        lines.append("                write_json(&resp).map_err(|e| e.to_string())?;")
        lines.append("            }")
    lines.append("            Ok(ExitCode::Success)")
    lines.append("        }")
    return "\n".join(lines)


def emit_typed_dispatch(rows: list[MetaRow]) -> str:
    header = '''//! Typed CLI dispatch runtime. DO NOT EDIT BY HAND.
//! Source: baselines/scripts/generate_operation_metadata.py
use super::generated_ops::CliOperation;
use super::output::{ExitCode, write_binary, write_json, write_ndjson_line};
use crate::provider_registry::id::ProviderId;
use crate::provider_registry::secrets::{
    admin_key_scope, application_key_scope, read_provider_secret,
};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;
use xai_grok_inference::openai_platform::MultipartFiles;
use xai_grok_inference::openai_platform::generated::{
    openai_admin_types, openai_types, openrouter_types,
};
use xai_grok_inference::{
    OpenAiAdminClient, OpenAiClient, OpenRouterClient, PlatformClientConfig, TransportPolicy,
};

fn merge_params(
    input_json: Option<&str>,
    path_params: &[(String, String)],
    query: &[(String, String)],
) -> Result<Value, String> {
    let mut obj = match input_json {
        Some(s) if !s.trim().is_empty() => {
            let v: Value =
                serde_json::from_str(s).map_err(|e| format!("typed request JSON: {e}"))?;
            match v {
                Value::Object(m) => m,
                other => {
                    let mut m = serde_json::Map::new();
                    m.insert("body".into(), other);
                    m
                }
            }
        }
        _ => serde_json::Map::new(),
    };
    for (k, v) in path_params {
        obj.insert(k.clone(), Value::String(v.clone()));
    }
    for (k, v) in query {
        obj.insert(k.clone(), Value::String(v.clone()));
    }
    Ok(Value::Object(obj))
}
fn decode_params<T: DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| {
        format!(
            "typed Params deserialize ({}): {e}",
            std::any::type_name::<T>()
        )
    })
}
fn multipart_from(files: &[(String, PathBuf)]) -> MultipartFiles {
    let mut m = MultipartFiles::new();
    for (field, path) in files {
        m = m.file(field.clone(), path.clone());
    }
    m
}

pub async fn dispatch_runtime(
    provider: &str,
    op: &CliOperation,
    path_params: &[(String, String)],
    query: &[(String, String)],
    input_json: Option<&str>,
    dry_run: bool,
    stream: bool,
    output: Option<&Path>,
    multipart_files: &[(String, PathBuf)],
) -> Result<ExitCode, String> {
    if !matches!(
        op.method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD"
    ) {
        return Err(format!("unsupported HTTP method {}", op.method));
    }
    // Merge typed params first. Dry-run must return before any credential
    // resolver, env/vault/auth-file access, token construction, or network setup.
    let merged = merge_params(input_json, path_params, query)?;
    if dry_run {
        write_json(&json!({
            "provider": provider,
            "operation_id": op.operation_id,
            "request_type": op.request_type,
            "response_type": op.response_type,
            "client_method": op.client_method,
            "transports": op.transports,
            "credential_class": op.credential_class,
            "requires_confirmation": op.requires_confirmation,
            "typed_request": merged,
            "dry_run": true,
        }))
        .map_err(|e| e.to_string())?;
        return Ok(ExitCode::Success);
    }
    note_live_dispatch_credential_phase();
    let home = xai_grok_config::grok_home();
    let meta = resolve_provider_from_registry(provider, &home)?;
    let pid = ProviderId::new(provider).map_err(|e| e.to_string())?;
    // Credential selection is provider-native and metadata-driven: admin slots
    // never fall back to the application key when admin is missing.
    let want_admin = op.is_admin || op.credential_class == "admin";
    let app_token = if want_admin {
        None
    } else {
        resolve_app_token(provider, &home, &pid, meta.env_key.as_deref())
    };
    let admin_token = resolve_admin_token(provider, &home, &pid, meta.admin_env_key.as_deref());
    if want_admin && admin_token.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
        return Err(format!(
            "admin credential required for {}::{} (never borrowing application key)",
            op.provider_namespace, op.operation_id
        ));
    }
    let cfg = PlatformClientConfig {
        provider_id: provider.to_owned(),
        display_name: meta.display_name,
        base_url: meta.base_url,
        admin_base_url: meta.admin_base_url,
        application_token: if want_admin { None } else { app_token },
        admin_token,
        extra_headers: meta.extra_headers.into_iter().collect(),
        policy: TransportPolicy::default(),
    };
    match op.provider_namespace {
        "openai" => {
            let client = OpenAiClient::from_config(cfg, CancellationToken::new())
                .map_err(|e| e.to_string())?;
            dispatch_openai(client, op, merged, stream, output, multipart_files).await
        }
        "openai_admin" => {
            let client = OpenAiAdminClient::from_config(cfg, CancellationToken::new())
                .map_err(|e| e.to_string())?;
            dispatch_openai_admin(client, op, merged, stream, output, multipart_files).await
        }
        "openrouter" => {
            let client = OpenRouterClient::from_config(cfg, CancellationToken::new())
                .map_err(|e| e.to_string())?;
            dispatch_openrouter(client, op, merged, stream, output, multipart_files).await
        }
        other => Err(format!("unknown namespace {other}")),
    }
}

/// Test seam: live dispatch increments this before any credential resolution.
/// Dry-run must never call this (proves zero credential-phase entry).
#[cfg(test)]
pub(crate) static LIVE_CREDENTIAL_PHASE_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[inline]
fn note_live_dispatch_credential_phase() {
    #[cfg(test)]
    LIVE_CREDENTIAL_PHASE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}
'''
    parts = [header]
    for ns, fn_name, client_ty in (
        ("openai", "dispatch_openai", "OpenAiClient"),
        ("openai_admin", "dispatch_openai_admin", "OpenAiAdminClient"),
        ("openrouter", "dispatch_openrouter", "OpenRouterClient"),
    ):
        parts.append(
            f"""
async fn {fn_name}(
    client: {client_ty},
    op: &CliOperation,
    merged: Value,
    stream: bool,
    output: Option<&Path>,
    multipart_files: &[(String, PathBuf)],
) -> Result<ExitCode, String> {{
    match op.operation_id {{
"""
        )
        for row in rows:
            if row.provider != ns:
                continue
            parts.append(emit_dispatch_arm(row))
            parts.append("")
        parts.append(
            """        other => Err(format!("no typed dispatch arm for {other}")),
    }
}
"""
        )

    parts.append(
        '''
struct ProviderMeta {
    base_url: String,
    display_name: String,
    admin_base_url: Option<String>,
    extra_headers: IndexMap<String, String>,
    env_key: Option<String>,
    admin_env_key: Option<String>,
}

fn resolve_provider_from_registry(provider: &str, home: &Path) -> Result<ProviderMeta, String> {
    match provider {
        "openai" => {
            return Ok(ProviderMeta {
                base_url: "https://api.openai.com/v1".into(),
                display_name: "OpenAI".into(),
                admin_base_url: None,
                extra_headers: IndexMap::new(),
                env_key: Some("OPENAI_API_KEY".into()),
                admin_env_key: Some("OPENAI_ADMIN_KEY".into()),
            });
        }
        "openrouter" => {
            return Ok(ProviderMeta {
                base_url: "https://openrouter.ai/api/v1".into(),
                display_name: "OpenRouter".into(),
                admin_base_url: None,
                extra_headers: IndexMap::new(),
                env_key: Some("OPENROUTER_API_KEY".into()),
                // Prefer OPENROUTER_ADMIN_API_KEY; OPENROUTER_MANAGEMENT_API_KEY is alias.
                admin_env_key: Some("OPENROUTER_ADMIN_API_KEY".into()),
            });
        }
        "zai" | "zai-model-api" => {
            return Ok(ProviderMeta {
                base_url: crate::agent::zai::ZAI_DEFAULT_BASE_URL.into(),
                display_name: "Z.ai".into(),
                admin_base_url: None,
                extra_headers: IndexMap::new(),
                env_key: Some(crate::agent::zai::ZAI_ENV_KEY.into()),
                admin_env_key: None,
            });
        }
        _ => {}
    }
    let cfg_path = home.join("config.toml");
    let raw = std::fs::read_to_string(&cfg_path).map_err(|e| format!("read config.toml: {e}"))?;
    let val: toml::Value = raw.parse().map_err(|e| format!("parse config: {e}"))?;
    let entry = val
        .get("model_providers")
        .and_then(|t| t.get(provider))
        .ok_or_else(|| format!("provider `{provider}` not found in config.toml"))?;
    let _ = ProviderId::new(provider).map_err(|e| e.to_string())?;
    let base = entry
        .get("base_url")
        .or_else(|| entry.get("api_base_url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("provider `{provider}` missing base_url"))?
        .to_owned();
    crate::provider_registry::lifecycle::validate_http_base_url(&base)
        .map_err(|e| e.to_string())?;
    let admin_base = entry
        .get("admin_base_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    if let Some(ref a) = admin_base {
        crate::provider_registry::lifecycle::validate_http_base_url(a)
            .map_err(|e| e.to_string())?;
    }
    let display_name = entry
        .get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or(provider)
        .to_owned();
    let mut extra_headers = IndexMap::new();
    if let Some(h) = entry.get("extra_headers").and_then(|v| v.as_table()) {
        for (k, v) in h {
            if let Some(s) = v.as_str() {
                extra_headers.insert(k.clone(), s.to_owned());
            }
        }
    }
    crate::provider_registry::lifecycle::validate_extra_headers(&extra_headers)
        .map_err(|e| e.to_string())?;
    let env_key = entry.get("env_key").and_then(|v| {
        v.as_str().map(|s| s.to_owned()).or_else(|| {
            v.as_array()
                .and_then(|a| a.first())
                .and_then(|x| x.as_str())
                .map(|s| s.to_owned())
        })
    });
    let admin_env_key = entry
        .get("admin_env_key")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    Ok(ProviderMeta {
        base_url: base,
        display_name,
        admin_base_url: admin_base,
        extra_headers,
        env_key,
        admin_env_key,
    })
}

fn resolve_app_token(
    provider: &str,
    home: &Path,
    pid: &ProviderId,
    env_key: Option<&str>,
) -> Option<String> {
    if let Some(name) = env_key {
        if let Ok(v) = std::env::var(name) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    match provider {
        "openai" => crate::auth::read_provider_api_key(home, crate::auth::OPENAI_API_KEY_SCOPE)
            .ok()
            .flatten()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok()),
        "openrouter" => {
            crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_API_KEY_SCOPE)
                .ok()
                .flatten()
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
        }
        "zai" | "zai-model-api" => read_provider_secret(home, &application_key_scope(pid))
            .ok()
            .flatten()
            .or_else(|| std::env::var(crate::agent::zai::ZAI_ENV_KEY).ok()),
        _ => read_provider_secret(home, &application_key_scope(pid))
            .ok()
            .flatten(),
    }
}

fn resolve_admin_token(
    provider: &str,
    home: &Path,
    pid: &ProviderId,
    admin_env_key: Option<&str>,
) -> Option<String> {
    // Never fall back to the application key when admin is missing.
    if let Some(name) = admin_env_key {
        if let Ok(v) = std::env::var(name) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    // Built-in OpenRouter management alias.
    if provider == "openrouter" {
        if let Ok(v) = std::env::var("OPENROUTER_MANAGEMENT_API_KEY") {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(v) = std::env::var("OPENROUTER_ADMIN_API_KEY") {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(Some(v)) =
            crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_ADMIN_KEY_SCOPE)
        {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(Some(v)) =
            crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_MANAGEMENT_KEY_SCOPE)
        {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    if provider == "openai" {
        if let Ok(v) = std::env::var("OPENAI_ADMIN_KEY") {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(Some(v)) =
            crate::auth::read_provider_api_key(home, crate::auth::OPENAI_ADMIN_KEY_SCOPE)
        {
            return Some(v);
        }
    }
    read_provider_secret(home, &admin_key_scope(pid))
        .ok()
        .flatten()
}
'''
    )
    return "\n".join(parts) + "\n"


def emit_operation_table(rows: list[MetaRow]) -> dict[str, Any]:
    return {
        "format_version": 1,
        "source": "inventories+ops",
        "openai_primary_count": sum(
            1 for r in rows if r.provider in ("openai", "openai_admin") and r.is_primary
        ),
        "openrouter_primary_count": sum(
            1 for r in rows if r.provider == "openrouter" and r.is_primary
        ),
        "sse_companion_count": sum(1 for r in rows if r.is_sse),
        "binary_primary_count": sum(1 for r in rows if r.is_binary and r.is_primary),
        "total": len(rows),
        "operations": [asdict(r) for r in rows],
    }


def emit_bindings_report(rows: list[MetaRow]) -> dict[str, Any]:
    return {
        "format_version": 5,
        "total": len(rows),
        "generic_value_body_count": sum(1 for b in rows if b.generic_value_body),
        "multipart_count": sum(1 for b in rows if b.is_multipart),
        "sse_count": sum(1 for b in rows if b.is_sse),
        "binary_count": sum(1 for b in rows if b.is_binary),
        "binary_primary_count": sum(1 for b in rows if b.is_binary and b.is_primary),
        "websocket_count": sum(1 for b in rows if b.is_websocket),
        "openai_app": sum(1 for b in rows if b.provider == "openai"),
        "openai_admin": sum(1 for b in rows if b.provider == "openai_admin"),
        "openrouter": sum(1 for b in rows if b.provider == "openrouter"),
        "openai_primary_count": sum(
            1 for b in rows if b.provider in ("openai", "openai_admin") and b.is_primary
        ),
        "openrouter_primary_count": sum(
            1 for b in rows if b.provider == "openrouter" and b.is_primary
        ),
        "bindings": [asdict(r) for r in rows],
    }


def rustfmt(paths: list[Path]) -> None:
    cmd = ["rustfmt", "--edition", "2024"]
    if RUSTFMT_TOML.exists():
        cmd += ["--config-path", str(RUSTFMT_TOML)]
    cmd += [str(p) for p in paths]
    subprocess.run(cmd, check=True, cwd=REPO)


def load_all() -> tuple[dict[str, list[InventoryEndpoint]], dict[str, list[ParsedOp]]]:
    inventories = {
        "openai": load_inventory(OPENAI_INV, "openai"),
        "openrouter": load_inventory(OPENROUTER_INV, "openrouter"),
    }
    parsed = {ns: parse_ops_file(ns, p) for ns, p in OPS_FILES.items()}
    return inventories, parsed


def generate(write: bool) -> list[str]:
    inventories, parsed = load_all()
    if write:
        repair_ops_and_types(inventories, parsed)
        inventories, parsed = load_all()
    rows = build_meta_rows(inventories, parsed)
    errors = validate_rows(rows, inventories)
    if write and not errors:
        table = emit_operation_table(rows)
        report = emit_bindings_report(rows)
        (BASELINES / "operation_table.json").write_text(
            json.dumps(table, indent=2) + "\n"
        )
        (BASELINES / "operation_bindings_report.json").write_text(
            json.dumps(report, indent=2) + "\n"
        )
        (GEN / "bindings.rs").write_text(emit_bindings_rs(rows))
        (SHELL_CLI / "generated_ops.rs").write_text(emit_cli_ops(rows))
        (SHELL_CLI / "typed_dispatch_runtime.rs").write_text(emit_typed_dispatch(rows))
        rustfmt(
            [
                GEN / "bindings.rs",
                GEN / "openai_ops.rs",
                GEN / "openai_types.rs",
                GEN / "openrouter_ops.rs",
                GEN / "openrouter_types.rs",
                GEN / "openai_admin_ops.rs",
                SHELL_CLI / "generated_ops.rs",
                SHELL_CLI / "typed_dispatch_runtime.rs",
            ]
        )
    return errors


def _rustfmt_text(source: str, name: str) -> str:
    """Format Rust source through repository rustfmt for deterministic compare."""
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / name
        p.write_text(source)
        rustfmt([p])
        return p.read_text()


def _extract_match_arm_body(source: str, operation_id: str) -> str | None:
    """Return the body of `"operation_id" => { ... }` (brace-balanced)."""
    needle = f'"{operation_id}" =>'
    idx = source.find(needle)
    if idx < 0:
        return None
    brace = source.find("{", idx)
    if brace < 0:
        return None
    depth = 0
    for i in range(brace, len(source)):
        ch = source[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return source[brace : i + 1]
    return None


def _assert_dispatch_arm_methods(dispatch: str, rows: list[MetaRow]) -> list[str]:
    """Per-arm method fidelity: each arm body must call its client_method."""
    errors: list[str] = []
    for r in rows:
        body = _extract_match_arm_body(dispatch, r.operation_id)
        if body is None:
            errors.append(f"missing dispatch arm for {r.provider}::{r.operation_id}")
            continue
        if f".{r.client_method}(" not in body:
            errors.append(
                f"dispatch arm for {r.operation_id} lacks local typed call .{r.client_method}("
            )
        # Detect swapped methods: another op's method in this arm when not own.
        # (Own method required above; extra foreign methods are allowed only if
        # they are not a different catalog method — keep this strict for primary.)
    return errors


def _check_detects_swapped_method(rows: list[MetaRow]) -> list[str]:
    """Regression: swapped client_method in one arm must fail check logic.

    Does not mutate on-disk sources. Mutates an in-memory emit only.
    """
    if len(rows) < 2:
        return ["swap fixture needs >=2 rows"]
    # Prefer two same-namespace rows with distinct methods.
    a = next((r for r in rows if r.provider == "openai" and r.is_primary), rows[0])
    b = next(
        (
            r
            for r in rows
            if r.provider == a.provider
            and r.client_method != a.client_method
            and r.is_primary
        ),
        None,
    )
    if b is None:
        return ["swap fixture could not find distinct methods"]
    source = emit_typed_dispatch(rows)
    body = _extract_match_arm_body(source, a.operation_id)
    if body is None:
        return [f"swap fixture missing arm {a.operation_id}"]
    poisoned = body.replace(f".{a.client_method}(", f".{b.client_method}(", 1)
    if poisoned == body:
        return [f"swap fixture could not rewrite method in {a.operation_id}"]
    mutated = source.replace(body, poisoned, 1)
    errs = _assert_dispatch_arm_methods(mutated, rows)
    if not any(a.operation_id in e for e in errs):
        return [
            f"swap fixture failed to detect swapped method in {a.operation_id} "
            f"({a.client_method} -> {b.client_method})"
        ]
    return []


def check_only() -> int:
    # Compare derived artifacts to freshly computed content without writing ops
    # repairs first — but repairs must already be applied for --check to pass.
    inventories, parsed = load_all()
    try:
        rows = build_meta_rows(inventories, parsed)
    except SystemExit as e:
        print(f"CHECK FAIL: {e}", file=sys.stderr)
        return 1
    errors = validate_rows(rows, inventories)

    # Stale artifact detection. Rust sources are compared after rustfmt so
    # formatting is deterministic against repository rustfmt.toml.
    expected_bindings = _rustfmt_text(emit_bindings_rs(rows), "bindings.rs")
    expected_cli = _rustfmt_text(emit_cli_ops(rows), "generated_ops.rs")
    expected_dispatch = _rustfmt_text(emit_typed_dispatch(rows), "typed_dispatch_runtime.rs")
    expected_table = json.dumps(emit_operation_table(rows), indent=2) + "\n"
    expected_report = json.dumps(emit_bindings_report(rows), indent=2) + "\n"
    pairs = [
        (GEN / "bindings.rs", expected_bindings),
        (SHELL_CLI / "generated_ops.rs", expected_cli),
        (SHELL_CLI / "typed_dispatch_runtime.rs", expected_dispatch),
        (BASELINES / "operation_table.json", expected_table),
        (BASELINES / "operation_bindings_report.json", expected_report),
    ]
    for path, expected in pairs:
        if not path.exists():
            errors.append(f"missing derived artifact {path}")
            continue
        actual = path.read_text()
        if actual != expected:
            errors.append(f"stale generated artifact {path}")

    # Per-arm dispatch method fidelity (not global substring).
    dispatch = (SHELL_CLI / "typed_dispatch_runtime.rs").read_text()
    errors.extend(_assert_dispatch_arm_methods(dispatch, rows))

    # Self-test: swapped method must be rejected without mutating source.
    errors.extend(_check_detects_swapped_method(rows))

    if errors:
        print("CHECK FAIL:", file=sys.stderr)
        for e in errors[:50]:
            print(f"  - {e}", file=sys.stderr)
        if len(errors) > 50:
            print(f"  ... and {len(errors) - 50} more", file=sys.stderr)
        return 1
    print(
        "CHECK OK:",
        f"openai_primary=287 openrouter_primary=89 sse={sum(1 for r in rows if r.is_sse)} "
        f"binary_primary={sum(1 for r in rows if r.is_binary and r.is_primary)} total={len(rows)}",
    )
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--check",
        action="store_true",
        help="Validate inventories, ops, and derived artifacts without writing",
    )
    ap.add_argument(
        "--write",
        action="store_true",
        help="Repair ops transport gaps and regenerate derived caches/artifacts",
    )
    args = ap.parse_args()
    if args.check and args.write:
        print("use either --check or --write", file=sys.stderr)
        return 2
    if args.check or not args.write:
        # Default to check when neither flag? Prefer explicit.
        if not args.write:
            return check_only()
    errors = generate(write=True)
    if errors:
        print("GENERATE validation issues:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1
    # Re-check after write.
    return check_only()


if __name__ == "__main__":
    raise SystemExit(main())
