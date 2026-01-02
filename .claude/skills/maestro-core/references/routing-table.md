# Routing Table

Complete trigger → skill mapping.

## Primary Triggers

| Trigger | Skill | Priority | Notes |
|---------|-------|----------|-------|
| `ds` | design | 3 | Also: `/conductor-design` |
| `/conductor-setup` | conductor | 1 | Initialize project |
| `/conductor-newtrack` | conductor | 1 | Create track from design |
| `ci`, `/conductor-implement` | conductor | 1 | May route to orchestrator |
| `co`, `/conductor-orchestrate` | orchestrator | 2 | Parallel execution |
| `/conductor-status` | conductor | 1 | Show progress |
| `/conductor-finish` | conductor | 1 | Complete track |
| `/conductor-revise` | conductor | 1 | Update spec/plan |
| `/conductor-revert` | conductor | 1 | Rollback changes |
| `fb`, `file-beads` | beads | 4 | File from plan |
| `rb`, `review-beads` | beads | 4 | Review beads |
| `bd *` | beads | 4 | All bd commands |
| `/create_handoff` | conductor | 1 | Manual handoff |
| `/resume_handoff` | conductor | 1 | Load prior context |

## Phrase Triggers

| Phrase | Skill | Example |
|--------|-------|---------|
| "design a feature" | design | "Let's design a login feature" |
| "run parallel" | orchestrator | "Run these tasks in parallel" |
| "spawn workers" | orchestrator | "Spawn workers for Track A" |
| "create task" | beads | "Create task for fixing auth" |
| "what's ready" | beads | "What's ready to work on?" |
| "what's blocking" | beads | "What's blocking task X?" |

## Routing Precedence

1. Explicit command (`/conductor-*`, `bd *`)
2. Short alias (`ds`, `ci`, `fb`)
3. Phrase match
4. Context detection (conductor/ exists)

## Conditional Routing

### `/conductor-implement` Auto-Route

1. Check `metadata.json.beads.orchestrated`
2. If `true` → continue sequential
3. Else check plan.md for `## Track Assignments`
4. If found → route to orchestrator
5. Else → continue sequential
