#!/usr/bin/env python3
"""Generate schema-derived OpenAI and OpenRouter platform bindings.

Run from the repository root. The generator performs no network I/O and reads
exact local copies of the pinned OpenAPI documents described in the baseline
provenance files.
"""
from __future__ import annotations
import hashlib
import json, re
import subprocess
from datetime import date, datetime
from pathlib import Path
from typing import Any

ROOT = Path('crates/codegen/xai-grok-inference')
OUT = ROOT / 'src' / 'openai_platform' / 'generated'
OPENAI_SPEC = Path('/tmp/openai-baseline-pin/openapi.yaml')
OPENAI_SHA256 = 'b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da'
OR_JSON = Path('/tmp/openrouter-baseline-pin/openapi.json')
OPENROUTER_SHA256 = '90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203'
if not OR_JSON.exists():
    OR_JSON = Path('/tmp/openrouter-openapi.json')

def json_default(value: Any) -> Any:
    if isinstance(value, (date, datetime)):
        return value.isoformat()
    raise TypeError(type(value))

def load_openapi(p: Path, expected_sha256: str) -> dict:
    content = p.read_bytes()
    actual_sha256 = hashlib.sha256(content).hexdigest()
    if actual_sha256 != expected_sha256:
        raise ValueError(
            f'pinned schema hash mismatch for {p}: expected {expected_sha256}, got {actual_sha256}'
        )
    if p.suffix in {'.yaml', '.yml'}:
        try:
            import yaml
        except ImportError as error:
            raise SystemExit('PyYAML is required to parse the pinned OpenAI schema') from error
        data = yaml.safe_load(content)
        return json.loads(json.dumps(data, default=json_default))
    return json.loads(content)

def camel_to_snake(name: str) -> str:
    if not name: return 'op'
    s1 = re.sub(r'(.)([A-Z][a-z]+)', r'\1_\2', name)
    s2 = re.sub(r'([a-z0-9])([A-Z])', r'\1_\2', s1)
    s3 = re.sub(r'[^a-zA-Z0-9_]+', '_', s2)
    s3 = re.sub(r'_+', '_', s3).strip('_').lower()
    if not s3: s3='op'
    if s3[0].isdigit(): s3=f'op_{s3}'
    keywords = {'type','match','move','ref','self','super','crate','async','await','dyn','where','as','break','const','continue','else','enum','extern','false','fn','for','if','impl','in','let','loop','mod','mut','pub','return','static','struct','trait','true','unsafe','use','while','try','gen','box'}
    if s3 in keywords: s3=f'{s3}_'
    return s3

def pascal(name: str) -> str:
    parts = re.sub(r'[^a-zA-Z0-9]+','_', name).split('_')
    base = ''.join(p[:1].upper()+p[1:] for p in parts if p)
    if not base: base='Type'
    if base[0].isdigit(): base=f'T{base}'
    return base

def ref_name(ref: str) -> str|None:
    if isinstance(ref,str) and ref.startswith('#/components/schemas/'):
        return ref.split('/')[-1]
    return None

def is_admin_path(path: str) -> bool:
    return path.startswith('/organization') or path.startswith('/dashboard')

def path_params(path: str) -> list[str]:
    return re.findall(r'\{([a-zA-Z0-9_]+)\}', path)

