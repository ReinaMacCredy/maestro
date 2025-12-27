"""Shared utilities for artifact scripts."""

import re
from pathlib import Path
from typing import Optional

import yaml


def parse_frontmatter(content: str) -> dict:
    """Parse YAML frontmatter from markdown content."""
    try:
        match = re.match(r"^---\s*\n(.*?)\n---\s*\n", content, re.DOTALL)
        if match:
            return yaml.safe_load(match.group(1)) or {}
    except (yaml.YAMLError, ValueError):
        pass
    return {}


def find_conductor_root() -> Optional[Path]:
    """Find conductor/ directory by walking up from cwd."""
    current = Path.cwd()
    while current != current.parent:
        conductor = current / "conductor"
        if conductor.is_dir():
            return conductor
        current = current.parent
    return None


def get_db_path(conductor_root: Path, ensure_cache: bool = False) -> Path:
    """Get path to SQLite database.
    
    Args:
        conductor_root: Path to conductor directory
        ensure_cache: If True, create .cache directory if it doesn't exist
    """
    cache_dir = conductor_root / ".cache"
    if ensure_cache:
        cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir / "artifact-index.db"
