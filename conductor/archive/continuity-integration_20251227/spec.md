# Continuity Integration Specification

## 1. Overview

### 1.1 Purpose
Integrate Continuous-Claude patterns into Maestro workflow to provide automatic session state preservation across sessions and compactions.

### 1.2 Background
Current Maestro workflow relies on manual session management via beads. When sessions get long or compact, context is lost. Users must manually track state and resume work. This specification defines an automated solution.

### 1.3 Goals
1. Auto-load session context on session start (Claude Code)
2. Auto-save state before compaction
3. Track modified files during work
4. Provide searchable session history
5. Support multiple platforms (Claude Code, Amp, Codex)

### 1.4 Non-Goals
- Braintrust tracing integration
- External service integration (Perplexity, Firecrawl)
- MCP harness pattern from Continuous-Claude

## 2. Functional Requirements

### 2.1 Data Storage

#### FR-2.1.1: Sessions Directory Structure
- System MUST create `conductor/sessions/active/` for current session state
- System MUST create `conductor/sessions/archive/` for archived handoffs
- System MUST create `conductor/.cache/` for SQLite index
- `LEDGER.md` in active/ MUST be gitignored (personal state)
- Handoffs in archive/ MUST be committed (shared history)

#### FR-2.1.2: LEDGER.md Format
LEDGER.md MUST contain:
- `updated`: ISO 8601 timestamp
- `session_id`: Current session identifier
- `platform`: claude | amp | codex
- `Active Task`: Current bead reference
- `Goal`: Session objective
- `State`: Done/Now/Next items
- `Key Decisions`: Choices made
- `Working Set`: Branch and modified files
- `Open Questions`: Unresolved items

#### FR-2.1.3: Handoff Format
Handoff files MUST contain YAML frontmatter:
- `date`: ISO 8601 timestamp
- `session_id`: Source session
- `trigger`: manual | pre-compact | session-end | stale
- `status`: complete | interrupted | handoff

### 2.2 Skill Interface

#### FR-2.2.1: Skill Triggers
The `continuity` skill MUST respond to:
- `continuity load` / `load context` - Load LEDGER.md + last handoff
- `continuity save` / `save state` - Update LEDGER.md
- `continuity handoff` / `create handoff` - Archive current state
- `continuity status` - Display health check
- `continuity search <query>` - Search archived handoffs

#### FR-2.2.2: Skill Replaces session-compaction
- `skills/session-compaction/` MUST be deleted
- `skills/continuity/` MUST be created
- All session-compaction triggers MUST be handled by continuity

### 2.3 Claude Code Hooks

#### FR-2.3.1: SessionStart Hook
- Hook MUST trigger on: startup, resume, clear, compact
- Hook MUST load LEDGER.md if exists
- Hook MUST check for stale ledger (>24h)
- Hook MUST archive stale ledger and create fresh
- Hook MUST load last handoff from archive
- Hook MUST inject context via `additionalContext`

#### FR-2.3.2: PreCompact Hook
- Hook MUST trigger on: manual, auto
- Hook MUST create handoff before compaction
- Hook MUST log action to stderr

#### FR-2.3.3: PostToolUse Hook
- Hook MUST trigger on: Edit, Write
- Hook MUST track modified file in LEDGER.md Working Set
- Hook MUST update timestamp

#### FR-2.3.4: Stop Hook
- Hook MUST archive session on clean exit
- Hook MUST be silent (no blocking)

### 2.4 Amp Code Hooks

#### FR-2.4.1: amp.hooks Configuration
- System MUST add hooks to `~/.config/amp/AGENTS.md`
- Hook MUST trigger reminder on tool:post-execute for edit_file/create_file
- Message MUST prompt user to run `continuity save`

#### FR-2.4.2: Session Protocol
- AGENTS.md MUST include instructions to run `continuity load` at session start
- AGENTS.md MUST include instructions to run `continuity handoff` before ending

### 2.5 Artifact Index

#### FR-2.5.1: SQLite Database
- Index MUST use SQLite with FTS5 extension
- Index MUST be stored in `conductor/.cache/artifact-index.db`
- Index MUST be gitignored

#### FR-2.5.2: Index Schema
- Table `handoffs`: id, filename, date, trigger, session_id, status, summary, content, indexed_at
- Virtual table `handoffs_fts`: FTS5 on id, summary, content

#### FR-2.5.3: Search Functionality
- `artifact-query.py` MUST support FTS5 search syntax
- Results MUST include snippet with match highlights
- Results MUST be sorted by relevance

#### FR-2.5.4: Cleanup Policy
- `artifact-cleanup.py` MUST delete handoffs older than configurable threshold (default 30 days)
- Cleanup MUST update SQLite index to remove deleted entries

### 2.6 Installation

#### FR-2.6.1: Global Hooks Install
- `install-global-hooks.sh` MUST install TypeScript hooks to `~/.claude/hooks/`
- Script MUST check for Node.js and npm
- Script MUST build TypeScript
- Script MUST merge hooks into existing `settings.json`

#### FR-2.6.2: /conductor-setup Integration
- Phase 8 MUST create sessions directory structure
- Phase 8 MUST detect platform (Claude/Amp/Codex)
- Phase 8 MUST install platform-specific hooks
- Phase 8 MUST remove session-compaction skill
- Phase 8 MUST create continuity skill

## 3. Non-Functional Requirements

### 3.1 Performance
- Hooks MUST complete within 5 seconds
- SQLite queries MUST complete within 1 second for <1000 handoffs

### 3.2 Reliability
- Hooks MUST NOT crash Claude (graceful degradation required)
- All hook handlers MUST use try/catch
- Missing directories MUST be created on demand (fallback)

### 3.3 Security
- No secrets in LEDGER.md (gitignored, but still)
- No network calls in hooks
- SQLite index is local only

### 3.4 Compatibility
- MUST work on macOS and Linux
- SHOULD work on Windows (with path adjustments)
- MUST support Claude Code, Amp Code, Codex

## 4. Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-1 | LEDGER.md auto-loaded on Claude Code session start | Start session, verify context injected |
| AC-2 | Auto-handoff created before compact | Run /compact, check archive/ |
| AC-3 | Modified files tracked in ledger | Edit file, check Working Set |
| AC-4 | Search works | Run `uv run scripts/artifact-query.py test` |
| AC-5 | /conductor-setup creates structure | Run on fresh repo |
| AC-6 | Skill triggers work | Say "continuity load" |
| AC-7 | Graceful degradation | Delete LEDGER.md, start session |
| AC-8 | Amp reminders work | Edit file in Amp, see message |

## 5. Dependencies

### 5.1 Required
- Node.js 18+ (TypeScript hooks)
- Python 3.8+ (artifact scripts)
- uv (Python package manager)
- SQLite 3.x with FTS5

### 5.2 Optional
- jq (for settings.json merge)

## 6. Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Hook crashes Claude | Low | High | Try/catch + exit 0 |
| Stale ledger causes confusion | Medium | Medium | 24h auto-archive |
| Concurrent sessions conflict | Low | Medium | Document limitation |
| Windows path issues | Medium | Low | Template + OS detection |