class SchemaGen:
    def __init__(self, openapi: dict, prefix: str):
        self.openapi = openapi
        self.prefix = prefix
        self.schemas = openapi.get('components',{}).get('schemas',{})
        self.emitted: dict[str, str] = {}
        self.pending: list[str] = []
        self.visited: set[str] = set()
        self.type_map: dict[str, str] = {}

    def ensure_schema(self, name: str) -> str:
        if name in self.type_map:
            return self.type_map[name]
        rust = self.prefix + pascal(name)
        base=rust; n=2
        while rust in self.emitted and self.type_map.get(name)!=rust:
            rust=f'{base}{n}'; n+=1
        self.type_map[name]=rust
        if name not in self.visited:
            self.visited.add(name)
            self.pending.append(name)
        return rust

    def resolve_allof(self, schema: dict) -> dict:
        if 'allOf' not in schema:
            return schema
        merged = {'type':'object','properties':{},'required':[]}
        for part in schema['allOf']:
            part = self.deref(part)
            if part.get('type'): merged['type']=part['type']
            for k,v in (part.get('properties') or {}).items():
                merged['properties'][k]=v
            for r in part.get('required') or []:
                if r not in merged['required']:
                    merged['required'].append(r)
            if 'additionalProperties' in part:
                merged['additionalProperties']=part['additionalProperties']
            if 'enum' in part:
                merged['enum']=part['enum']
        for k in ('title','description','nullable','default'):
            if k in schema: merged[k]=schema[k]
        return merged

    def deref(self, schema: Any) -> dict:
        if not isinstance(schema, dict):
            return {}
        if '$ref' in schema:
            name = ref_name(schema['$ref'])
            if name and name in self.schemas:
                return dict(self.schemas[name])
            return {}
        return schema

    def rust_type_for(self, schema: Any, hint: str, depth: int=0) -> str:
        if depth > 14:
            return 'serde_json::Value'
        if schema is None or schema is True:
            return 'serde_json::Value'
        if not isinstance(schema, dict):
            return 'serde_json::Value'
        if '$ref' in schema:
            name = ref_name(schema['$ref'])
            if name:
                return self.ensure_schema(name)
            return 'serde_json::Value'
        schema = self.resolve_allof(schema)
        if 'enum' in schema and schema.get('type','string') in (None,'string') and 'properties' not in schema:
            tname = pascal(hint)+'Enum'
            if tname not in self.emitted:
                self._emit_string_enum(tname, schema['enum'])
            return tname
        if 'oneOf' in schema or 'anyOf' in schema:
            variants = schema.get('oneOf') or schema.get('anyOf')
            non_null = [v for v in variants if not (isinstance(v,dict) and v.get('type')=='null')]
            if len(non_null)==1 and len(variants)==2:
                inner = self.rust_type_for(non_null[0], hint, depth+1)
                return f'Option<{inner}>'
            tname = pascal(hint)+'Union'
            if tname not in self.emitted:
                self._emit_union(tname, variants, hint, depth)
            return tname
        t = schema.get('type')
        if isinstance(t, list):
            non_null=[x for x in t if x!='null']
            if len(non_null)==1 and 'null' in t:
                return f"Option<{self.rust_type_for({**schema,'type':non_null[0]}, hint, depth+1)}>"
            t = non_null[0] if non_null else 'object'
        if t=='string':
            return 'Vec<u8>' if schema.get('format') in ('binary','byte') else 'String'
        if t=='integer':
            return 'i32' if schema.get('format')=='int32' else 'i64'
        if t=='number':
            return 'f64'
        if t=='boolean':
            return 'bool'
        if t=='array':
            items = schema.get('items', {})
            inner = self.rust_type_for(items, hint+'Item', depth+1)
            return f'Vec<{inner}>'
        if t=='object' or 'properties' in schema or schema.get('additionalProperties') is not None:
            props = schema.get('properties') or {}
            if not props:
                ap = schema.get('additionalProperties')
                if isinstance(ap, dict):
                    inner = self.rust_type_for(ap, hint+'Value', depth+1)
                    return f'std::collections::BTreeMap<String, {inner}>'
                return 'std::collections::BTreeMap<String, serde_json::Value>'
            tname = pascal(hint)
            if tname not in self.emitted:
                self._emit_object(tname, schema)
            return tname
        return 'serde_json::Value'

    def _emit_string_enum(self, tname: str, values: list):
        lines=[f'/// Generated string enum `{tname}`.','#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]',f'pub enum {tname} {{']
        seen=set()
        for v in values:
            if not isinstance(v,str): continue
            var = pascal(re.sub(r'[^a-zA-Z0-9]+','_', v)) or 'Value'
            if var[0].isdigit(): var=f'V{var}'
            base=var; i=2
            while var in seen:
                var=f'{base}{i}'; i+=1
            seen.add(var)
            lines.append(f'    #[serde(rename = {json.dumps(v)})]')
            lines.append(f'    {var},')
        # Reserve the catch-all name even when the schema includes the literal
        # string "unknown", which naturally generates an `Unknown` unit variant.
        lines += ['    #[serde(untagged)]','    UnknownValue(String),','}',f'impl Default for {tname} {{ fn default() -> Self {{ Self::UnknownValue(String::new()) }} }}','']
        self.emitted[tname]='\n'.join(lines)

    def _emit_union(self, tname: str, variants: list, hint: str, depth: int):
        lines=[f'/// Generated union `{tname}`.','#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]','#[serde(untagged)]',f'pub enum {tname} {{']
        for i,v in enumerate(variants):
            if isinstance(v,dict) and v.get('type')=='null': continue
            rt = self.rust_type_for(v, f'{hint}V{i}', depth+1)
            lines.append(f'    Variant{i}({rt}),')
        lines += ['    Unknown(serde_json::Value),','}',f'impl Default for {tname} {{ fn default() -> Self {{ Self::Unknown(serde_json::Value::Null) }} }}','']
        self.emitted[tname]='\n'.join(lines)

    def _emit_object(self, tname: str, schema: dict):
        if tname in self.emitted and self.emitted[tname]:
            return
        self.emitted[tname] = ''
        schema = self.resolve_allof(schema)
        props = schema.get('properties') or {}
        required = set(schema.get('required') or [])
        lines=[f'/// Generated object `{tname}`.','#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]',f'pub struct {tname} {{']
        field_names=[]
        for pname, pschema in props.items():
            field = camel_to_snake(pname)
            base=field; n=2
            while field in field_names:
                field=f'{base}_{n}'; n+=1
            field_names.append(field)
            rt = self.rust_type_for(pschema, tname+pascal(pname), 0)
            nullable = False
            if isinstance(pschema, dict):
                if pschema.get('nullable') is True: nullable=True
                t=pschema.get('type')
                if isinstance(t,list) and 'null' in t: nullable=True
            optional = pname not in required or nullable
            if optional and not rt.startswith('Option<'):
                rt = f'Option<{rt}>'
            if pname != field:
                lines.append(f'    #[serde(rename = {json.dumps(pname)})]')
            if optional or rt.startswith('Option<'):
                lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
            lines.append(f'    pub {field}: {rt},')
        lines += ['    #[serde(default, flatten)]','    pub extra: std::collections::BTreeMap<String, serde_json::Value>,','}','']
        self.emitted[tname]='\n'.join(lines)

    def process_pending(self):
        while self.pending:
            name = self.pending.pop(0)
            rust = self.type_map[name]
            if rust in self.emitted and self.emitted[rust]:
                continue
            schema = self.schemas.get(name, {})
            if not isinstance(schema, dict):
                schema = {}
            schema = self.resolve_allof(schema)
            if 'enum' in schema and schema.get('type','string') in (None,'string') and 'properties' not in schema:
                self._emit_string_enum(rust, schema['enum'])
            elif 'oneOf' in schema or 'anyOf' in schema:
                self._emit_union(rust, schema.get('oneOf') or schema.get('anyOf'), name, 0)
            else:
                self._emit_object(rust, schema)

    def types_source(self) -> str:
        self.process_pending()
        sources = [source for source in self.emitted.values() if source]
        parts=['//! Schema-derived types from pinned OpenAPI. DO NOT EDIT BY HAND.','','use serde::{Deserialize, Serialize};']
        if any('SseEvent' in source for source in sources):
            parts.append('use crate::openai_platform::transport::SseEvent;')
        parts.append('')
        parts.extend(self.emitted[name] for name in sorted(self.emitted) if self.emitted[name])
        return '\n'.join(parts)

