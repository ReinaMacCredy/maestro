# Design: Scripts Restructure

## Problem Statement

Restructure the my-workflow skills repository to match claudekit-skills conventions by relocating existing Python scripts to skill-specific `scripts/` directories and extracting the beads auto-orchestrate algorithm into a runnable CLI utility.

## Goals

1. Match claudekit-skills pattern: `SKILL.md` + `references/` + `scripts/`
2. Self-contained scripts (no shared lib imports)
3. CLI scaffolding with argparse + JSON output
4. Clean separation of documentation vs executable code

## Scope

### In Scope
- Move `scripts/*.py` → `skills/conductor/scripts/`
- Extract `auto-orchestrate.md` algorithm → `skills/beads/scripts/`
- Inline shared `lib/` utils into each script
- Update `conductor/AGENTS.md` references
- Add `--json` flag to all scripts

### Out of Scope
- orchestrator skill (separate session)
- design skill (no code to extract)
- Shell scripts (remain at project root)
- Global ~/.config/amp/AGENTS.md (manual update later)

## Architecture

```
my-workflow/
├── scripts/                    # Shell scripts only
│   ├── beads-metrics-summary.sh
│   ├── install-*.sh
│   └── validate-*.sh
│
├── skills/
│   ├── beads/
│   │   ├── SKILL.md
│   │   ├── references/
│   │   └── scripts/           # NEW
│   │       └── track_assigner.py
│   │
│   └── conductor/
│       ├── SKILL.md
│       ├── references/
│       └── scripts/           # NEW
│           ├── artifact_query.py
│           ├── artifact_index.py
│           └── artifact_cleanup.py
```

## Implementation Details

### Scripts to Move (conductor)

| Original | Target | Changes |
|----------|--------|---------|
| `scripts/artifact-query.py` | `skills/conductor/scripts/artifact_query.py` | Inline lib, add `--json` |
| `scripts/artifact-index.py` | `skills/conductor/scripts/artifact_index.py` | Inline lib, add `--json` |
| `scripts/artifact-cleanup.py` | `skills/conductor/scripts/artifact_cleanup.py` | Inline lib, add `--json` |

### Script to Extract (beads)

| Source | Target | Description |
|--------|--------|-------------|
| `beads/references/auto-orchestrate.md` (L91-129) | `skills/beads/scripts/track_assigner.py` | Track assignment algorithm |

### Lib Functions to Inline

```python
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
    cache_dir = conductor_root / ".cache"
    if ensure_cache:
        cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir / "artifact-index.db"

def parse_frontmatter(content: str) -> dict:
    try:
        match = re.match(r"^---\s*\n(.*?)\n---\s*\n", content, re.DOTALL)
        if match:
            return yaml.safe_load(match.group(1)) or {}
    except (yaml.YAMLError, ValueError):
        pass
    return {}
```

### CLI Pattern

All scripts follow this pattern:

```python
#!/usr/bin/env python3
# /// script
# dependencies = ["pyyaml"]
# ///
"""
Script Name - Brief description.

Usage:
    uv run skills/conductor/scripts/script_name.py <args>
    uv run skills/conductor/scripts/script_name.py <args> --json
"""
import argparse
import json

def main():
    parser = argparse.ArgumentParser(description="...")
    parser.add_argument("--json", action="store_true", help="JSON output")
    args = parser.parse_args()
    
    result = do_work(args)
    
    if args.json:
        print(json.dumps(result))
    else:
        print_human_readable(result)
```

## Documentation Updates

### conductor/AGENTS.md

```diff
- - `uv run scripts/artifact-index.py` - Build/rebuild SQLite FTS5 index
- - `uv run scripts/artifact-query.py <query>` - Search archived handoffs
- - `uv run scripts/artifact-cleanup.py --dry-run` - Preview cleanup
+ - `uv run skills/conductor/scripts/artifact_index.py` - Build/rebuild index
+ - `uv run skills/conductor/scripts/artifact_query.py <query>` - Search handoffs
+ - `uv run skills/conductor/scripts/artifact_query.py <query> --json` - JSON output
+ - `uv run skills/conductor/scripts/artifact_cleanup.py --dry-run` - Preview cleanup
```

## Acceptance Criteria

- [ ] `uv run skills/conductor/scripts/artifact_query.py test` works
- [ ] `uv run skills/conductor/scripts/artifact_query.py test --json` outputs valid JSON
- [ ] `uv run skills/conductor/scripts/artifact_index.py --verify` works
- [ ] `uv run skills/beads/scripts/track_assigner.py sample.json --json` works
- [ ] `conductor/AGENTS.md` references new paths
- [ ] Old `scripts/*.py` and `scripts/lib/` deleted
- [ ] Shell scripts remain at project root

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Scripts break after move | High | Test each before deleting old |
| Global AGENTS.md outdated | Low | Document for manual update |
| Missing references | Medium | Audited - only conductor/AGENTS.md |

## Post-Implementation

- Update `~/.config/amp/AGENTS.md` line 163 (manual)
- Consider adding beads/scripts/README.md
