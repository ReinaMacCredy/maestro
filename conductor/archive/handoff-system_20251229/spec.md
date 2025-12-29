# Specification: HumanLayer-Inspired Handoff System

## Overview

Replace the current LEDGER.md/continuity system with a new handoff system inspired by HumanLayer's approach. The new system provides shareable (git-committed), standalone handoffs with structured metadata, deeply integrated with Conductor workflows.

## Functional Requirements

### FR-1: Handoff Creation (`/create_handoff`)

- **FR-1.1**: Create handoff files in `conductor/handoffs/<track-id>/` or `conductor/handoffs/general/`
- **FR-1.2**: Generate YAML frontmatter with: timestamp (milliseconds), trigger, track_id, bead_id, git_commit, git_branch, author, validation_snapshot
- **FR-1.3**: Include 4 content sections: Context, Changes, Learnings, Next Steps
- **FR-1.4**: Append entry to `index.md` in same directory
- **FR-1.5**: Update `metadata.json.last_activity` timestamp
- **FR-1.6**: Scan for secrets before writing (warn on detection)
- **FR-1.7**: Handle parallel writes via millisecond timestamps + collision suffix

### FR-2: Handoff Resumption (`/resume_handoff`)

- **FR-2.1**: Accept explicit path, track name, or no argument
- **FR-2.2**: Smart discovery: auto-select if 1 track, list if multiple
- **FR-2.3**: Load handoff file completely
- **FR-2.4**: Validate git state (branch match, file existence)
- **FR-2.5**: Flag drift if detected (warn, don't block)
- **FR-2.6**: Present analysis with Context, Changes, Learnings, Next Steps
- **FR-2.7**: Create todo list from Next Steps section
- **FR-2.8**: Warn if handoff is stale (>7 days)

### FR-3: Automatic Triggers

- **FR-3.1**: `design-end` - After `/conductor-newtrack` completes
- **FR-3.2**: `epic-start` - Before each CI epic in `/conductor-implement`
- **FR-3.3**: `epic-end` - After each CI epic closes
- **FR-3.4**: `pre-finish` - At start of `/conductor-finish`
- **FR-3.5**: `manual` - User runs `/create_handoff`
- **FR-3.6**: `idle` - Prompt after 30min inactivity (message-triggered)

### FR-4: Index Management

- **FR-4.1**: Auto-generate `index.md` per track/general
- **FR-4.2**: Append-only (atomic) for parallel safety
- **FR-4.3**: Sort on read for display
- **FR-4.4**: Auto-repair if corrupted (scan directory, rebuild)
- **FR-4.5**: Infer from filename if frontmatter malformed

### FR-5: Archive on Finish

- **FR-5.1**: Move handoff files to `archive/` subdirectory on `/conductor-finish`
- **FR-5.2**: Mark entries in `index.md` as archived
- **FR-5.3**: Keep `index.md` in place for historical reference

### FR-6: Idle Detection

- **FR-6.1**: Track last activity via `conductor/.last_activity` marker file
- **FR-6.2**: Check mtime on next user message
- **FR-6.3**: Prompt if gap > 30 minutes: "Create handoff? [Y/n/skip]"
- **FR-6.4**: Configurable threshold in `conductor/workflow.md`

### FR-7: Secrets Scanning

- **FR-7.1**: Scan for hardcoded patterns (OpenAI, GitHub, AWS keys)
- **FR-7.2**: Support configurable patterns in `conductor/workflow.md`
- **FR-7.3**: Use `gitleaks` if available in PATH
- **FR-7.4**: Warn on detection, ask to proceed or abort

### FR-8: Continuity Deprecation

- **FR-8.1**: Create local `skills/continuity/SKILL.md` stub that redirects to handoff
- **FR-8.2**: Delete `conductor/sessions/` directory
- **FR-8.3**: Delete `skills/conductor/references/ledger/` directory
- **FR-8.4**: Update all references from LEDGER.md to handoff system

## Non-Functional Requirements

### NFR-1: Performance
- Handoff creation: < 2 seconds
- Handoff load: < 1 second
- Index append: Atomic, no locking required

### NFR-2: Reliability
- Parallel-safe via millisecond timestamps
- Auto-repair corrupted index
- Graceful handling of missing git

### NFR-3: Compatibility
- Works with Conductor workflows
- Works standalone (no active track required)
- Git-committed (shareable with team)

### NFR-4: Maintainability
- 4 sections (leaner than HumanLayer's 7)
- Consistent with Maestro skill patterns
- References in `conductor/references/handoff/`

## Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-1 | `/create_handoff` creates file in correct location | Run command, check file exists |
| AC-2 | `/resume_handoff` finds and loads latest handoff | Run command, verify analysis shown |
| AC-3 | Index.md updated on each handoff | Check file after create |
| AC-4 | Secrets scan warns on `sk-test123` | Include in context, verify warning |
| AC-5 | Archive on `/conductor-finish` | Check files moved to `archive/` |
| AC-6 | Idle detection prompts after 30min gap | Wait, send message, verify prompt |
| AC-7 | Old `sessions/` dir deleted | Run `ls conductor/`, verify absent |
| AC-8 | All 6 triggers work at integration points | End-to-end track test |
| AC-9 | Validation snapshot captured in frontmatter | Check handoff file frontmatter |
| AC-10 | Command aliases work (`/conductor-handoff`) | Run alias, verify behavior |

## Out of Scope

- Migration script for old `sessions/` data (manual process if needed)
- Real-time sync (like HumanLayer's `thoughts sync`)
- Integration with external ticketing (Linear, Jira)
- Handoff search/indexing beyond existing `artifact-query.py`
- Cross-repository handoffs

## Dependencies

- `beads` skill for issue tracking integration
- `maestro-core` skill for idle detection placement
- Git available in PATH (graceful fallback if not)

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing sessions/ users | Low (unused) | Document in CHANGELOG |
| Marketplace continuity conflicts | Medium | Local stub overrides |
| Idle detection false positives | Low | Configurable threshold |
| Index corruption | Low | Auto-repair on resume |