def content_schema(content: dict) -> tuple[Any|None, list[str]]:
    media = list(content.keys()) if content else []
    for key in content:
        if 'json' in key and 'event' not in key:
            return content[key].get('schema'), media
    for k,v in content.items():
        if 'multipart' in k:
            return v.get('schema'), media
    if content:
        first=next(iter(content.values()))
        return first.get('schema'), media
    return None, media

def classify_transports(req_media, resp_media) -> list[str]:
    t=set()
    for mt in list(req_media)+list(resp_media):
        if 'event-stream' in mt: t.add('http_sse')
        elif 'multipart' in mt: t.add('http_multipart')
        elif any(x in mt for x in ('octet-stream','image/','audio/','video/','application/pdf','application/octet')):
            t.add('http_binary')
        elif 'json' in mt: t.add('http_json')
    if not t: t.add('http_json')
    return sorted(t)

def gen_provider(namespace: str, openapi: dict, ops_path: Path, types_path: Path):
    prefix = 'Or' if namespace=='openrouter' else ''
    gen = SchemaGen(openapi, prefix)
    ops_meta=[]; method_fns=set(); op_sources=[]
    client = {'openai':'OpenAiClient','openai_admin':'OpenAiAdminClient','openrouter':'OpenRouterClient'}[namespace]
    types_mod = {'openai':'openai_types','openai_admin':'openai_admin_types','openrouter':'openrouter_types'}[namespace]

    for path, item in sorted(openapi.get('paths',{}).items()):
        if not isinstance(item, dict): continue
        path_level_params = item.get('parameters') or []
        for method, op in item.items():
            if method not in ('get','post','put','patch','delete','head') or not isinstance(op, dict):
                continue
            if namespace=='openai' and is_admin_path(path): continue
            if namespace=='openai_admin' and not is_admin_path(path): continue
            op_id = op.get('operationId') or f'{method}_{path}'
            op_id_clean = re.sub(r'[^a-zA-Z0-9_]','_', op_id)
            fn = camel_to_snake(op_id_clean)
            base=fn; n=2
            while fn in method_fns:
                fn=f'{base}_{n}'; n+=1
            method_fns.add(fn)

            rb=op.get('requestBody') or {}
            req_content=rb.get('content') or {}
            req_schema, req_media = content_schema(req_content)
            resp_media=[]; resp_schema=None
            for code, resp in (op.get('responses') or {}).items():
                content=(resp or {}).get('content') or {}
                for mt in content: resp_media.append(mt)
                if content and resp_schema is None and str(code).startswith('2'):
                    for mt, cv in content.items():
                        if 'json' in mt and 'event' not in mt:
                            resp_schema = cv.get('schema'); break
                    if resp_schema is None:
                        s,_=content_schema(content); resp_schema=s
            transports = classify_transports(req_media, resp_media)
            if op.get('x-websocket') is True:
                transports = sorted(set(transports)|{'websocket'})

            multipart = 'http_multipart' in transports
            has_sse = 'http_sse' in transports
            has_json = 'http_json' in transports or (not has_sse and not multipart and 'http_binary' not in transports)
            binary = 'http_binary' in transports
            websocket = 'websocket' in transports

            params = list(path_level_params) + list(op.get('parameters') or [])
            query_ps=[p for p in params if isinstance(p,dict) and p.get('in')=='query']

            # ALWAYS Params / Result to avoid schema name collisions.
            req_ty = pascal(op_id_clean)+'Params'
            has_body = req_schema is not None
            body_ty = None
            if has_body:
                if isinstance(req_schema, dict) and '$ref' in req_schema:
                    body_ty = gen.ensure_schema(ref_name(req_schema['$ref']))
                else:
                    body_ty = gen.rust_type_for(req_schema, pascal(op_id_clean)+'Body', 0)

            req_lines=[f'/// Typed params for `{method.upper()} {path}` (`{op_id}`).','#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]',f'pub struct {req_ty} {{']
            for p in path_params(path):
                req_lines.append(f'    pub {camel_to_snake(p)}: String,')
            for qp in query_ps:
                name=qp.get('name','q'); field=camel_to_snake(name)
                qschema=qp.get('schema') or {'type':'string'}
                rt=gen.rust_type_for(qschema, req_ty+pascal(name), 0)
                if not qp.get('required'):
                    if not rt.startswith('Option<'): rt=f'Option<{rt}>'
                    req_lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
                if name!=field:
                    req_lines.append(f'    #[serde(rename = {json.dumps(name)})]')
                req_lines.append(f'    pub {field}: {rt},')
            if has_body:
                req_lines.append(f'    pub body: {body_ty},')
            req_lines += ['}','']
            gen.emitted[req_ty]='\n'.join(req_lines)

            resp_json_ty = pascal(op_id_clean)+'Result'
            if binary and not has_json and not has_sse:
                gen.emitted[resp_json_ty]=f'''/// Binary result for `{op_id}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct {resp_json_ty} {{
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}}
'''
            elif websocket and not has_json and not has_sse:
                resp_json_ty = 'RealtimeSession'
            else:
                if resp_schema is not None:
                    if isinstance(resp_schema, dict) and '$ref' in resp_schema:
                        inner = gen.ensure_schema(ref_name(resp_schema['$ref']))
                    else:
                        inner = gen.rust_type_for(resp_schema, pascal(op_id_clean)+'ResultBody', 0)
                    # Never allow recursive same-name.
                    if inner == resp_json_ty:
                        inner = 'serde_json::Value'
                    if inner == 'String':
                        gen.emitted[resp_json_ty]=f'''/// Text result for `{op_id}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct {resp_json_ty} {{
    pub body: {inner},
}}
'''
                    else:
                        gen.emitted[resp_json_ty]=f'''/// JSON result for `{op_id}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct {resp_json_ty} {{
    #[serde(flatten)]
    pub body: {inner},
}}
'''
                else:
                    gen.emitted[resp_json_ty]=f'''/// JSON result for `{op_id}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct {resp_json_ty} {{
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}}
'''

            resp_sse_ty = pascal(op_id_clean)+'SseResult'
            if has_sse:
                gen.emitted[resp_sse_ty]=f'''/// SSE event stream for `{op_id}` (all frames preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct {resp_sse_ty} {{
    pub events: Vec<SseEvent>,
}}
'''

            admin = namespace=='openai_admin' or is_admin_path(path)
            cred = 'CredentialKind::Admin' if admin else 'CredentialKind::Application'
            m = method.upper()

            def emit_method(fn_name, resp_ty, mode):
                meth=[f'    /// `{m} {path}` — `{op_id}` ({mode}).', f'    /// Transports: {", ".join(transports)}.']
                request_is_used = bool(path_params(path) or query_ps or has_body)
                request_name = 'request' if request_is_used else '_request'
                if mode=='multipart':
                    meth.append(f'    pub async fn {fn_name}(&self, {request_name}: {req_ty}, files: MultipartFiles) -> PlatformResult<{resp_ty}> {{')
                elif mode=='binary':
                    meth.append(f'    pub async fn {fn_name}(&self, {request_name}: {req_ty}, sink: Option<&std::path::Path>) -> PlatformResult<{resp_ty}> {{')
                else:
                    meth.append(f'    pub async fn {fn_name}(&self, {request_name}: {req_ty}) -> PlatformResult<{resp_ty}> {{')
                path_binding = 'mut path' if path_params(path) else 'path'
                meth.append(f'        let {path_binding} = String::from({json.dumps(path)});')
                for p in path_params(path):
                    ident=camel_to_snake(p)
                    meth.append(f'        path = path.replace("{{{p}}}", &crate::openai_platform::url_policy::encode_path_segment(&request.{ident}));')
                query_binding = 'mut query' if query_ps else 'query'
                meth.append(f'        let {query_binding}: BTreeMap<String, String> = BTreeMap::new();')
                for qp in query_ps:
                    name=qp.get('name','q'); field=camel_to_snake(name)
                    if qp.get('required'):
                        meth.append(f'        query.insert({json.dumps(name)}.into(), query_value(&request.{field}));')
                    else:
                        meth.append(f'        if let Some(v) = request.{field}.as_ref() {{ query.insert({json.dumps(name)}.into(), query_value(v)); }}')
                if has_body:
                    meth.append('        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);')
                else:
                    meth.append('        let body: Option<serde_json::Value> = None;')
                meth.append('        let spec = HttpRequestSpec {')
                meth.append(f'            method: {json.dumps(m)}, path, query, body, credential: {cred},')
                meth.append(f'            expect_sse: {str(mode=="sse").lower()}, expect_binary: {str(mode=="binary").lower()}, multipart: {str(mode=="multipart").lower()},')
                meth.append(f'            operation_id: {json.dumps(op_id)}, idempotent: {str(m in ("GET","HEAD")).lower()},')
                meth.append('        };')
                if mode=='multipart':
                    meth.append('        let raw = self.transport.execute_multipart(spec, files).await?;')
                    meth.append(f'        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))')
                elif mode=='binary':
                    meth.append('        let (bytes, content_type) = self.transport.execute_binary(spec, sink).await?;')
                    meth.append(f'        Ok({resp_ty} {{ bytes, content_type }})')
                elif mode=='sse':
                    meth.append('        let events = self.transport.execute_sse(spec).await?;')
                    meth.append(f'        Ok({resp_ty} {{ events }})')
                elif mode=='websocket':
                    meth.append('        self.transport.connect_realtime(spec).await')
                else:
                    meth.append('        let raw = self.transport.execute_json(spec).await?;')
                    meth.append(f'        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))')
                meth.append('    }\n')
                return '\n'.join(meth)

            # Prefer binary when advertised (media/content downloads).
            if multipart:
                primary_mode, primary_resp = 'multipart', resp_json_ty
            elif binary:
                # ensure binary response type exists
                if 'bytes' not in gen.emitted.get(resp_json_ty,''):
                    gen.emitted[resp_json_ty]=f'''/// Binary result for `{op_id}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct {resp_json_ty} {{
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}}
'''
                primary_mode, primary_resp = 'binary', resp_json_ty
            elif has_json:
                primary_mode, primary_resp = 'json', resp_json_ty
            elif has_sse:
                primary_mode, primary_resp = 'sse', resp_sse_ty
            elif websocket:
                primary_mode, primary_resp = 'websocket', resp_json_ty
            else:
                primary_mode, primary_resp = 'json', resp_json_ty

            op_sources.append(emit_method(fn, primary_resp, primary_mode))
            if has_sse and primary_mode != 'sse':
                stream_fn = f'{fn}_stream'
                base=stream_fn; n=2
                while stream_fn in method_fns:
                    stream_fn=f'{base}_{n}'; n+=1
                method_fns.add(stream_fn)
                op_sources.append(emit_method(stream_fn, resp_sse_ty, 'sse'))
                ops_meta.append(meta_row(namespace, op_id+'_stream', m, path, client, stream_fn, req_ty, resp_sse_ty, body_ty, transports=['http_sse'], admin=admin, multipart=False, sse=True, binary=False, ws=False))

            ops_meta.append(meta_row(namespace, op_id, m, path, client, fn, req_ty, primary_resp, body_ty, transports, admin, multipart=(primary_mode=='multipart'), sse=(primary_mode=='sse'), binary=(primary_mode=='binary'), ws=(primary_mode=='websocket')))

    gen.process_pending()
    types_path.write_text(gen.types_source())
    transport_imports = ['CredentialKind', 'HttpRequestSpec']
    if any(meta['is_multipart'] for meta in ops_meta):
        transport_imports.append('MultipartFiles')
    if any(meta['is_websocket'] for meta in ops_meta):
        transport_imports.append('RealtimeSession')
    transport_imports_source = ', '.join(transport_imports)
    ops_path.write_text(f'''//! Generated typed operations for {namespace}.
//! DO NOT EDIT BY HAND.

use super::super::error::{{PlatformError, PlatformResult}};
use super::super::transport::{{{transport_imports_source}}};
use super::{types_mod}::*;
use std::collections::BTreeMap;

fn query_value<T: serde::Serialize + ?Sized>(v: &T) -> String {{
    match serde_json::to_value(v) {{
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) if !other.is_null() => other.to_string(),
        _ => String::new(),
    }}
}}

impl crate::openai_platform::client::{client} {{
''' + '\n'.join(op_sources) + '}\n')
    return ops_meta, gen

