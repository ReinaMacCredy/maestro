# Maestro Reference

Quick-lookup for commands, triggers, and technical details.

---

## Commands

| Command | Alias | Description | Phase |
|---------|-------|-------------|-------|
| `/conductor-setup` | - | Initialize project context | Setup |
| `/conductor-design` | `ds` | Start Double Diamond design session | 1-8 |
| `/conductor-newtrack` | `cn` | Create spec + plan + beads from design.md | 5-8 |
| `/conductor-implement` | `ci` | Execute track with TDD | 9 |
| `/conductor-orchestrate` | `co` | Spawn parallel workers | 9 |
| `/conductor-autonomous` | `ca` | Ralph loop (autonomous) | 9 |
| `/conductor-status` | - | Display progress overview | Any |
| `/conductor-revise` | - | Update spec/plan mid-work | Any |
| `/conductor-validate` | - | Verify beads, run `bv` | 6 |
| `/conductor-finish` | - | Extract learnings, archive track | 10 |
| `/conductor-handoff` | `ho` | Save/load session context | Any |

---

## Triggers

| Trigger | Skill | Action |
|---------|-------|--------|
| `ds` | designing | Double Diamond session (phases 1-10) |
| `cn` | designing | Create track from design.md |
| `pl` | designing | Planning phases (5-10) |
| `fb` | tracking | File beads from plan |
| `rb` | tracking | Review beads |
| `tdd` | conductor | RED-GREEN-REFACTOR cycle |
| `ci` | conductor | Execute track (auto-routes if parallel) |
| `co` | orchestrator | Spawn parallel workers |
| `ca` | conductor | Autonomous execution (Ralph) |
| `ho` | handoff | Save/load session context |
| `finish branch` | conductor | Finalize and merge/PR |

---

## Skills

9 skills organized by workflow phase:

### Core Routing

| Skill | Description |
|-------|-------------|
| **maestro-core** | Central router. Skill hierarchy, HALT/DEGRADE policies, trigger mappings. Load FIRST before any workflow skill. |

### Planning Skills

| Skill | Description |
|-------|-------------|
| **designing** | Double Diamond design sessions. 10-phase unified pipeline with A/P/C checkpoints. Triggers: `ds`, `cn`, `pl`. |

### Execution Skills

| Skill | Description |
|-------|-------------|
| **conductor** | Implementation execution. TDD by default, beads integration, validation gates. Triggers: `ci`, `ca`, `tdd`. |
| **orchestrator** | Multi-agent parallel execution. Spawns workers, Agent Mail coordination. Triggers: `co`, "run parallel". |
| **tracking** | Issue tracking via beads. `bd` CLI wrapper, dependency graphs. Triggers: `fb`, `rb`, `bd *`. |

### Session Skills

| Skill | Description |
|-------|-------------|
| **handoff** | Session cycling. Context preservation, resume support. Triggers: `ho`, `/conductor-finish`. |

### Utility Skills

| Skill | Description |
|-------|-------------|
| **creating-skills** | Author new skills. SKILL.md structure, best practices. |
| **sharing-skills** | Contribute skills upstream. Branch, commit, PR workflow. |
| **using-git-worktrees** | Isolated workspaces. Smart directory selection, safety verification. |

### Skill Hierarchy

```
conductor (1) > orchestrator (2) > designing (3) > tracking (4) > specialized (5)
```

Higher rank wins on conflicts.

---

## Unified Pipeline (10 Phases)

| # | Phase | Type | Purpose | Exit Criteria |
|---|-------|------|---------|---------------|
| 1 | **DISCOVER** | Diverge | Explore problem + research | Problem articulated |
| 2 | **DEFINE** | Converge | Frame problem + approach | Approach selected |
| 3 | **DEVELOP** | Diverge | Architecture + components | Interfaces defined |
| 4 | **VERIFY** | Converge | Oracle audit + risk | Oracle APPROVED |
| 5 | **DECOMPOSE** | Execute | Create beads (`fb`) | Beads filed |
| 6 | **VALIDATE** | Execute | Dependency check (`bv`) | Dependencies valid |
| 7 | **ASSIGN** | Execute | Track assignments | Tracks assigned |
| 8 | **READY** | Complete | Handoff to `ci`/`co` | Execution ready |
| 9 | **EXECUTE** | Implement | Run implementation | All beads completed |
| 10 | **FINISH** | Archive | Extract learnings | Track archived |

### Mode Routing

