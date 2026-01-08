# Routing Table

Complete trigger → skill mapping.

## Primary Triggers

| Trigger | Skill | Priority | Notes |
|---------|-------|----------|-------|
| `ds` | designing | 3 | Also: `/conductor-design` |
| `cn` | designing | 3 | Also: `/conductor-newtrack` |
| `/conductor-setup` | conductor | 1 | Initialize project |
| `/conductor-newtrack` | designing | 3 | Create track from design |
| `/conductor-design` | designing | 3 | Start design session |
| `ci`, `/conductor-implement` | conductor | 1 | Implementation only |
| `co`, `/conductor-orchestrate` | orchestrator | 2 | Parallel execution |
| `/conductor-status` | conductor | 1 | Show progress |
| `/conductor-finish` | handoff | 5 | Complete track |
| `/conductor-revise` | conductor | 1 | Update spec/plan |
| `/conductor-revert` | conductor | 1 | Rollback changes |
| `/conductor-handoff` | handoff | 5 | Session cycling |
| `fb`, `file-beads` | tracking | 4 | File from plan |
| `rb`, `review-beads` | tracking | 4 | Review beads |
| `bd *` | tracking | 4 | All bd commands |
| `ho` | handoff | 5 | Session handoff |
| `/create_handoff` | handoff | 5 | Manual handoff |
| `/resume_handoff` | handoff | 5 | Load prior context |

## Phrase Triggers

| Phrase | Skill | Example |
|--------|-------|---------|
| "design a feature" | designing | "Let's design a login feature" |
| "explore", "brainstorm" | designing | "Let's brainstorm the auth flow" |
| "run parallel" | orchestrator | "Run these tasks in parallel" |
| "spawn workers" | orchestrator | "Spawn workers for Track A" |
| "create task" | tracking | "Create task for fixing auth" |
| "what's ready" | tracking | "What's ready to work on?" |
| "what's blocking" | tracking | "What's blocking task X?" |
| "create skill", "write skill" | creating-skills | "Create a new skill for X" |
| "hand off", "session cycling" | handoff | "Hand off to next session" |

## Ownership Matrix

| Skill | Owns | Does NOT Own |
|-------|------|--------------|
| designing | Phases 1-8, design.md, spec.md, plan.md generation | Implementation |
| conductor | `ci` implementation execution | Design sessions, track creation |
| orchestrator | Parallel worker coordination | Sequential implementation |
| tracking | Bead CRUD, dependency graphs | Workflow routing |
| handoff | Session cycling, context preservation | Active work |
| creating-skills | Skill authoring, validation | Skill deployment |

## Routing Precedence

1. Explicit command (`/conductor-*`, `bd *`)
2. Short alias (`ds`, `cn`, `ci`, `fb`)
3. Phrase match
4. Context detection (conductor/ exists)

## Conditional Routing

### Design (`ds`) vs Implementation (`ci`)

| Aspect | `ds`/`cn` (Designing) | `ci` (Conductor) |
|--------|----------------------|------------------|
| Intent | Exploratory → actionable plan | Execute tasks |
| Output | design.md → spec.md → plan.md | Code changes |
| Process | Double Diamond + 6-phase pipeline | TDD implementation |
| When | New features, unclear scope | Ready to implement |
| Skill | designing | conductor |

### `/conductor-implement` Auto-Route

1. Check `metadata.json.beads.orchestrated`
2. If `true` → continue sequential
3. Else check plan.md for `## Track Assignments`
4. If found → route to orchestrator
5. Else → continue sequential
