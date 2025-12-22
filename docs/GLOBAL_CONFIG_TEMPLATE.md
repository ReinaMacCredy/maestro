# Maestro Plugin - Global Configuration Template

Add this to your global config file after installing maestro plugin.

| Tool | Config File |
|------|-------------|
| Claude Code | `~/.claude/CLAUDE.md` |
| Amp | `~/.config/amp/AGENTS.md` |
| Codex | `~/.codex/AGENTS.md` |

> **Local Reference:** After plugin installation, this template is available at:
> `~/.claude/plugins/marketplaces/maestro-marketplace/docs/GLOBAL_CONFIG_TEMPLATE.md`

---

## Core Triggers

**Planning:** `ds`(design) → `/conductor-setup` → `/conductor-newtrack`

**Execution:** `fb` → `rb` → `/conductor-implement` (uses TDD) → `finish branch`

**Utilities:** `/ground`, `/doc-sync`, `/compact`, `dispatch`, `git worktree`

**Review:** `rb`, `review code`

---

## Session Protocol

**Start:**
```bash
bv --robot-status                     # Check team state (multi-agent)
bd ready --json                       # Find available work
bd list --status in_progress --json   # Check active work
bd show <issue-id>                    # Read notes to resume
```

**End:**
```bash
bd update <id> --notes "COMPLETED: X. IN PROGRESS: Y. NEXT: Z"
bd sync
```

---

## Trigger Phrases

### Conductor (Planning)

| Phrase | Skill | Description |
|--------|-------|-------------|
| `/conductor-setup` | `conductor` | Initialize project context |
| `/conductor-newtrack [desc]` | `conductor` | Create feature/bug track with spec + plan |
| `/conductor-implement [id]` | `conductor` | Execute track tasks with TDD |
| `/conductor-status` | `conductor` | Display progress overview |

### Beads (Issue Tracking)

| Phrase | Skill | Description |
|--------|-------|-------------|
| `fb`, `file beads` | `file-beads` | Convert plan to bd issues |
| `rb`, `review beads` | `review-beads` | Review and refine issues |
| `bd ready` | `beads` | Check available work |
| `bd status` | `beads` | Show ready + in_progress |
| `bd checkpoint` | `beads` | Update notes before compaction |

### Development

| Phrase | Skill | Description |
|--------|-------|-------------|
| `tdd` | `test-driven-development` | Enter TDD mode |
| `ct`, `claim task` | `execution-workflow` | Claim + implement next task |
| `git worktree` | `using-git-worktrees` | Create isolated feature branch |
| `finish branch` | `finishing-a-development-branch` | Finalize and prepare for merge |
| `execute plan` | `plan-executor` | Execute implementation plan |

### Debugging

| Phrase | Skill | Description |
|--------|-------|-------------|
| `debug`, `investigate` | `systematic-debugging` | Four-phase debugging |
| `trace`, `find source` | `root-cause-tracing` | Trace bugs backward |
| `flaky`, `race condition` | `condition-based-waiting` | Replace timeouts with polling |

### Exploration

| Phrase | Skill | Description |
|--------|-------|-------------|
| `ds` | `design` | Design session - collaborative brainstorming |
| `bs`, `brainstorm` | `brainstorming` | Deep exploration, creative design |
| `spike [topic]` | `spike-workflow` | Time-boxed research |
| `retro` | `retro-workflow` | Capture lessons learned |

### Memory & Meta

| Phrase | Skill | Description |
|--------|-------|-------------|
| `/compact` | `memory-update` | Archive scratch, migrate facts |

---

## Workflow Pipeline

```
PLANNING
  /conductor-setup → product.md, tech-stack.md, workflow.md
  /conductor-newtrack → spec.md + plan.md
  fb → bd issues

EXECUTION
  bd ready → ct → TDD cycle → bd checkpoint → finish branch

RETROSPECTIVE
  bd close → retro → history/retros/
```

## Workflow Chains