def meta_row(namespace, op_id, m, path, client, fn, req_ty, resp_ty, body_ty, transports, admin, multipart, sse, binary, ws):
    return {
        'provider': namespace, 'operation_id': op_id, 'method': m, 'path': path,
        'client_type': client, 'client_method': fn,
        'request_type': req_ty, 'response_type': resp_ty, 'body_type': body_ty,
        'cli_route': f'{namespace}.{fn}', 'transports': transports, 'is_admin': admin,
        'is_deprecated': 'assistants' in path or 'threads' in path,
        'is_multipart': multipart, 'is_sse': sse, 'is_binary': binary, 'is_websocket': ws,
        'typed_request': True, 'typed_response': True,
        'generic_value_body': body_ty == 'serde_json::Value' if body_ty else False,
    }

openai = load_openapi(OPENAI_SPEC, OPENAI_SHA256)
orouter = load_openapi(OR_JSON, OPENROUTER_SHA256)
meta_app, gen_app = gen_provider('openai', openai, OUT/'openai_ops.rs', OUT/'openai_types.rs')
meta_admin, gen_admin = gen_provider('openai_admin', openai, OUT/'openai_admin_ops.rs', OUT/'openai_admin_types.rs')
meta_or, gen_or = gen_provider('openrouter', orouter, OUT/'openrouter_ops.rs', OUT/'openrouter_types.rs')
all_meta = meta_app+meta_admin+meta_or

