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

## Maestro Workflow

### Core Triggers

**Planning:** `bs` (brainstorm) → `/conductor-setup` → `/conductor-newtrack`

**Execution:** `fb` → `bd ready` → `ct` → `tdd` → `finish branch`

**Utilities:** `/ground`, `/doc-sync`, `/compact`, `dispatch`, `git worktree`

**Review:** `rb`, `review code`

### Workflow Pipeline

```
PLANNING PHASE
  bs (brainstorm)
       │
       └─ Creates: conductor/design/YYYY-MM-DD-<topic>-design.md
       
  /conductor-newtrack [description]
       │
       ├─ Step 1: Clarifying questions
       ├─ Step 2: Generate spec.md
       ├─ Step 3: Generate plan.md
       │
       └─ Creates: conductor/tracks/<id>/{spec.md, plan.md}
                        │
                        ▼
  fb (file beads) → beads issues created (.beads/ database)
                        │
                        ▼
                   Outputs HANDOFF block

EXECUTION PHASE (new session)
  Paste HANDOFF block
       │
       └─ ct (claim task) → TDD cycle → verify → close
                                              │
                                              ▼
                              finish branch → merge/PR
```

### Workflow Chains

| Scenario | Flow |
|----------|------|
| Standard | `/conductor-newtrack` → `fb` → `bd ready` → `ct` |
| With exploration | `bs` → `/conductor-newtrack` → `fb` |
| Resume work | `bd status` → `ct` |

### Standard Paths

| Type | Path |
|------|------|
| Conductor Context | `conductor/{product,tech-stack,workflow,tracks}.md` |
| Conductor Tracks | `conductor/tracks/<id>/{spec,plan}.md` |
| Design Docs | `conductor/design/*.md` |
| Beads Database | `.beads/` |

### Session Protocols

**Session Start:**
```bash
bd ready --json          # Find available work
bd list --status in_progress --json  # Check active work
bd show <issue-id>       # Read notes to resume
```

**Session End:**
```bash
bd update <id> --notes "COMPLETED: X. IN PROGRESS: Y. NEXT: Z"
```

### Code Quality

- TDD for all implementation (write test first)
- Run verification before claiming done
- No emojis in code, comments, or docs
- Conventional commits: `feat:`, `fix:`, `test:`, `refactor:`

---

## Installation Verification

After adding to your global config, verify:

```bash
# Check skills loaded
/skill list  # Should show skills including conductor, beads, brainstorming

# Check beads CLI
bd --version

# Test trigger
bs  # Should activate brainstorming skill
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Triggers not working | Ensure plugin is installed: `/plugin list` |
| `bd: command not found` | Install beads CLI (see SETUP_GUIDE.md § "Step 3: Install CLI Tools") |
| Skills not loading | Check `~/.claude/settings.json` has `enabledPlugins` |
| Workflow confusion | Re-read TUTORIAL.md for complete guide |
