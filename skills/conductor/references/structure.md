# Conductor Directory Structure

When you see this structure, the project uses Conductor:

```
conductor/
├── product.md              # Product vision, users, goals
├── product-guidelines.md   # Brand/style guidelines (optional)
├── tech-stack.md           # Technology choices
├── workflow.md             # Development standards (TDD, commits, coverage)
├── tracks.md               # Master track list with status markers
├── setup_state.json        # Setup progress tracking
├── AGENTS.md               # Learnings hub (auto-updated by /conductor-finish)
├── code_styleguides/       # Language-specific style guides
├── archive/                # Archived completed tracks
├── exports/                # Exported summaries
└── tracks/
    └── <track_id>/         # Format: shortname_YYYYMMDD
        ├── metadata.json   # Track type, status, dates
        ├── design.md       # High-level design (created via /conductor-design)
        ├── spec.md         # Requirements and acceptance criteria
        ├── plan.md         # Phased task list with status
        ├── implement_state.json  # Implementation resume state (if in progress)
        ├── blockers.md     # Block history log (if any)
        ├── skipped.md      # Skipped tasks log (if any)
        └── revisions.md    # Revision history log (if any)
```

## Status Markers

Throughout conductor files:

| Marker | Meaning |
|--------|---------|
| `[ ]` | Pending/New |
| `[~]` | In Progress |
| `[x]` | Completed (often followed by 7-char commit SHA) |
| `[!]` | Blocked (followed by reason in brackets) |
| `[-]` | Skipped (followed by reason) |

## State Files Reference

| File | Purpose |
|------|---------|
| `conductor/setup_state.json` | Track setup progress for resume |
| `conductor/product.md` | Product vision, users, goals |
| `conductor/tech-stack.md` | Technology choices |
| `conductor/workflow.md` | Development workflow (TDD, commits) |
| `conductor/tracks.md` | Master track list with status |
| `conductor/tracks/<id>/metadata.json` | Track metadata |
| `conductor/tracks/<id>/design.md` | High-level design |
| `conductor/tracks/<id>/spec.md` | Requirements |
| `conductor/tracks/<id>/plan.md` | Phased task list |
| `conductor/tracks/<id>/implement_state.json` | Implementation resume state |
| `conductor/tracks/<id>/blockers.md` | Block history log |
| `conductor/tracks/<id>/skipped.md` | Skipped tasks log |
| `conductor/tracks/<id>/revisions.md` | Revision history log |
| `conductor/tracks/<id>/finish-state.json` | Finish resume state |
| `conductor/archive/` | Archived completed tracks |
| `conductor/exports/` | Exported summaries |

## Gemini CLI, Claude Code, Amp Code, and Codex Compatibility

Projects set up with Gemini CLI's Conductor extension use identical structure.
The only differences are command syntax:

| Gemini CLI | Claude Code | Amp Code | Codex 
|------------|-------------|----------|-------|
| `/conductor:setup` | `/conductor-setup` |
| `/conductor:design` | `/conductor-design` |
| `/conductor:newTrack` | `/conductor-newtrack` |
| `/conductor:implement` | `/conductor-implement` |
| `/conductor:status` | `/conductor-status` |
| `/conductor:revert` | `/conductor-revert` |
| `/conductor:revise` | `/conductor-revise` |
| `/conductor:finish` | `/conductor-finish` |

Files, workflows, and state management are fully compatible.