# write bindings + report + cli catalog + dispatch (reuse previous style quickly)
bind=['//! Operation bindings (generated).','','#[derive(Debug, Clone, Copy, PartialEq, Eq)]','pub struct OperationBinding {']
for f in ['provider','operation_id','method','path','client_type','client_method','request_type','response_type','cli_route']:
    bind.append(f"    pub {f}: &'static str,")
bind += ["    pub transports: &'static [&'static str],",'    pub is_admin: bool,','    pub is_deprecated: bool,','    pub is_multipart: bool,','    pub is_sse: bool,','    pub is_binary: bool,','    pub is_websocket: bool,','    pub typed_request: bool,','    pub typed_response: bool,','    pub generic_value_body: bool,','}','','pub static OPERATION_BINDINGS: &[OperationBinding] = &[']
for b in all_meta:
    transports=', '.join(f'"{t}"' for t in b['transports'])
    bind.append('    OperationBinding {')
    for f in ['provider','operation_id','method','path','client_type','client_method','request_type','response_type','cli_route']:
        bind.append(f'        {f}: {json.dumps(b[f])},')
    bind.append(f'        transports: &[{transports}],')
    for f in ['is_admin','is_deprecated','is_multipart','is_sse','is_binary','is_websocket','typed_request','typed_response','generic_value_body']:
        bind.append(f'        {f}: {str(b[f]).lower()},')
    bind.append('    },')
