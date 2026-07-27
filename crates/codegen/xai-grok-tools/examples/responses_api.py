#!/usr/bin/env python3
"""Send a standard model request to the xAI Responses API, without tools."""

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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("prompt", help="Prompt sent to the model")
    parser.add_argument(
        "--base-url",
        default=os.environ.get("GROK_XAI_API_BASE_URL", DEFAULT_BASE_URL),
    )
    parser.add_argument(
        "--model",
        default=os.environ.get("GROK_MODEL", DEFAULT_MODEL),
    )
    parser.add_argument("--max-output-tokens", type=int, default=256)
    parser.add_argument("--raw", action="store_true", help="Print the full JSON response")
    return parser.parse_args()


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "model": args.model,
        "input": args.prompt,
        "store": False,
        "max_output_tokens": args.max_output_tokens,
    }


def call_responses_api(args: argparse.Namespace, api_key: str) -> dict[str, Any]:
    request = urllib.request.Request(
        f"{args.base_url.rstrip('/')}/responses",
        data=json.dumps(build_payload(args)).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
            "User-Agent": "xai-grok-build/responses-api-example",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"API returned HTTP {error.code}: {body}") from error
    except urllib.error.URLError as error:
        raise RuntimeError(f"Request failed: {error.reason}") from error


def extract_output_text(response: dict[str, Any]) -> str:
    texts: list[str] = []
    for item in response.get("output", []):
        if not isinstance(item, dict) or item.get("type") != "message":
            continue
        for content in item.get("content", []):
            if isinstance(content, dict) and content.get("type") == "output_text":
                if text := content.get("text"):
                    texts.append(text)
    return "\n".join(texts)


def main() -> int:
    args = parse_args()
    api_key = os.environ.get("XAI_API_KEY", "").strip()
    if not api_key:
        print("error: XAI_API_KEY is not set", file=sys.stderr)
        return 2

    try:
        response = call_responses_api(args, api_key)
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if args.raw:
        print(json.dumps(response, indent=2, ensure_ascii=False))
    else:
        print(extract_output_text(response) or "No output text returned.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