| Score | Mode | Phases | A/P/C | Research |
|-------|------|--------|-------|----------|
| < 4 | **SPEED** | 1,2,4,8 | No | 1 hook |
| 4-6 | **ASK** | User chooses | Optional | User chooses |
| > 6 | **FULL** | 1-10 | Yes | 2 hooks |

---

## A/P/C Checkpoints

**Advanced / Party / Continue** - decision points at end of phases 1-4 (FULL mode only).

| Option | Action |
|--------|--------|
| **[A] Advanced** | Phase-specific deep dive |
| **[P] Party** | Multi-agent feedback (BMAD v6 personas) |
| **[C] Continue** | Proceed to next phase |
| **[↩ Back]** | Return to previous phase |

### Advanced Options by Phase

| After Phase | A Option |
|-------------|----------|
| 1 (DISCOVER) | Advanced assumption audit |
| 2 (DEFINE) | Scope stress-test |
| 3 (DEVELOP) | Architecture deep-dive |
| 4 (VERIFY) | Oracle runs BEFORE menu |

### State Ladder

```
INLINE → MICRO_APC → NUDGE → DS_FULL → DS_BRANCH → BRANCH_MERGE
```

---

## bd CLI Cheatsheet

### Essential Commands

| Command | Action |
|---------|--------|
| `bd ready --json` | Find available work |
| `bd show <id>` | Read task context |
| `bd update <id> --status in_progress` | Claim task |
| `bd update <id> --notes "..."` | Add notes |
| `bd close <id> --reason completed` | Close (completed/skipped/blocked) |
| `bd status` | Show ready + in_progress |
| `bd sync` | Sync to git |

### Session Pattern

```bash
# Start
bd ready --json                      # Find work
bd show <id>                         # Read context
bd update <id> --status in_progress  # Claim

# During (heartbeat every 5 min)
# ...work...

# End
bd update <id> --notes "COMPLETED: X. NEXT: Y"
bd close <id> --reason completed
bd sync
```

### bv (Beads Validation)

```bash
bv --robot-stdout    # Machine-readable (NEVER run bare `bv`)
bv --robot-status    # Status code only
```

**WARNING:** Bare `bv` hangs. Always use `--robot-*` flags.

---

## Directory Structure

```
conductor/
├── product.md              # Product context
├── tech-stack.md           # Technology decisions
├── workflow.md             # Workflow preferences
├── code_styleguides/       # Language-specific rules
├── CODEMAPS/               # Architecture documentation
├── handoffs/               # Session context
│   ├── <track-id>/
│   │   ├── index.md        # Handoff log
│   │   └── YYYY-MM-DD_*.md # Individual handoffs
│   └── general/
├── spikes/                 # Research spikes
└── tracks/<id>/            # Per-track work
    ├── design.md           # Design document
    ├── spec.md             # Specification
    ├── plan.md             # Implementation plan
    └── metadata.json       # State tracking

.beads/
├── beads.db                # SQLite database
└── beads.jsonl             # Export format

skills/                     # Project skills (symlink to .claude/skills/)
.claude/skills/             # Actual skill location
```

---

## Fallback Policies

| Condition | Action | Message |
|-----------|--------|---------|
| `bd` unavailable | **HALT** | `❌ Cannot proceed: bd CLI required` |
| `conductor/` missing | **DEGRADE** | `⚠️ Standalone mode - limited features` |
| Agent Mail unavailable | **HALT** | `❌ Cannot proceed: Agent Mail required` |

### Decision Tree: bd vs TodoWrite

```
bd available?
├─ YES → Use bd CLI
└─ NO  → HALT (do NOT use TodoWrite as fallback)
```

---

## Session Protocol

### First Message

1. Check `conductor/handoffs/` for recent handoffs (< 7 days)
2. If found: `📋 Prior context: [track] (Xh ago)`
3. Skip if: "fresh start", no `conductor/`, or handoffs > 7 days

### Preflight Triggers

| Command | Preflight |
|---------|-----------|
| `/conductor-implement` | ✅ Yes |
| `/conductor-orchestrate` | ✅ Yes |
| `ds` | ❌ Skip |
| `bd ready/show/list` | ❌ Skip |

### Session Identity

- Format: `{BaseAgent}-{timestamp}` (internal)
- Registered on `/conductor-implement` or `/conductor-orchestrate`
- Stale threshold: 10 min → takeover prompt

### Ralph (Autonomous Mode)

| Phase | Action |
|-------|--------|
| Start | `ca` sets `ralph.active = true`, invokes ralph.sh |
| During | Iterates stories, updates passes status |
| End | `ralph.active = false`, `workflow.state = DONE` |