bind.append('];')
bind.append(f'pub const OPENAI_APP_BINDING_COUNT: usize = {sum(1 for b in all_meta if b["provider"]=="openai")};')
bind.append(f'pub const OPENAI_ADMIN_BINDING_COUNT: usize = {sum(1 for b in all_meta if b["provider"]=="openai_admin")};')
bind.append(f'pub const OPENROUTER_BINDING_COUNT: usize = {sum(1 for b in all_meta if b["provider"]=="openrouter")};')
bind.append(f'pub const TOTAL_BINDING_COUNT: usize = {len(all_meta)};')
(OUT/'bindings.rs').write_text('\n'.join(bind)+'\n')
(OUT/'mod.rs').write_text('//! Generated modules.\npub mod bindings;\npub mod openai_types;\npub mod openai_admin_types;\npub mod openrouter_types;\npub mod openai_ops;\npub mod openai_admin_ops;\npub mod openrouter_ops;\npub use bindings::*;\n')
report={'format_version':4,'total':len(all_meta),'generic_value_body_count':sum(1 for b in all_meta if b['generic_value_body']),'multipart_count':sum(1 for b in all_meta if b['is_multipart']),'sse_count':sum(1 for b in all_meta if b['is_sse']),'binary_count':sum(1 for b in all_meta if b['is_binary']),'websocket_count':sum(1 for b in all_meta if b['is_websocket']),'openai_app':sum(1 for b in all_meta if b['provider']=='openai'),'openai_admin':sum(1 for b in all_meta if b['provider']=='openai_admin'),'openrouter':sum(1 for b in all_meta if b['provider']=='openrouter'),'bindings':all_meta}
(ROOT/'baselines'/'operation_bindings_report.json').write_text(json.dumps(report,indent=2)+'\n')

