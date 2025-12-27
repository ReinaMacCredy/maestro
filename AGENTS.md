# AGENTS.md - Maestro Plugin

## Overview
Claude Code plugin bundling workflow skills: Conductor (planning), Design (Double Diamond sessions), Beads (issue tracking), and Superpowers (TDD, debugging, code review).

## Build/Test Commands
No build required - this is a skill/documentation plugin. Validate JSON:
```bash
cat .claude-plugin/plugin.json | jq .   # Validate plugin manifest
```

## Architecture
```
skills/           # Skill directories, each with SKILL.md (frontmatter + instructions)
  beads/          # Issue tracking skill with references/ subdirectory
  conductor/      # Planning methodology (includes /conductor-design, CODEMAPS generation)
  design/         # Double Diamond design sessions (ds trigger), includes bmad/
  continuity/     # Session state preservation (replaces session-compaction)
  ...             # TDD, debugging, code review, etc.
lib/              # Shared utilities (skills-core.js)
.claude-plugin/   # Plugin manifest (plugin.json, marketplace.json)
conductor/        # Unified save location for plans and tracks
  tracks/<id>/    # Active work (design.md + spec.md + plan.md per track)
    metadata.json       # Track info + thread IDs + generation + beads state
  CODEMAPS/       # Architecture documentation (overview.md, module codemaps)
  archive/        # Completed work
```

## Handoff Mechanism (Planning → Execution)

**What is Handoff?**
Handoff is the structured transition of work between AI agent sessions or phases. Since agent threads have limited context and sessions may expire, handoff ensures continuity by:
- Capturing decisions, context, and progress in persistent files (`design.md`, `spec.md`, `plan.md`)
- Creating trackable work items (beads/issues) that survive session boundaries
- Enabling any future session to resume work without losing context

**Why Handoff Matters:**
- **Context Preservation**: Threads get compacted or abandoned; handoff artifacts persist
- **Multi-Session Work**: Complex tasks span multiple sessions; handoff bridges them
- **Human-AI Collaboration**: Humans can review artifacts between sessions
- **Resumability**: Any agent can pick up where another left off using `bd ready`

**Unified flow via `/conductor-newtrack`:**
```
ds → design.md → /conductor-newtrack → spec.md + plan.md + beads + review
```

**Flags:**
- `--no-beads` / `-nb`: Skip beads filing (spec + plan only)
- `--plan-only` / `-po`: Alias for --no-beads
- `--force`: Overwrite existing track or remove stale locks

**State files:**
- `metadata.json`: Track info, thread IDs, generation state, and beads filing state

**Execution session starts with:** `Start epic <epic-id>` or `/conductor-implement <track-id>`

## Code Style
- Skills: Markdown with YAML frontmatter (`name`, `description` required)
- Commands: Markdown files defining slash command behavior
- Follow existing skill structure: SKILL.md at skill root, optional references/ subdirectory
- Keep skills self-contained with minimal cross-references

## Naming Conventions
- Skill directories: kebab-case (`test-driven-development`, `using-git-worktrees`)
- SKILL.md must match directory name in frontmatter `name` field

## Versioning

### Plugin Version (Automated)
Plugin version in `.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json` is **auto-bumped by CI** based on conventional commits:
- `feat:` → minor bump (1.5.0 → 1.6.0)
- `fix:` → patch bump (1.5.0 → 1.5.1)
- `feat!:` or `BREAKING CHANGE:` → major bump (1.5.0 → 2.0.0)
- `docs:`, `chore:` → changelog only, no version bump

### Skill Versions (Manual)
Individual skill versions in SKILL.md frontmatter remain **manually updated**:
- **Major bump** (1.x.x → 2.x.x): Breaking changes, renamed triggers, removed features
- **Minor bump** (x.1.x → x.2.x): New features, significant changes
- **Patch bump** (x.x.1 → x.x.2): Small fixes, tweaks

### Escape Hatch
Add `[skip ci]` to commit message to bypass all automation (changelog + version bump).

## Key Skills

| Skill | Trigger | Description |
|-------|---------|-------------|
| `design` | `ds` | Double Diamond design session with A/P/C checkpoints and Party Mode option |
| `conductor` | `/conductor-setup`, `/conductor-design`, `/conductor-newtrack`, `/conductor-implement`, `/conductor-status`, `/conductor-revert`, `/conductor-revise`, `/conductor-finish`, `/conductor-validate`, `/conductor-block`, `/conductor-skip` | Structured planning and execution through specs and plans |
| `beads` | `fb`, `rb`, `bd ready`, `bd status` | Issue tracking: file beads from plan, review beads, multi-session work |
| `doc-sync` | `/doc-sync`, after `/conductor-finish` | Auto-sync documentation with code changes |

<!-- bv-agent-instructions-v1 -->

---

## Beads Workflow Integration

