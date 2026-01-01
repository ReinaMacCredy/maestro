# Maestro Plugin Quick Reference

> Fast lookup for commands, triggers, and common patterns. ~300 lines.

---

## Commands

| Command | Description | Output |
|---------|-------------|--------|
| `/conductor-setup` | Initialize project context (once per project) | `conductor/product.md`, `tech-stack.md`, `workflow.md` |
| `/conductor-design` | Start Double Diamond design session (alias: `ds`) | `conductor/tracks/<id>/design.md` |
| `/conductor-newtrack` | Create spec + plan + beads from design | `spec.md`, `plan.md`, beads filed |
| `/conductor-implement` | Execute track with TDD (auto-routes if parallel) | Code + tests |
| `/conductor-status` | Display progress overview | Status summary |
| `/conductor-revise` | Update spec/plan mid-implementation | Updated artifacts |
| `/conductor-finish` | Complete track, extract learnings, archive | `LEARNINGS.md`, archived track |
| `/conductor-validate` | Run validation checks on current track | Validation report |
| `/conductor-orchestrate` | Spawn parallel workers for multi-agent execution | Worker coordination |
| `/create_handoff` | Save session context for continuity | `conductor/handoffs/<track>/` |
| `/resume_handoff` | Load prior session context | Restored state |

---

## Triggers

| Trigger | What It Does | Loads Skill |
|---------|--------------|-------------|
| `ds` | Start Double Diamond design session | `design` |
| `fb` | File beads from plan.md tasks | `beads` |
| `rb` | Review beads (status check) | `beads` |
| `tdd` | Enter RED-GREEN-REFACTOR cycle | `test-driven-development` |
| `finish branch` | Complete dev work, merge/PR options | `finishing-a-development-branch` |
| `run parallel` | Trigger multi-agent parallel execution | `orchestrator` |

---

## Skills Reference

| Skill | Triggers | Description |
|-------|----------|-------------|
| **conductor** | `/conductor-*` commands | Context-driven development methodology |
| **design** | `ds`, `/conductor-design` | Double Diamond brainstorming sessions |
| **beads** | `fb`, `rb`, `bd ready` | Issue tracking with dependency graphs |
| **orchestrator** | `/conductor-orchestrate`, `run parallel` | Multi-agent parallel execution |
| **maestro-core** | Auto-loads with any Maestro skill | HALT/DEGRADE policies, routing rules |
| **test-driven-development** | `tdd` | RED-GREEN-REFACTOR cycle enforcement |

---

## bd CLI Quick Reference

> Beads CLI for issue tracking. Always use `--json` for structured output.

### Essential Commands

| Command | Description |
|---------|-------------|
| `bd ready --json` | Find work ready to start (no blockers) |
| `bd show <id>` | Display bead details and context |
| `bd update <id> --status in_progress` | Claim a bead and start work |
| `bd update <id> --notes "message"` | Add notes to a bead |
| `bd close <id> --reason completed` | Close bead (reasons: `completed`, `skipped`, `blocked`) |
| `bd sync` | Sync beads to git |
| `bd list` | List all beads |
| `bd status` | Show ready + in_progress counts |

### Session Pattern

```bash
# Start session
bd ready --json                      # Find work
bd show <id>                         # Read context
bd update <id> --status in_progress  # Claim

# During session
bd update <id> --notes "PROGRESS: ..." # Update notes

# End session
bd update <id> --notes "COMPLETED: X. NEXT: Y"
bd close <id> --reason completed     # Close
bd sync                              # Commit to git
```

---

## bv CLI Quick Reference (Village MCP)

> Multi-agent coordination. **Always use `--robot-*` flags** (bare `bv` hangs).

| Command | Description |
|---------|-------------|
| `bv --robot-status` | Check team state |
| `bv --robot-triage` | Auto-assign ready work to agents |
| `bv init` | Initialize village in `.beads-village/` |
| `bv claim <id>` | Claim bead for current agent |
| `bv done <id>` | Mark bead complete |
| `bv reserve <path>` | Reserve file for editing |
| `bv release <path>` | Release file reservation |
| `bv msg <agent> <text>` | Send message to agent |
| `bv inbox` | Check messages |

---

## Directory Structure