# CLI catalog
cli=['//! Generated CLI catalog.','','#[derive(Debug, Clone, Copy, PartialEq, Eq)]','pub struct CliOperation {',
"    pub provider_namespace: &'static str,","    pub operation_id: &'static str,","    pub method: &'static str,","    pub path: &'static str,",
"    pub client_method: &'static str,","    pub request_type: &'static str,","    pub response_type: &'static str,","    pub cli_route: &'static str,",
"    pub transports: &'static [&'static str],",'    pub is_admin: bool,','    pub is_deprecated: bool,','    pub is_multipart: bool,','    pub is_sse: bool,','    pub is_binary: bool,','    pub is_websocket: bool,','    pub typed_request: bool,','}','pub static CLI_OPERATIONS: &[CliOperation] = &[']
for b in all_meta:
    transports=', '.join(f'"{t}"' for t in b['transports'])
    cli.append('    CliOperation {')
    cli.append(f'        provider_namespace: {json.dumps(b["provider"])},')
    for f in ['operation_id','method','path','client_method','request_type','response_type','cli_route']:
        cli.append(f'        {f}: {json.dumps(b[f])},')
    cli.append(f'        transports: &[{transports}],')
    for f in ['is_admin','is_deprecated','is_multipart','is_sse','is_binary','is_websocket','typed_request']:
        cli.append(f'        {f}: {str(b[f]).lower()},')
    cli.append('    },')
