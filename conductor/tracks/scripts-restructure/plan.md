# Plan: Scripts Restructure

## Epic 1: Conductor Scripts Migration

### 1.1 Create conductor scripts directory
- [ ] Create `skills/conductor/scripts/` directory

### 1.2 Migrate artifact_query.py
- [ ] Create `skills/conductor/scripts/artifact_query.py`
- [ ] Inline `find_conductor_root()` function
- [ ] Inline `get_db_path()` function
- [ ] Add `--json` flag support
- [ ] Update docstring with new path
- [ ] Test: `uv run skills/conductor/scripts/artifact_query.py test`
- [ ] Test: `uv run skills/conductor/scripts/artifact_query.py test --json | jq .`

### 1.3 Migrate artifact_index.py
- [ ] Create `skills/conductor/scripts/artifact_index.py`
- [ ] Inline `find_conductor_root()` function
- [ ] Inline `get_db_path()` function
- [ ] Inline `parse_frontmatter()` function
- [ ] Add `--json` flag support
- [ ] Update docstring with new path
- [ ] Test: `uv run skills/conductor/scripts/artifact_index.py --verify`

### 1.4 Migrate artifact_cleanup.py
- [ ] Create `skills/conductor/scripts/artifact_cleanup.py`
- [ ] Inline `find_conductor_root()` function
- [ ] Inline `get_db_path()` function
- [ ] Inline `parse_frontmatter()` function
- [ ] Add `--json` flag support
- [ ] Update docstring with new path
- [ ] Test: `uv run skills/conductor/scripts/artifact_cleanup.py --dry-run`

## Epic 2: Beads Script Extraction

### 2.1 Create beads scripts directory
- [ ] Create `skills/beads/scripts/` directory

### 2.2 Extract track_assigner.py
- [ ] Create `skills/beads/scripts/track_assigner.py`
- [ ] Extract `generate_track_assignments()` from `beads/references/auto-orchestrate.md`
- [ ] Extract `merge_smallest_two_tracks()` helper
- [ ] Add argparse CLI with `--max-workers` and `--json` flags
- [ ] Add docstring with usage examples
- [ ] Test: `echo '[{"id":"t1","type":"task","ready":true,"blocked_by":[]}]' > /tmp/test.json && uv run skills/beads/scripts/track_assigner.py /tmp/test.json --json`

## Epic 3: Documentation & Cleanup

### 3.1 Update conductor/AGENTS.md
- [ ] Update script paths from `scripts/` to `skills/conductor/scripts/`
- [ ] Add `--json` flag examples
- [ ] Verify: `grep "skills/conductor/scripts" conductor/AGENTS.md`

### 3.2 Delete old files
- [ ] Delete `scripts/artifact-query.py`
- [ ] Delete `scripts/artifact-index.py`
- [ ] Delete `scripts/artifact-cleanup.py`
- [ ] Delete `scripts/lib/` directory
- [ ] Verify: `! test -f scripts/artifact-query.py`
- [ ] Verify: `! test -d scripts/lib`

## Verification

Final verification after all epics complete:

```bash
# All scripts work
uv run skills/conductor/scripts/artifact_query.py test
uv run skills/conductor/scripts/artifact_index.py --verify
uv run skills/conductor/scripts/artifact_cleanup.py --dry-run
uv run skills/beads/scripts/track_assigner.py /tmp/test.json --json

# JSON output valid
uv run skills/conductor/scripts/artifact_query.py test --json | jq .

# Old files gone
! test -f scripts/artifact-query.py
! test -d scripts/lib
```
