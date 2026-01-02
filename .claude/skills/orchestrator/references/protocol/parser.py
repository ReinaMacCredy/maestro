"""Message parser for orchestrator protocol.

Handles YAML frontmatter parsing and message construction.
"""

from __future__ import annotations

import re
from typing import Any

import yaml

FRONTMATTER_PATTERN = re.compile(
    r"^---\s*\n(.*?)\n---\s*\n(.*)$",
    re.DOTALL,
)


def parse_message(body_md: str) -> dict[str, Any]:
    """Parse a message body extracting YAML frontmatter.

    Args:
        body_md: Raw markdown message body with optional YAML frontmatter.

    Returns:
        Dictionary with 'meta' (parsed frontmatter) and 'content' (remaining body).
        If no frontmatter, meta contains {'type': 'UNKNOWN'}.
    """
    if not body_md:
        return {"meta": {"type": "UNKNOWN"}, "content": ""}

    body_md = body_md.strip()
    match = FRONTMATTER_PATTERN.match(body_md)

    if not match:
        return {"meta": {"type": "UNKNOWN"}, "content": body_md}

    yaml_str, content = match.groups()

    try:
        meta = yaml.safe_load(yaml_str)
        if not isinstance(meta, dict):
            meta = {"type": "UNKNOWN"}
    except yaml.YAMLError:
        meta = {"type": "UNKNOWN"}

    if "type" not in meta:
        meta["type"] = "UNKNOWN"

    return {"meta": meta, "content": content.strip()}


def build_message(
    msg_type: str,
    content: str,
    **fields: Any,
) -> str:
    """Build a formatted message with YAML frontmatter.

    Args:
        msg_type: Message type (e.g., 'ASSIGN', 'PROGRESS', 'COMPLETED').
        content: Message body content.
        **fields: Additional frontmatter fields.

    Returns:
        Formatted message with YAML frontmatter.
    """
    meta = {"type": msg_type, **fields}

    yaml_str = yaml.safe_dump(meta, default_flow_style=False, sort_keys=False)

    return f"---\n{yaml_str}---\n\n{content}"
