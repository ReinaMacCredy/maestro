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

**Planning:** `ds` → `/conductor-setup` → `/conductor-newtrack`

**Execution:** `fb` → `rb` → `/conductor-implement` (uses TDD) → `finish branch`

**Utilities:** `/ground`, `/doc-sync`, `/compact`, `dispatch`, `git worktree`

**Review:** `rb`, `review code`

### Quick Triggers

| Shortcut | Command | Description |
|----------|---------|-------------|
| `ds` | `/conductor-design` | Design a feature through dialogue |
| `st` | `/conductor-setup` | Initialize project context |
| `fb` | file-beads | File beads issues from plan |
| `rb` | review-beads | Review and refine beads issues |
| `ct` | claim task | Claim and implement next task |
| `bs` | brainstorming | Deep exploration before design |

### Workflow Pipeline

```
PLANNING PHASE
  /conductor-design "feature"
       │
       └─ Creates: conductor/tracks/<id>/design.md
       
  /conductor-newtrack
       │
       ├─ Uses design.md (if exists)
       ├─ Generate spec.md
       ├─ Generate plan.md
       │
       └─ Creates: conductor/tracks/<id>/{design.md, spec.md, plan.md}
                        │
                        ▼
  fb (file beads) → beads issues created (.beads/ database)
                        │
                        ▼
  rb (review beads) → refine issues
                        │
                        ▼
                   Outputs HANDOFF block

EXECUTION PHASE (new session)
  Paste HANDOFF block ("Start epic <id>")
       │
       └─ /conductor-implement → claims tasks → TDD cycle → verify → close
                                                                   │
                                                                   ▼
                                                   finish branch → merge/PR
```

### Workflow Chains

| Scenario | Flow |
|----------|------|
| Standard | `/conductor-design` → `/conductor-newtrack` → `fb` → `rb` → `/conductor-implement` |
| Skip design | `/conductor-newtrack` → `fb` → `rb` → `/conductor-implement` |
| Resume work | `bd status` → `/conductor-implement` or `Start epic <id>` |

### Standard Paths

| Type | Path |
|------|------|
| Conductor Context | `conductor/{product,tech-stack,workflow,tracks}.md` |
| Conductor Tracks | `conductor/tracks/<id>/{design,spec,plan}.md` |
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
/skill list  # Should show skills including conductor, beads

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
