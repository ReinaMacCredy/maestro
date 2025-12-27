# Conductor Codemap

The planning and execution methodology.

## Key Files

| File | Responsibility |
|------|----------------|
| `skills/conductor/SKILL.md` | Main conductor skill |
| `skills/conductor/references/commands/conductor-*.md` | Slash command definitions |
| `skills/conductor/references/*.md` | Multi-step workflow implementations |
| `conductor/` | Project-specific context storage |

## Conductor Directory Structure

```
conductor/
├── product.md              # Product vision (created by /conductor-setup)
├── tech-stack.md           # Technology choices
├── workflow.md             # Development standards
├── tracks.md               # Master track list
└── tracks/<track-id>/
    ├── design.md           # From ds / /conductor-design
    ├── spec.md             # From /conductor-newtrack
    ├── plan.md             # From /conductor-newtrack
    └── metadata.json       # Track info + thread IDs + generation + beads

## Command Flow

```
/conductor-setup → creates conductor/ directory with context docs
        ↓
ds (design session) → creates tracks/<id>/design.md
        ↓
/conductor-newtrack → generates spec.md + plan.md + files beads
        ↓
/conductor-implement → executes ONE epic with TDD
        ↓
/conductor-finish → archives track, extracts learnings
```

## State Files

| File | Purpose |
|------|---------|
| `metadata.json` | Track info, thread IDs, generation, and beads state |

## Common Tasks

| Task | Command |
|------|---------|
| Initialize project | `/conductor-setup` |
| Start design | `ds` or `/conductor-design` |
| Create track from design | `/conductor-newtrack` |
| Execute work | `/conductor-implement` |
| Check status | `/conductor-status` |
| Update mid-track | `/conductor-revise` |
| Undo work | `/conductor-revert` |
