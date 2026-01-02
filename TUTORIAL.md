# Maestro: The Complete Guide

> **For humans**: Read this to understand what these skills do and why they matter.
> **For agents**: See the Quick Reference at the end for triggers.

---

## Why This Exists

AI coding agents are powerful but forgetful. They lose context between sessions, let plans evaporate into chat, write tests as afterthoughts, and debug chaotically. Maestro solves these problems by giving agents a structured methodology: persistent planning artifacts, dependency-aware issue tracking, and consistent workflows that survive session boundaries.

---

## Key Insights (The "Aha" Moments)

1. **"Spend tokens once on a good plan; reuse it many times."**
   Long, fuzzy chats chew context. A structured spec+plan is cheaper to revisit.

2. **"Your project's state lives in git, not in the agent's memory."**
   Chat history is ephemeral. Beads issues persist in `.beads/` and survive compaction.

3. **"Beads are dependency-aware, not a flat todo list."**
   Encode constraints once ("A blocks B"). All later sessions respect them.

4. **"Skills are mental modes, not just commands."**
   Invoking `tdd` or `debug` switches the agent into a specific methodology.

5. **"Evidence before assertions."**
   Don't claim "tests pass"—show the output. Don't claim "fixed"—show the verification.

---

## Core Concepts

### Conductor (Planning)

Conductor creates structured planning artifacts that persist across sessions:

| Artifact | Purpose | Created By |
|----------|---------|------------|
| `design.md` | High-level architecture decisions | `ds` or `/conductor-design` |
| `spec.md` | Requirements and acceptance criteria | `/conductor-newtrack` |
| `plan.md` | Task breakdown with status markers | `/conductor-newtrack` |
| `metadata.json` | Track state and validation info | `/conductor-newtrack` |

**Directory structure:**
```
conductor/
├── product.md, tech-stack.md, workflow.md  # Project context
├── CODEMAPS/                               # Architecture docs
├── handoffs/                               # Session context (git-committed)
└── tracks/<track_id>/                      # Per-track work
    ├── design.md, spec.md, plan.md
    └── metadata.json
```

### Beads (Issue Tracking)

Beads are persistent, dependency-aware issues that survive session boundaries:

```bash
bd ready --json      # What's unblocked?
bd show <id>         # Read context and notes
bd update <id> --status in_progress
bd close <id> --reason completed
```

The key insight: **notes survive compaction**. Write handoff context there.

### Skills (Mental Modes)

Skills aren't scripts—they're methodologies the agent adopts:

| Skill | Trigger | What It Does |
|-------|---------|--------------|
| `design` | `ds` | Double Diamond design session |
| `conductor` | `/conductor-*` | Structured planning and execution |
| `beads` | `fb`, `rb` | File/review issues from plans |
| `orchestrator` | `/conductor-orchestrate` | Multi-agent parallel execution |

---

## The Complete Workflow

```
ds → /conductor-newtrack → /conductor-implement → /conductor-finish
```

### Phase 1: Design (`ds`)

Start with a Double Diamond design session:

```
User: ds
Agent: [loads design skill, begins collaborative dialogue]
```

The agent will:
1. Ask clarifying questions (one at a time, multiple choice preferred)
2. Explore 2-3 approaches with trade-offs
3. Present design in digestible sections
4. Write `design.md` when you're aligned

**Output**: `conductor/tracks/<id>/design.md`

### Phase 2: Create Track (`/conductor-newtrack`)

Convert the design into actionable artifacts:

```
User: /conductor-newtrack
Agent: [reads design.md, creates spec + plan + beads]
```

The agent will:
1. Generate `spec.md` with requirements and acceptance criteria
2. Generate `plan.md` with task breakdown
3. Automatically create beads via `fb` (file beads)
4. Optionally run `rb` (review beads) to refine

**Output**: `spec.md`, `plan.md`, `.beads/` issues

### Phase 3: Implement (`/conductor-implement`)

Execute the plan with TDD (enabled by default):

```
User: /conductor-implement
Agent: [claims bead, TDD cycle, closes bead, repeats]
```

The agent will:
1. Run `bd ready --json` to find unblocked work
2. Claim an issue with `bd update <id> --status in_progress`
3. Execute RED → GREEN → REFACTOR cycle
4. Close with `bd close <id> --reason completed`
5. Commit and repeat

**Disable TDD**: Use `--no-tdd` flag when appropriate.

### Phase 4: Finish (`/conductor-finish`)

Complete the track and extract learnings:

```
User: /conductor-finish
Agent: [validates, extracts learnings, archives]
```

The agent will:
1. Verify all beads closed and tests passing
2. Extract learnings to `AGENTS.md`
3. Sync documentation if needed
4. Archive the track

---

## Handoff: Preserving Context Across Sessions

### The Problem

AI agents forget everything between sessions. Context windows fill up, sessions end, and the next agent starts from scratch.

### The Solution

Handoff persists context in **files that outlive sessions**:

```
Session 1 (Planning):
  ds → design.md
  /conductor-newtrack → spec.md + plan.md + beads
  → HANDOFF (planning complete)

Session 2+ (Execution):
  /conductor-implement → execute epics
  → HANDOFF (after each epic)

Final Session:
  /conductor-finish → archive + learnings
```

### Handoff Artifacts

| Artifact | What It Preserves |
|----------|------------------|
| `design.md` | Architecture decisions, trade-offs |
| `spec.md` | Requirements, acceptance criteria |
| `plan.md` | Tasks with `[x]`/`[ ]` status markers |
| `.beads/` | Issues with notes field (key!) |
| `conductor/handoffs/` | Session snapshots |

### Handoff Triggers

Six triggers create handoffs at key moments:

| Trigger | When | Automatic |
|---------|------|-----------|
| `design-end` | After `/conductor-newtrack` | ✅ |
| `epic-start` | Before each epic | ✅ |
| `epic-end` | After each epic closes | ✅ |
| `pre-finish` | At start of `/conductor-finish` | ✅ |
| `manual` | User runs `/conductor-handoff` | ❌ |
| `idle` | 30min gap detected | ✅ (prompted) |

### Commands

```bash
/conductor-handoff         # Auto-detect mode (create or resume)
/conductor-handoff create  # Save current session context
/conductor-handoff resume  # Load prior session context
```

### Writing Good Handoff Notes

The beads **notes field** is your session-to-session memory:

```bash
bd update <id> --notes "COMPLETED: Auth middleware. IN PROGRESS: Token refresh. NEXT: Add tests for edge cases."
```

Write like you're leaving instructions for yourself in two weeks with zero context.

### Session Protocol

**At session end:**
```bash
bd update <id> --notes "COMPLETED: X. NEXT: Y."
git add -A && git commit -m "progress on feature"
git push
```

**At session start:**
```bash
bd ready --json      # What's unblocked?
bd show <id>         # Read notes for context
```

---

## Session Walkthrough: Building User Invitations

### Session 1: Design and Planning

```
User: ds

Agent: I'll help you design a new feature. What would you like to build?

User: User invitation system for our SaaS

Agent: [asks clarifying questions about email flow, permissions, expiry]
Agent: [explores approaches: magic links vs codes, self-service vs admin-only]
Agent: [presents design sections for approval]
Agent: Design captured in design.md. Create track now?

User: yes

Agent: [runs /conductor-newtrack]
Agent: [creates spec.md with 5 acceptance criteria]
Agent: [creates plan.md with 3 epics, 12 tasks]
Agent: [runs fb to create beads]
Agent: [runs rb to review and add dependencies]

Agent: Planning complete. 12 issues created across 3 epics:
  - E1: Database schema (3 tasks, unblocked)
  - E2: API endpoints (5 tasks, blocked by E1)
  - E3: Email integration (4 tasks, blocked by E2)

User: /conductor-handoff

Agent: Handoff saved to conductor/handoffs/invitations/...
```

### Session 2: First Epic

```
Agent: [auto-loads handoff on session start]
Agent: 📋 Prior context: invitations (2h ago)

User: /conductor-implement

Agent: Executing E1: Database schema
  [claims inv-001: Create invitations table]
  [TDD: writes test for schema, runs migration, refactors]
  [closes inv-001]
  [claims inv-002: Add foreign keys]
  ...

Agent: Epic E1 complete. 3/12 tasks done. Continue to E2?

User: no, stopping for today

Agent: [creates epic-end handoff]
```

### Session 3: Continue

```
Agent: 📋 Prior context: invitations E1 complete (16h ago)

User: continue

Agent: [runs /conductor-implement]
Agent: Executing E2: API endpoints (now unblocked)
  ...
```

---

## Common Scenarios

### Already Have a Plan

Skip Conductor, go straight to beads:

```
fb                          # File beads from existing plan
bd ready --json             # See what's unblocked
tdd                         # Execute with TDD
```

### Bug Investigation

```
bd create "Investigate billing 500" -t bug -p 0
tdd                         # Fix with TDD
bd close <id>
```

### Track in Bad State

Validate and auto-repair:

```
/conductor-validate <track-id>
```

---

## Quick Reference

### For Humans

| Task | Command |
|------|---------|
| Start design | `ds` |
| Create track from design | `/conductor-newtrack` |
| See what's ready | `bd ready --json` |
| Start implementation | `/conductor-implement` |
| Check progress | `/conductor-status` |
| Save context | `/conductor-handoff` |
| Complete track | `/conductor-finish` |

### For Agents: Triggers

| Trigger | Skill/Action |
|---------|--------------|
| `ds` | Design session (Double Diamond) |
| `/conductor-setup` | Initialize project |
| `/conductor-design` | Design with A/P/C checkpoints |
| `/conductor-newtrack` | Create spec + plan + beads |
| `/conductor-implement` | Execute track with TDD |
| `/conductor-status` | Show progress |
| `/conductor-revise` | Update spec/plan mid-work |
| `/conductor-finish` | Complete and archive track |
| `fb`, `file beads` | Create beads from plan |
| `rb`, `review beads` | Review and refine beads |
| `tdd` | Enter TDD mode |
| `/conductor-handoff` | Save/load session context |
| `/conductor-orchestrate` | Parallel workers |

### Critical Rules

1. **No production code without a failing test first** (use `--no-tdd` to disable)
2. **Always checkpoint before session end** — notes field survives compaction
3. **Commit `.beads/` with code** — it's your persistent memory
4. **Evidence before assertions** — show test output, don't just say "tests pass"

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Agent forgets context | Run `bd show <id>` for notes |
| Plan seems incomplete | Run `rb` to review beads |
| Tests pass immediately | You wrote code first. Delete it. Start with failing test. |
| Too many issues | Run `bd ready` for unblocked only |
| Track in bad state | Run `/conductor-validate <track-id>` |

---

## Tips

### Plan Before Each Epic

Before `/conductor-implement`, switch to plan mode (Shift+Tab in Claude Code) to let the agent strategize.

### Handoff Across Tools

| Tool | Handoff Method |
|------|----------------|
| Amp | Handoff command or `@T-<id>` references |
| Claude Code | `/compact` before session end |
| Codex | `/compact` before session end |

---

*Built on foundations from [BMAD-METHOD](https://github.com/bmad-code-org/BMAD-METHOD), [conductor](https://github.com/NguyenSiTrung/conductor), [beads](https://github.com/steveyegge/beads), and [Knowledge & Vibes](https://github.com/kyleobrien91/knowledge-and-vibes).*
