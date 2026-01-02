# Handoff Template

Reference template for handoff files created by `/create_handoff`.

## Frontmatter Schema

```yaml
---
timestamp: 2025-12-29T10:00:00.123+07:00  # ISO 8601 with milliseconds
trigger: design-end | epic-start | epic-end | pre-finish | manual | idle
track_id: auth-system | general            # Track ID or "general" for non-track work
bead_id: E1-user-login                     # Only for epic triggers
git_commit: abc123f                        # Short SHA (7 chars) or "unknown"
git_branch: feat/auth-system               # Current branch or "unknown"
author: agent | human                      # Who created the handoff
validation_snapshot:                       # Captured from metadata.json
  gates_passed: [design, spec, plan-structure]
  current_gate: plan-execution
---
```

### Field Definitions

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `timestamp` | Yes | ISO 8601 | Creation time with millisecond precision |
| `trigger` | Yes | enum | One of 6 trigger types (see triggers.md) |
| `track_id` | Yes | string | Track ID or "general" for non-track work |
| `bead_id` | No | string | Epic/bead ID, only for epic-start/epic-end |
| `git_commit` | Yes | string | 7-char SHA or "unknown" if git unavailable |
| `git_branch` | Yes | string | Branch name or "unknown" if git unavailable |
| `author` | Yes | enum | "agent" or "human" |
| `validation_snapshot` | No | object | Validation gate state at handoff time |

## Content Template

```markdown
---
timestamp: {{TIMESTAMP}}
trigger: {{TRIGGER}}
track_id: {{TRACK_ID}}
bead_id: {{BEAD_ID}}
git_commit: {{GIT_COMMIT}}
git_branch: {{GIT_BRANCH}}
author: agent
validation_snapshot:
  gates_passed: {{GATES_PASSED}}
  current_gate: {{CURRENT_GATE}}
---

# Handoff: {{TRACK_ID}} | {{TRIGGER}}

## Context

{What you were working on, current state, active decisions}

- Track: {{TRACK_ID}}
- Phase: {{CURRENT_PHASE}}
- Decisions made: 
  - Decision 1
  - Decision 2

## Changes

{Files modified with line references}

- `path/to/file.ts:10-45` - Added login handler
- `path/to/other.ts:100-120` - Updated validation

## Learnings

{Patterns discovered, gotchas, important context for resuming agent}

- Learning 1: Description
- Gotcha: Something to watch out for

## Next Steps

{Immediate actions for resuming agent - converted to todo list on resume}

1. [ ] First task to complete
2. [ ] Second task to complete
3. [ ] Verification step
```

## File Naming Convention

```
YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
```

Examples:
- `2025-12-29_10-00-00-123_auth-system_design-end.md`
- `2025-12-29_11-30-00-456_auth-system_E1_epic-start.md`
- `2025-12-29_14-15-00-789_general_manual.md`

### Collision Handling

If filename already exists, append suffix:
- `..._design-end-1.md`
- `..._design-end-2.md`

## Section Guidelines

### Context Section

**Purpose:** Orient the resuming agent quickly.

Include:
- What track/feature is being worked on
- Current phase (design, implementation, etc.)
- Active decisions or constraints
- Any open questions

**Length:** 3-8 lines

### Changes Section

**Purpose:** Show what files were touched.

Format:
- Use file paths with line numbers when relevant
- Brief description of what changed
- Group related changes together

**Length:** 2-15 lines (vary by scope)

### Learnings Section

**Purpose:** Capture context that would otherwise be lost.

Include:
- Patterns discovered during work
- Gotchas or edge cases found
- Important context for understanding decisions
- Things that didn't work (and why)

**Length:** 2-10 lines

### Next Steps Section

**Purpose:** Provide immediate action items.

Format:
- Numbered list with checkbox syntax `[ ]`
- Actionable, specific tasks
- Order by priority/sequence
- Last item should be verification if applicable

**Length:** 3-7 items

## Comparison with HumanLayer

| Aspect | HumanLayer (7 sections) | Our Template (4 sections) |
|--------|-------------------------|---------------------------|
| Background | ✅ | Merged into Context |
| Current State | ✅ | Merged into Context |
| Key Technical Decisions | ✅ | Merged into Learnings |
| Important Code Patterns | ✅ | Merged into Learnings |
| Known Issues | ✅ | Merged into Learnings |
| Next Steps | ✅ | ✅ |
| Open Questions | ✅ | Merged into Context |

**Rationale:** Beads handles task tracking, so we can have a leaner template.
