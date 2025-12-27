# Continuity Integration Design

## Overview

Integrate Continuous-Claude patterns into Maestro workflow for automatic session state preservation across sessions and compactions.

## Problem Statement

Maestro workflow lacks automatic context preservation, leading to:
- Lost information when sessions get long/compact
- Difficulty resuming work after session breaks
- No automatic learnings capture
- Poor visibility into session history

## Solution

Multi-platform continuity system with:
- **Ledger**: Live session state (survives /clear)
- **Handoffs**: Cross-session context transfer
- **Hooks**: Platform-specific automation
- **Artifact Index**: Searchable history

## Architecture

```text
┌─────────────────────────────────────────────────────────┐
│                    COMMON LAYER                         │
├─────────────────────────────────────────────────────────┤
│  skills/continuity/     │ Skill interface               │
│  scripts/*.py           │ Python utilities              │
│  conductor/sessions/    │ Data storage                  │
└─────────────────────────────────────────────────────────┘
         │                      │                    │
    Claude Code              Amp Code             Codex
    (TS Hooks)           (amp.hooks JSON)     (Skills only)
```

## Components

### 1. Directory Structure
- `conductor/sessions/active/LEDGER.md` - Current state (gitignored)
- `conductor/sessions/archive/*.md` - Archived handoffs (committed)
- `conductor/.cache/artifact-index.db` - SQLite index (gitignored)

### 2. Skill: continuity
Replaces `session-compaction` with triggers:
- `continuity load` / `continuity save` / `continuity handoff`
- `continuity status` / `continuity search <query>`

### 3. Hooks (Claude Code)
Single TypeScript entry point at `~/.claude/hooks/`:
- SessionStart: Load ledger + last handoff
- PreCompact: Create auto-handoff
- PostToolUse: Track modified files
- Stop: Archive session

### 4. Hooks (Amp Code)
JSON in `~/.config/amp/AGENTS.md`:
- tool:post-execute reminder for save
- Session protocol instructions

### 5. Python Scripts
- `artifact-index.py` - Build/rebuild SQLite index
- `artifact-query.py` - FTS5 search
- `artifact-cleanup.py` - Remove old handoffs

### 6. Install Script
- `install-global-hooks.sh` - Install Claude Code hooks globally

### 7. /conductor-setup Integration
Phase 8: Continuity Setup
- Create sessions structure
- Install skill
- Platform-specific hook setup

## Phased Delivery

| Phase | Scope | Success Criteria |
|-------|-------|------------------|
| P1 | Skill + directories | `continuity load/save` works |
| P2 | Claude Code hooks | Auto context on SessionStart |
| P3 | Artifact index | `continuity search` works |
| P4 | Amp hooks + setup integration | Full automation |

## Acceptance Criteria

- [ ] LEDGER.md auto-loaded on Claude Code session start
- [ ] Auto-handoff created before compact
- [ ] Modified files tracked in ledger
- [ ] Searchable history via artifact index
- [ ] /conductor-setup Phase 8 creates all structure
- [ ] Works on Claude Code, Amp Code, Codex

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Hook crash blocks Claude | Try/catch + graceful degradation |
| Stale ledger | 24h threshold + auto-archive |
| SQLite corruption | Regenerate from markdown |
| Cross-platform paths | Template + OS detection |

## Dependencies

- Node.js (for TypeScript hooks)
- Python 3 + uv (for artifact scripts)
- SQLite with FTS5 (standard)

## Out of Scope

- Braintrust tracing integration
- External services (Perplexity, etc.)
- MCP harness pattern

## Design Session Reference

This design was created through a Double Diamond design session:
- Thread: T-019b5e5e-a504-774f-9b14-f95a0da40b51
- Date: 2025-12-27
- Phases: DISCOVER → DEFINE → DEVELOP → DELIVER (all complete)
- Party Mode reviews: 4 (with BMad agents)