cli.append('];')
cli.append(f'pub const CLI_OPERATION_COUNT: usize = {len(all_meta)};')
cli.append("pub fn find_cli_operation(namespace: &str, operation_id: &str) -> Option<&'static CliOperation> { CLI_OPERATIONS.iter().find(|op| op.provider_namespace == namespace && op.operation_id == operation_id) }")
cli.append("pub fn operations_for_namespace(namespace: &str) -> impl Iterator<Item = &'static CliOperation> { CLI_OPERATIONS.iter().filter(move |op| op.provider_namespace == namespace) }")
Path('crates/codegen/xai-grok-shell/src/cli/generated_ops.rs').write_text('\n'.join(cli)+'\n')

rust_sources = sorted(OUT.glob('*.rs'))
rust_sources.append(Path('crates/codegen/xai-grok-shell/src/cli/generated_ops.rs'))
subprocess.run(['rustfmt', '--edition', '2024', *map(str, rust_sources)], check=True)

print('total', report['total'], 'generic', report['generic_value_body_count'], 'binary', report['binary_count'], 'sse', report['sse_count'], 'multipart', report['multipart_count'])
# verify CreateEmbeddingResult no recursion
t=Path('crates/codegen/xai-grok-inference/src/openai_platform/generated/openai_types.rs').read_text()
i=t.find('struct CreateEmbeddingResult')
print(t[i:i+300] if i>=0 else 'missing CreateEmbeddingResult')
i=t.find('struct CreateEmbeddingParams')
print(t[i:i+250] if i>=0 else 'missing params')
# recursive check
import re as _re
bad=[]
for m in _re.finditer(r'pub struct (\w+) \{[^}]*pub body: \1,', t, _re.S):
    bad.append(m.group(1))
print('recursive bodies', bad[:10], 'count', len(bad))