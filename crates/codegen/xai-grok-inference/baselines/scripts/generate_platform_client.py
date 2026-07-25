#!/usr/bin/env python3
"""Generate typed OpenAI / OpenRouter platform client bindings from inventories.

Usage (from repository root):
  python3 crates/codegen/xai-grok-inference/baselines/scripts/generate_platform_client.py

Deterministic, offline, no network. Regenerates:
  - src/openai_platform/generated/*.rs
  - baselines/operation_bindings_report.json
"""

from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from pathlib import Path

SCRIPT = Path(__file__).resolve()
ROOT = SCRIPT.parents[2]  # xai-grok-inference
OUT = ROOT / "src" / "openai_platform" / "generated"


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
        "abstract",
        "become",
        "box",
        "do",
        "final",
        "macro",
        "override",
        "priv",
        "typeof",
        "unsized",
        "virtual",
        "yield",
        "try",
        "gen",
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
    }
    if s3 in keywords:
        s3 = f"{s3}_"
    return s3


def type_name(op_id: str, suffix: str) -> str:
    parts = re.sub(r"[^a-zA-Z0-9]+", "_", op_id).split("_")
    base = "".join(p[:1].upper() + p[1:] for p in parts if p)
    if not base:
        base = "Op"
    if base[0].isdigit():
        base = f"Op{base}"
    return f"{base}{suffix}"


def path_params(path: str) -> list[str]:
    return re.findall(r"\{([a-zA-Z0-9_]+)\}", path)


def family_from_path(path: str) -> str:
    p = path.strip("/")
    if not p:
        return "root"
    first = p.split("/")[0]
    if first == "organization":
        return "admin"
    if first in ("fine_tuning", "fine-tuning"):
        return "fine_tuning"
    if first == "vector_stores":
        return "vector_stores"
    return first.replace("-", "_")


def is_admin_path(path: str) -> bool:
    return path.startswith("/organization") or path.startswith("/dashboard")


def load_inventory(path: Path) -> dict:
    return json.loads(path.read_text())