**Exclusive Lock:** `ci`/`co` blocked while `ralph.active` is true.

---

## MCPorter Toolboxes

CLI tools generated from MCP servers via [MCPorter](https://github.com/steipete/mcporter).

### Location

`toolboxes/<tool>/<tool>.js`

### Available Tools

| CLI | Source | Description |
|-----|--------|-------------|
| `agent-mail/agent-mail.js` | mcp-agent-mail | Agent coordination and messaging |

### Usage

```bash
# From project root
toolboxes/agent-mail/agent-mail.js <command> [args...]

# Example: health check
toolboxes/agent-mail/agent-mail.js health-check

# Example: send message
toolboxes/agent-mail/agent-mail.js send_message to:BlueLake subject:"Hello"
```

### Argument Syntax

```bash
# Colon-delimited
agent-mail.js send_message to:BlueLake subject:"Hello"

# Equals-delimited  
agent-mail.js send_message to=BlueLake subject="Hello"

# Function-call style
agent-mail.js 'send_message(to: "BlueLake", subject: "Hello")'
```

---

## Orchestrator Protocol

### 8-Phase Protocol

| Phase | Action |
|-------|--------|
| 0. Preflight | Session identity, detect active sessions |
| 1. Read Plan | Parse Track Assignments from plan.md |
| 2. Validate | Health check Agent Mail (HALT if unavailable) |
| 3. Initialize | ensure_project, register orchestrator + workers |
| 4. Spawn | Task() for each track (parallel) |
| 5. Monitor | fetch_inbox, verify worker summaries |
| 6. Resolve | reply_message for blockers |
| 7. Complete | Send summary, close epic, `rb` review |

### Worker 4-Step Protocol

```
┌─────────────────────────────────────────────────────────────┐
│  STEP 1: INITIALIZE  - macro_start_session() FIRST         │
│  STEP 2: EXECUTE     - claim beads, do work, close beads   │
│  STEP 3: REPORT      - send_message() to orchestrator      │
│  STEP 4: CLEANUP     - release_file_reservations()         │
└─────────────────────────────────────────────────────────────┘
```

---

## Troubleshooting

| Problem | Cause | Solution |
|---------|-------|----------|
| `bv` hangs | Missing `--robot-*` flag | Always use `bv --robot-stdout` |
| Task not found | Stale beads cache | Run `bd sync` |
| Agent Mail error | CLI not available | Check `toolboxes/agent-mail/agent-mail.js health-check` |
| Parallel not detected | Missing Track Assignments | Add `## Track Assignments` to plan.md |
| Handoff stale | > 7 days old | Create new handoff or use "fresh start" |
| `ci`/`co` blocked | Ralph active | Wait for `ca` to complete |
| Skill not loading | Wrong trigger | Check routing table in maestro-core |
| File conflicts | Missing reservation | Use `file_reservation_paths` before editing |

### Validation Checklist

```bash
# Verify beads CLI
bd --version

# Verify Agent Mail
toolboxes/agent-mail/agent-mail.js health-check

# Verify project structure
ls conductor/
ls .beads/

# Validate plugin manifest
cat .claude-plugin/plugin.json | jq .
```

---

## Critical Rules

1. Use `--json` with `bd` for structured output
2. Use `--robot-*` with `bv` (bare `bv` hangs)
3. Never write production code without failing test first (TDD)
4. Always commit `.beads/` with code changes
5. Load `maestro-core` FIRST before any workflow skill

---

## See Also

| Topic | Path |
|-------|------|
| Full tutorial | [TUTORIAL.md](TUTORIAL.md) |
| Agent guidance | [AGENTS.md](AGENTS.md) |
| Beads workflow | [skills/tracking/references/workflow-integration.md](skills/tracking/references/workflow-integration.md) |
| Handoff system | [skills/conductor/references/workflows/handoff.md](skills/conductor/references/workflows/handoff.md) |
| Agent coordination | [skills/orchestrator/references/agent-coordination.md](skills/orchestrator/references/agent-coordination.md) |
| TDD checkpoints | [skills/conductor/references/tdd-checkpoints-beads.md](skills/conductor/references/tdd-checkpoints-beads.md) |
| Routing table | [skills/maestro-core/references/routing-table.md](skills/maestro-core/references/routing-table.md) |
| Pipeline details | [skills/designing/references/pipeline.md](skills/designing/references/pipeline.md) |
| Glossary | [skills/maestro-core/references/glossary.md](skills/maestro-core/references/glossary.md) |