This project uses [beads_viewer](https://github.com/Dicklesworthstone/beads_viewer) for issue tracking. Issues are stored in `.beads/` and tracked in git.

### Beads-Conductor Integration

Conductor commands automatically manage beads lifecycle via a **facade pattern**:

| Integration Point | Conductor Command | Beads Action |
|-------------------|-------------------|--------------|
| Preflight | All commands | Mode detect (SA/MA), validate bd |
| Track Init | `/conductor-newtrack` | Create epic + issues from plan.md |
| Claim | `/conductor-implement` | `bd update --status in_progress` |
| Close | `/conductor-implement` | `bd close --reason completed` |
| Sync | All (session end) | `bd sync` with retry |
| Compact | `/conductor-finish` | AI summaries for closed issues |
| Cleanup | `/conductor-finish` | Remove oldest when >150 closed |

**Zero manual bd commands** in the happy path - Conductor handles everything.

### Dual-Mode Architecture

| Mode | Description | Used When |
|------|-------------|-----------|
| **SA** (Single-Agent) | Direct `bd` CLI calls | Default, one agent |
| **MA** (Multi-Agent) | Village MCP server | Parallel agents coordinating |

Mode is detected at session start and locked for the session.

### Essential Commands

```bash
# View issues (launches TUI - avoid in automated sessions)
bv

# CLI commands for agents (use these instead)
bd ready              # Show issues ready to work (no blockers)
bd list --status=open # All open issues
bd show <id>          # Full issue details with dependencies
bd create --title="..." --type=task --priority=2
bd update <id> --status=in_progress
bd close <id> --reason="Completed"
bd close <id1> <id2>  # Close multiple issues at once
bd sync               # Commit and push changes

# Cleanup commands (used by /conductor-finish Phase 2)
bd compact --analyze --json      # Find issues needing summary
bd compact --apply --id <id> --summary "text"  # Add AI summary
bd count --status closed --json  # Count closed issues
bd cleanup --older-than 0 --limit <n> --force  # Remove oldest closed
```

### Session Protocol

**Session Start:**
```bash
# Preflight runs automatically via Conductor commands
# If manual, check bd availability first:
bd version            # Verify bd is available

# Find and claim work:
bd ready --json       # Get available tasks
bd update <id> --status in_progress
```

**During Session:**
- Update heartbeat every 5 minutes (automatic)
- Use `--tdd` flag for RED/GREEN/REFACTOR checkpoints
- Close tasks with reason: `completed`, `skipped`, or `blocked`

**Session End:**
```bash
bd close <id> --reason completed  # Close current task
bd sync                           # Sync beads to git
git add <files> && git commit -m "..."  # Commit code changes
git push                          # Push to remote
```

### State Files

| File | Location | Purpose |
|------|----------|---------|
| `LEDGER.md` | `conductor/sessions/active/` | Session state in frontmatter (mode, bound_track, bound_bead, tdd_phase) |
| `session-lock_<track>.json` | `.conductor/` | Concurrent session prevention |
| `pending_*.jsonl` | `.conductor/` | Failed operations for replay |
| `metrics.jsonl` | `.conductor/` | Usage metrics (append-only) |

### Metrics Logging

Usage events are logged to `.conductor/metrics.jsonl`:

```jsonl
{"event": "ma_attempt", "mode": "MA", "timestamp": "2025-12-25T10:00:00Z"}
{"event": "tdd_cycle", "taskId": "bd-42", "phase": "GREEN", "duration": 180, "timestamp": "..."}
{"event": "manual_bd", "command": "bd show", "timestamp": "..."}
```

Run `scripts/beads-metrics-summary.sh` for weekly summary.

### Key Concepts

- **Dependencies**: Issues can block other issues. `bd ready` shows only unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers, not words)
- **Types**: task, bug, feature, epic, question, docs
- **Blocking**: `bd dep add <issue> <depends-on>` to add dependencies
- **planTasks Mapping**: Bidirectional mapping between plan task IDs and bead IDs in `metadata.json.beads`

### References

- [Beads Facade](skills/conductor/references/beads-facade.md) - API contract
- [Beads Integration](skills/conductor/references/beads-integration.md) - All 13 integration points
- [Preflight Workflow](skills/conductor/references/conductor/preflight-beads.md) - Session initialization
- [Session Workflow](skills/conductor/references/conductor/beads-session.md) - Claim/close/sync protocol

<!-- end-bv-agent-instructions -->

---

## Continuity (Session Preservation)

Automatic session state preservation across sessions and compactions.

### Claude Code (Automatic)

When hooks are installed at `~/.claude/hooks/`:
- **SessionStart**: Auto-loads LEDGER.md + last handoff
- **PreCompact**: Auto-creates handoff before compaction
- **PostToolUse**: Tracks modified files
- **Stop**: Archives session on exit

Install hooks: `./scripts/install-global-hooks.sh`

### Amp Code (Manual)

Run these commands during your session:
- `continuity load` - At session start
- `continuity save` - After significant changes
- `continuity handoff` - Before ending session

See [skills/continuity/references/amp-setup.md](skills/continuity/references/amp-setup.md) for full setup.

### Data Storage

```text
conductor/sessions/
├── active/LEDGER.md    # Current session state (gitignored)
└── archive/*.md        # Archived handoffs (committed)
```

### Search History

```bash
uv run scripts/artifact-query.py <query>   # FTS5 search
uv run scripts/artifact-index.py           # Rebuild index
uv run scripts/artifact-cleanup.py         # Remove old handoffs
```

---

## Agent Coordination (Optional)

When `agent_mail` MCP is available, agents can coordinate file access and share context.

### Session Protocol

**Session start:**
```bash
# Check inbox for context from previous sessions
fetch_inbox(project_key, agent_name)
```

**Session end:**
```bash
# Send handoff message for next session
send_message(project_key, sender_name, to, subject, body_md)
```

### Parallel Dispatch

Before dispatching parallel subagents:
1. Reserve files with `file_reservation_paths`
2. Inject coordination block into Task prompts
3. Release reservations after completion

See [skills/dispatching-parallel-agents/references/agent-coordination/](skills/dispatching-parallel-agents/references/agent-coordination/) for full protocol.

### Failure Handling

If MCP is unavailable:
- Proceed without coordination (work completion is mandatory)
- Show `⚠️ Agent coordination unavailable` warning
- Don't block on optional features