def generate_ops(provider: str, endpoints: list[dict], admin_default: bool) -> tuple[str, list[dict]]:
    bindings: list[dict] = []
    lines: list[str] = []
    lines.append(f"//! Generated typed operations for {provider} platform baseline.")
    lines.append(
        "//! DO NOT EDIT BY HAND — regenerate via baselines/scripts/generate_platform_client.py"
    )
    lines.append("")
    lines.append("use super::super::error::{PlatformError, PlatformResult};")
    lines.append(
        "use super::super::transport::{CredentialKind, HttpRequestSpec, PlatformTransport};"
    )
    lines.append("use serde::{Deserialize, Serialize};")
    lines.append("use serde_json::Value;")
    lines.append("use std::collections::BTreeMap;")
    lines.append("")

    if provider == "openai":
        client_type = "OpenAiClient"
    elif provider == "openai_admin":
        client_type = "OpenAiAdminClient"
    else:
        client_type = "OpenRouterClient"

    for ep in endpoints:
        op = ep.get("operation_id") or f"anon_{ep['method'].lower()}_{ep['path']}"
        req_ty = type_name(op, "Request")
        resp_ty = type_name(op, "Response")
        params = path_params(ep["path"])
        has_body = bool(ep.get("request_content_types"))
        lines.append(f"/// Request for `{ep['method']} {ep['path']}` (`{op}`).")
        lines.append("#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]")
        lines.append(f"pub struct {req_ty} {{")
        for p in params:
            lines.append(f"    pub {camel_to_snake(p)}: String,")
        if has_body:
            lines.append(
                "    /// Typed JSON body for this operation (deserialized, not raw-forwarded)."
            )
            lines.append("    #[serde(flatten)]")
            lines.append(f"    pub body: {type_name(op, 'Body')},")
        if ep["method"].upper() == "GET" and not has_body:
            lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
            lines.append("    pub limit: Option<u32>,")
            lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
            lines.append("    pub after: Option<String>,")
            lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
            lines.append("    pub before: Option<String>,")
            lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
            lines.append("    pub order: Option<String>,")
            lines.append("    /// Additional documented query parameters for this operation.")
            lines.append("    #[serde(default, flatten)]")
            lines.append("    pub query: BTreeMap<String, Value>,")
        lines.append("}")
        lines.append("")
        if has_body:
            lines.append(f"/// Body for `{op}`.")
            lines.append("#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]")
            lines.append(f"pub struct {type_name(op, 'Body')} {{")
            lines.append("    /// Documented and additive fields accepted for this operation.")
            lines.append("    #[serde(flatten)]")
            lines.append("    pub fields: BTreeMap<String, Value>,")
            lines.append("}")
            lines.append("")
            lines.append(f"impl {type_name(op, 'Body')} {{")
            lines.append("    pub fn from_json(value: Value) -> PlatformResult<Self> {")
            lines.append(
                "        serde_json::from_value(value)"
                ".map_err(|e| PlatformError::InvalidRequest(e.to_string()))"
            )
            lines.append("    }")
            lines.append("}")
            lines.append("")
        lines.append(f"/// Response for `{ep['method']} {ep['path']}` (`{op}`).")
        lines.append("#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]")
        lines.append(f"pub struct {resp_ty} {{")
        lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
        lines.append("    pub object: Option<String>,")
        lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
        lines.append("    pub id: Option<String>,")
        lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
        lines.append("    pub data: Option<Vec<Value>>,")
        lines.append("    #[serde(default, flatten)]")
        lines.append("    pub fields: BTreeMap<String, Value>,")
        lines.append("}")
        lines.append("")

    lines.append(f"impl crate::openai_platform::client::{client_type} {{")
    seen_fns: set[str] = set()
    for ep in endpoints:
        op = ep.get("operation_id") or f"anon_{ep['method'].lower()}_{ep['path']}"
        fn = camel_to_snake(op)
        base_fn = fn
        n = 2
        while fn in seen_fns:
            fn = f"{base_fn}_{n}"
            n += 1
        seen_fns.add(fn)
        req_ty = type_name(op, "Request")
        resp_ty = type_name(op, "Response")
        params = path_params(ep["path"])
        has_body = bool(ep.get("request_content_types"))
        transports = ep.get("transports") or (
            ["http_json"] if not ep.get("transport") else [ep["transport"]]
        )
        method = ep["method"].upper()
        path = ep["path"]
        admin = admin_default or is_admin_path(path)
        cred = "CredentialKind::Admin" if admin else "CredentialKind::Application"
        stream = "http_sse" in transports
        binary = "http_binary" in transports
        multipart = "http_multipart" in transports

        lines.append(f"    /// `{method} {path}` — `{op}`.")
        lines.append(f"    pub async fn {fn}(")
        lines.append("        &self,")
        lines.append(f"        request: {req_ty},")
        lines.append(f"    ) -> PlatformResult<{resp_ty}> {{")
        lines.append(f'        let mut path = String::from("{path}");')
        for p in params:
            ident = camel_to_snake(p)
            lines.append(
                f'        path = path.replace("{{{p}}}", '
                f"&crate::openai_platform::url_policy::encode_path_segment(&request.{ident}));"
            )
        lines.append("        let mut query: BTreeMap<String, String> = BTreeMap::new();")
        if method == "GET" and not has_body:
            lines.append(
                '        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }'
            )
            lines.append(
                '        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }'
            )
            lines.append(
                '        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }'
            )
            lines.append(
                '        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }'
            )
            lines.append("        for (k, v) in &request.query {")
            lines.append(
                '            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }'
            )
            lines.append(
                '            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }'
            )
            lines.append("        }")
        if has_body:
            lines.append(
                "        let body = Some(serde_json::to_value(&request.body)"
                ".map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);"
            )
        else:
            lines.append("        let body: Option<Value> = None;")
        lines.append("        let spec = HttpRequestSpec {")
        lines.append(f'            method: "{method}",')
        lines.append("            path,")
        lines.append("            query,")
        lines.append("            body,")
        lines.append(f"            credential: {cred},")
        lines.append(f"            expect_sse: {str(stream).lower()},")
        lines.append(f"            expect_binary: {str(binary).lower()},")
        lines.append(f"            multipart: {str(multipart).lower()},")
        lines.append(f'            operation_id: "{op}",')
        lines.append(
            f"            idempotent: {str(method in ('GET', 'HEAD', 'OPTIONS')).lower()},"
        )
        lines.append("        };")
        lines.append("        let raw = self.transport.execute_json(spec).await?;")
        lines.append(
            "        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))"
        )
        lines.append("    }")
        lines.append("")

        bindings.append(
            {
                "provider": provider,
                "operation_id": op,
                "method": method,
                "path": path,
                "client_method": fn,
                "request_type": req_ty,
                "response_type": resp_ty,
                "client_type": client_type,
                "cli_route": f"{provider}.{fn}",
                "transports": transports,
            }
        )
    lines.append("}")
    lines.append("")
    return "\n".join(lines), bindings


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    openai = load_inventory(ROOT / "baselines/openai/endpoint_inventory.json")
    openrouter = load_inventory(ROOT / "baselines/openrouter/endpoint_inventory.json")

    openai_app = [e for e in openai["endpoints"] if not is_admin_path(e["path"])]
    openai_admin = [e for e in openai["endpoints"] if is_admin_path(e["path"])]

    src_app, bind_app = generate_ops("openai", openai_app, False)
    src_admin, bind_admin = generate_ops("openai_admin", openai_admin, True)
    src_or, bind_or = generate_ops("openrouter", openrouter["endpoints"], False)

    (OUT / "openai_ops.rs").write_text(src_app)
    (OUT / "openai_admin_ops.rs").write_text(src_admin)
    (OUT / "openrouter_ops.rs").write_text(src_or)

    all_bindings = bind_app + bind_admin + bind_or
    bind_lines = [
        "//! Checked-in operation binding inventory (generated).",
        "//! Maps every baseline operation to a typed client method and CLI route.",
        "",
        "/// One implemented binding row.",
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
        "}",
        "",
        "/// Every OpenAI application, OpenAI administration, and OpenRouter-native operation binding.",
        "pub static OPERATION_BINDINGS: &[OperationBinding] = &[",
    ]
    for b in all_bindings:
        bind_lines.extend(
            [
                "    OperationBinding {",
                f'        provider: "{b["provider"]}",',
                f'        operation_id: "{b["operation_id"]}",',
                f'        method: "{b["method"]}",',
                f'        path: "{b["path"]}",',
                f'        client_type: "{b["client_type"]}",',
                f'        client_method: "{b["client_method"]}",',
                f'        request_type: "{b["request_type"]}",',
                f'        response_type: "{b["response_type"]}",',
                f'        cli_route: "{b["cli_route"]}",',
                "    },",
            ]
        )
    bind_lines.append("];")
    bind_lines.append("")
    bind_lines.append(f"pub const OPENAI_APP_BINDING_COUNT: usize = {len(bind_app)};")
    bind_lines.append(f"pub const OPENAI_ADMIN_BINDING_COUNT: usize = {len(bind_admin)};")
    bind_lines.append(f"pub const OPENROUTER_BINDING_COUNT: usize = {len(bind_or)};")
    bind_lines.append(f"pub const TOTAL_BINDING_COUNT: usize = {len(all_bindings)};")
    bind_lines.append("")
    (OUT / "bindings.rs").write_text("\n".join(bind_lines))

    (OUT / "mod.rs").write_text(
        """//! Generated OpenAI / OpenRouter platform operation modules.
pub mod bindings;
pub mod openai_ops;
pub mod openai_admin_ops;
pub mod openrouter_ops;

pub use bindings::*;
"""
    )

    report = {
        "format_version": 1,
        "openai_app": len(bind_app),
        "openai_admin": len(bind_admin),
        "openrouter": len(bind_or),
        "total": len(all_bindings),
        "bindings": all_bindings,
    }
    (ROOT / "baselines" / "operation_bindings_report.json").write_text(
        json.dumps(report, indent=2) + "\n"
    )
    print(
        f"generated openai_app={len(bind_app)} openai_admin={len(bind_admin)} "
        f"openrouter={len(bind_or)} total={len(all_bindings)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