```
conductor/
├── product.md              # Product context
├── tech-stack.md           # Technology choices
├── workflow.md             # Workflow config (idle threshold, etc.)
├── CODEMAPS/               # Architecture documentation
│   ├── overview.md         # High-level architecture
│   └── <module>.md         # Per-module codemaps
├── handoffs/               # Session context (git-committed)
│   └── <track>/
│       ├── index.md        # Handoff summary
│       └── *.md            # Detailed context
├── tracks/                 # Active work
│   └── <track-id>/
│       ├── design.md       # Design decisions
│       ├── spec.md         # Requirements spec
│       ├── plan.md         # Implementation plan
│       └── metadata.json   # State tracking
└── archive/                # Completed tracks

.beads/
├── index.json              # Bead database
├── <id>.md                 # Individual bead files
└── schema.json             # Bead schema

.beads-village/             # Multi-agent state (if using Village)
├── agents/                 # Agent registrations
├── .reservations/          # File reservations
└── .mail/                  # Inter-agent messages

skills/
├── beads/                  # Issue tracking skill
├── conductor/              # Context-driven development
├── design/                 # Double Diamond sessions
├── orchestrator/           # Parallel execution
├── maestro-core/           # Routing policies
└── <other-skills>/         # Additional skills
```

---

## Workflow Phases

| Phase | Command | Input | Output |
|-------|---------|-------|--------|
| **1. Setup** | `/conductor-setup` | — | `product.md`, `tech-stack.md`, `workflow.md` |
| **2. Design** | `ds` or `/conductor-design` | Idea | `design.md` |
| **3. Plan** | `/conductor-newtrack` | `design.md` | `spec.md`, `plan.md`, beads |
| **4. Implement** | `/conductor-implement` | `plan.md`, beads | Code + tests |
| **5. Finish** | `/conductor-finish` | Completed work | `LEARNINGS.md`, archive |

---

## Handoff System

### When to Create Handoffs

| Trigger | When |
|---------|------|
| `design-end` | After design session completes |
| `epic-start` | Before starting each epic |
| `epic-end` | After closing each epic |
| `manual` | Before ending any session |
| `idle` | After 30min inactivity (auto-prompted) |

### Commands

```bash
# Save context
/create_handoff                    # Auto-detect trigger
/create_handoff manual             # Explicit trigger

# Load context
/resume_handoff                    # Load most recent
/resume_handoff <track>            # Load specific track
```

---

## Fallback Policies

| Condition | Action | What Happens |
|-----------|--------|--------------|
| `bd` unavailable | **HALT** | Cannot proceed without beads CLI |
| `conductor/` missing | **DEGRADE** | Standalone mode, no structured workflow |
| Village MCP unavailable | **DEGRADE** | Fall back to single-agent mode |
| Handoff stale (>7 days) | **WARN** | Show warning, suggest fresh start |

---

## Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| `bd` command not found | Beads CLI not installed | Install via command in [SETUP_GUIDE.md](./SETUP_GUIDE.md) |
| `bv` hangs | Missing `--robot-*` flag | Always use `bv --robot-status` not bare `bv` |
| No beads found | Plan not filed | Run `fb` to file beads from plan.md |
| Track validation fails | Missing artifacts | Ensure `design.md`, `spec.md`, `plan.md` exist |
| Handoff not loading | Wrong directory | Check `conductor/handoffs/` exists |
| Orchestrator not triggering | No Track Assignments | Add `## Track Assignments` section to plan.md |
| Tests not running | TDD skipped | Remove `--no-tdd` flag or run `tdd` explicitly |
| File conflicts in MA mode | Reservation missing | Run `bv reserve <path>` before editing |
| Stale bead status | Not synced | Run `bd sync` to update git |
| Session context lost | No handoff created | Always `/create_handoff` before ending session |

---

## Quick Start

```bash
# 1. Initialize project (once)
/conductor-setup

# 2. Design feature
ds
# ... complete Double Diamond session ...

# 3. Create track
/conductor-newtrack

# 4. Find and start work
bd ready --json
bd update <id> --status in_progress

# 5. Implement with TDD
/conductor-implement <track>

# 6. Complete track
/conductor-finish <track>
```

---

## Common Patterns

### Single-Agent Session

```bash
bd ready --json                      # Find work
bd show BEAD-001                     # Read context
bd update BEAD-001 --status in_progress
# ... implement ...
bd close BEAD-001 --reason completed
bd sync
/create_handoff
```

### Multi-Agent Parallel

```bash
/conductor-orchestrate              # Spawn workers
bv --robot-status                   # Check team state
bv reserve src/api/*.ts             # Reserve files
# ... work ...
bv release src/api/*.ts             # Release files
bv done BEAD-001                    # Mark complete
```

### Mid-Session Revision

```bash
/conductor-revise                   # Update spec/plan
# System reopens affected beads automatically
bd ready --json                     # See updated work
```

---

## See Also

- [TUTORIAL.md](TUTORIAL.md) — Full workflow walkthrough
- [AGENTS.md](AGENTS.md) — Project configuration
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — System architecture
- [skills/conductor/references/handoff/](skills/conductor/references/handoff/) — Handoff system details
