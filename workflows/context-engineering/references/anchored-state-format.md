# Anchored State Format

Session context format for preserving critical information across compaction cycles.

## Version Header

All anchored state files must begin with:

```html
<!-- session-context v1 -->
```

This enables version detection and format migration.

## Format Template

```markdown
<!-- session-context v1 -->

## Intent [PRESERVE]

What we're building and why.

- **Goal**: [Primary objective]
- **Why**: [Motivation/problem being solved]
- **Success criteria**: [How we know we're done]

## Constraints & Ruled-Out [PRESERVE]

What we've explicitly decided NOT to do.

- RULED_OUT: [Approach we ruled out] -- [Why]
- RULED_OUT: [Technology we won't use] -- [Reason]
- CONSTRAINT: [Constraint] -- [Impact]

## Decisions Made (with Why)

Key architectural/design decisions with rationale.

| Decision | Why | Date |
|----------|-----|------|
| [Choice made] | [Reasoning] | YYYY-MM-DD |

## Files Modified

Files touched this session.

- `path/to/file.ext` — [Brief change description]

## Open Questions / TODOs

Things still to address.

- [ ] [Question or TODO]
- [ ] [Another item]

## Current State

Where we are now.

[Describe current progress, blockers, state of implementation]

## Next Steps

What to do next.

1. [Immediate next action]
2. [Following action]
```

## [PRESERVE] Marker Rules

Sections marked with `[PRESERVE]` have special handling:

### Never Compressed

- Content in `[PRESERVE]` sections is never summarized or truncated
- These sections survive all compaction cycles intact
- They contain the "soul" of the session that must persist

### Validation Requirements

Before saving anchored state:

1. **Check for empty PRESERVE sections** — HALT if Intent or Constraints is empty
2. **Validate content exists** — At minimum, each PRESERVE section needs one substantive bullet
3. **Warn on sparse content** — Prompt user if PRESERVE section has fewer than 2 items

### Enforcement

```text
VALID:
## Intent [PRESERVE]
- **Goal**: Implement user authentication with OAuth2

INVALID:
## Intent [PRESERVE]
[Empty or placeholder text]
```

## Section Compression Rules

Non-PRESERVE sections can be summarized when token budget is low.

### Files Modified

- **Keep**: Last 20 entries
- **Compress**: Older entries summarized as "N earlier files modified"
- **Never lose**: Files with uncommitted changes

### Decisions Made

- **Keep**: All decisions (never compress)
- Decisions are permanent artifacts of the design process
- Rationale is essential for future understanding

### Open Questions / TODOs

- **Keep**: All open items
- **Compress**: Completed items can be removed or summarized

### Current State

- **Compress**: Can be summarized to 2-3 sentences when space-constrained
- **Keep**: Current blockers and immediate context

### Next Steps

- **Keep**: Top 3-5 next steps
- **Compress**: Later steps can be generalized

## Example Template (Copy-Paste Ready)

```markdown
<!-- session-context v1 -->

## Intent [PRESERVE]

- **Goal**: 
- **Why**: 
- **Success criteria**: 

## Constraints & Ruled-Out [PRESERVE]

- RULED_OUT: 

## Decisions Made (with Why)

| Decision | Why | Date |
|----------|-----|------|

## Files Modified

- 

## Open Questions / TODOs

- [ ] 

## Current State



## Next Steps

1. 
```

## Usage Notes

1. **Create at session start** — Initialize with Intent and any known Constraints
2. **Update continuously** — Add Decisions and Files as you work
3. **Save before ending** — Ensure all sections are populated
4. **Load on resume** — Parse to restore context in new session
