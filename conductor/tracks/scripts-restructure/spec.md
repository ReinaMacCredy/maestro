# Spec: Scripts Restructure

## Overview

Restructure Python scripts to match claudekit-skills pattern, relocating scripts to skill-specific `scripts/` directories with self-contained code and standardized CLI interfaces.

## Requirements

### Functional Requirements

1. **FR-1: Relocate Conductor Scripts**
   - Move `scripts/artifact-query.py` → `skills/conductor/scripts/artifact_query.py`
   - Move `scripts/artifact-index.py` → `skills/conductor/scripts/artifact_index.py`
   - Move `scripts/artifact-cleanup.py` → `skills/conductor/scripts/artifact_cleanup.py`

2. **FR-2: Inline Shared Library**
   - Each script must be self-contained (no shared lib imports)
   - Inline `find_conductor_root()`, `get_db_path()`, `parse_frontmatter()` into each script

3. **FR-3: Add JSON Output**
   - All scripts must support `--json` flag for structured output
   - JSON output must be valid and parseable

4. **FR-4: Extract Beads Algorithm**
   - Extract `generate_track_assignments()` from `beads/references/auto-orchestrate.md`
   - Create `skills/beads/scripts/track_assigner.py` with CLI interface

5. **FR-5: Update Documentation**
   - Update `conductor/AGENTS.md` with new script paths
   - Update docstrings in each script with new paths

6. **FR-6: Cleanup Old Files**
   - Delete `scripts/artifact-query.py`
   - Delete `scripts/artifact-index.py`
   - Delete `scripts/artifact-cleanup.py`
   - Delete `scripts/lib/` directory

### Non-Functional Requirements

1. **NFR-1: Backward Compatibility**
   - Scripts must work when run from any directory in project
   - `find_conductor_root()` behavior unchanged

2. **NFR-2: Convention Compliance**
   - Follow claudekit-skills pattern (argparse, JSON I/O, docstrings)
   - Use underscore naming for Python files (`artifact_query.py` not `artifact-query.py`)

## Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-1 | Query script works | `uv run skills/conductor/scripts/artifact_query.py test` |
| AC-2 | JSON output valid | `uv run skills/conductor/scripts/artifact_query.py test --json \| jq .` |
| AC-3 | Index script works | `uv run skills/conductor/scripts/artifact_index.py --verify` |
| AC-4 | Cleanup script works | `uv run skills/conductor/scripts/artifact_cleanup.py --dry-run` |
| AC-5 | Track assigner works | `echo '[{"id":"t1","type":"task","ready":true,"blocked_by":[]}]' > /tmp/test.json && uv run skills/beads/scripts/track_assigner.py /tmp/test.json --json` |
| AC-6 | AGENTS.md updated | `grep "skills/conductor/scripts" conductor/AGENTS.md` |
| AC-7 | Old scripts deleted | `! test -f scripts/artifact-query.py` |
| AC-8 | Lib deleted | `! test -d scripts/lib` |

## Out of Scope

- orchestrator skill scripts
- design skill (no scripts needed)
- Shell scripts (remain at project root)
- Global `~/.config/amp/AGENTS.md` (manual update)

## Dependencies

- None (self-contained work)

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Scripts break after move | High | Test each before deleting old |
| Global AGENTS.md outdated | Low | Document for manual update |
