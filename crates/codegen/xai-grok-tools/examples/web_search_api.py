#!/usr/bin/env python3
"""Call xAI's Responses API with the hosted web_search tool."""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from typing import Any

DEFAULT_BASE_URL = "https://api.x.ai/v1"
DEFAULT_MODEL = "grok-4.20-multi-agent"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run a web search through the xAI Responses API."
    )
    parser.add_argument("query", help="Search query")
    parser.add_argument(
        "--allowed-domain",
        action="append",
        dest="allowed_domains",
        help="Restrict results to a domain; repeat for multiple domains",
    )
    parser.add_argument(
        "--base-url",
        default=os.environ.get("GROK_XAI_API_BASE_URL", DEFAULT_BASE_URL),
        help="API base URL (default: %(default)s)",
    )
    parser.add_argument(
        "--model",
        default=os.environ.get("GROK_WEB_SEARCH_MODEL", DEFAULT_MODEL),
        help="Web-search model (default: %(default)s)",
    )
    parser.add_argument(
        "--max-output-tokens",
        type=int,
        default=8192,
        help="Maximum output tokens (default: %(default)s)",
    )
    parser.add_argument("--raw", action="store_true", help="Print the complete JSON response")
    return parser.parse_args()


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    web_search: dict[str, Any] = {"type": "web_search"}
    if args.allowed_domains:
        web_search["filters"] = {"allowed_domains": args.allowed_domains}

    return {
        "model": args.model,
        "input": args.query,
        "tools": [web_search],
        "store": False,
        "temperature": 0.1,
        "top_p": 0.95,
        "max_output_tokens": args.max_output_tokens,
    }


def request_search(args: argparse.Namespace, api_key: str) -> dict[str, Any]:
    endpoint = f"{args.base_url.rstrip('/')}/responses"
    request = urllib.request.Request(
        endpoint,
        data=json.dumps(build_payload(args)).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
            "User-Agent": "xai-grok-build/web-search-api-example",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        try:
            details = json.dumps(json.loads(body), indent=2, ensure_ascii=False)
        except json.JSONDecodeError:
            details = body
        raise RuntimeError(f"API returned HTTP {error.code}:\n{details}") from error
    except urllib.error.URLError as error:
        raise RuntimeError(f"Request failed: {error.reason}") from error


def extract_result(response: dict[str, Any]) -> tuple[str, list[str]]:
    texts: list[str] = []
    citations: list[str] = []

    for output in response.get("output", []):
        if not isinstance(output, dict):
            continue
        for content in output.get("content", []):
            if not isinstance(content, dict) or content.get("type") != "output_text":
                continue
            if text := content.get("text"):
                texts.append(text)
            for annotation in content.get("annotations", []):
                if isinstance(annotation, dict) and annotation.get("type") == "url_citation":
                    if url := annotation.get("url"):
                        if url not in citations:
                            citations.append(url)

    return "\n".join(texts), citations


def main() -> int:
    args = parse_args()
    api_key = os.environ.get("XAI_API_KEY", "").strip()
    if not api_key:
        print("error: XAI_API_KEY is not set", file=sys.stderr)
        return 2

    try:
        response = request_search(args, api_key)
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if args.raw:
        print(json.dumps(response, indent=2, ensure_ascii=False))
        return 0

    text, citations = extract_result(response)
    print(text or "No search results found.")
    if citations:
        print("\nCitations:")
        for index, url in enumerate(citations, start=1):
            print(f"{index}. {url}")

    usage = response.get("usage")
    if usage:
        print("\nUsage:")
        print(json.dumps(usage, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