| Scenario | Flow |
|----------|------|
| Standard | `/conductor-newtrack` → `fb` → `bd ready` → `ct` |
| With exploration | `bs` → `/conductor-newtrack` → `fb` |
| Resume work | `bd status` → `ct` |

---

## Beads CLI Reference

| Command | Purpose |
|---------|---------|
| `bd create "title" -t bug\|feature\|task -p 0-3` | Create issue |
| `bd update <id> --status in_progress` | Claim work |
| `bd dep add <from> <to> --type blocks` | Add dependency |
| `bd close <id> --reason "summary"` | Complete work |
| `bd blocked --json` | Find stuck work |

**Dependency Types:** `blocks` (hard), `related` (soft), `parent-child`, `discovered-from`

---

## Beads Village (Multi-Agent)

MCP server for task coordination via `npx beads-village`.

| Tool | Purpose |
|------|---------|
| `init` | Join workspace with team/role |
| `claim` | Atomic task claiming |
| `done` | Complete task, release locks |
| `reserve` / `release` | Lock/unlock files |
| `msg` / `inbox` | Team messaging |
| `status` | View team state |
| `assign` | (Leader) Assign tasks |

---

## Available Skills (15)

| Skill | Trigger | Description |
|-------|---------|-------------|
| `beads` | `bd ready`, `bd status` | Issue tracking for multi-session work |
| `file-beads` | `fb` | File beads from plan |
| `review-beads` | `rb` | Review and refine beads issues |
| `codemaps` | — | Token-aware architecture documentation |
| `conductor` | `/conductor-design`, `/conductor-newtrack` | Design and planning through dialogue |
| `dispatching-parallel-agents` | `dispatch` | 2+ independent parallel tasks |
| `doc-sync` | `doc-sync`, `/doc-sync` | Sync AGENTS.md from completed threads |
| `finishing-a-development-branch` | `finish branch` | Complete work: merge/PR/cleanup |
| `sharing-skills` | `share skill` | Contribute skills upstream via PR |
| `subagent-driven-development` | — | Coordinate multiple subagents |
| `test-driven-development` | `tdd` | RED-GREEN-REFACTOR cycle |
| `using-git-worktrees` | `git worktree` | Isolated feature work |
| `using-superpowers` | — | Session initialization |
| `verification-before-completion` | — | Evidence before assertions |
| `writing-skills` | `write skill` | Create or edit skills |

---

## Code Quality & Security

- **No emojis** in code, comments, or docs
- **Security:** No SQL concat, validate inputs, no hardcoded secrets
- **Testing:** TDD (RED-GREEN-REFACTOR), AAA pattern, fail fast
- **Docs:** Explain WHY decisions were made

---

## Critical Rules

- **Always** use `--robot-*` flags with `bv` CLI (bare `bv` hangs)
- **Always** use `--json` flags with `bd` for structured output
- **Never** write production code without a failing test first
- **Always** commit `.beads/` with code changes

---

## Standard Paths

| Type | Path |
|------|------|
| Conductor | `conductor/{product,tech-stack,workflow,tracks}.md` |
| Tracks | `conductor/tracks/<id>/{design,spec,plan}.md` |
| Design Docs | `conductor/plans/*.md` |
| Beads | `.beads/` |
| Village | `.beads-village/`, `.reservations/`, `.mail/` |

---

## Language & Git

See `~/.config/amp/rules/*.md` for language-specific rules.
See `~/.config/amp/rules/git-commit-conventions.md` for commit style.

---

## Installation Verification

After adding to your global config, verify:

```bash
# Check beads CLI
bd --version

# Test trigger
/conductor-design  # Should activate design exploration workflow
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Triggers not working | Ensure plugin is installed: `/plugin list` |
| `bd: command not found` | Install beads CLI (see SETUP_GUIDE.md § "Step 3: Install CLI Tools") |
| Skills not loading | Check `~/.claude/settings.json` has `enabledPlugins` |
| Workflow confusion | Re-read TUTORIAL.md for complete guide |
