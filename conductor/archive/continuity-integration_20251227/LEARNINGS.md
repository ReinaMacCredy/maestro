# LEARNINGS: Continuity Integration

Extracted from design thread T-019b5e5e-a504-774f-9b14-f95a0da40b51 and implementation thread T-019b5eb3-57b1-77cd-b5eb-1c77f64c509b.

## Commands

- `uv run scripts/artifact-index.py` - Build/rebuild SQLite FTS5 index of handoffs
- `uv run scripts/artifact-index.py --verify` - Check index integrity
- `uv run scripts/artifact-query.py <query>` - Search archived handoffs with FTS5
- `uv run scripts/artifact-cleanup.py --dry-run` - Preview handoffs to delete
- `./scripts/install-global-hooks.sh` - Install Claude Code hooks to ~/.claude/hooks/
- `node dist/continuity.js --version` - Verify hooks installation

## Gotchas

- Claude Code hooks must exit 0 even on error (try/catch + graceful exit) to avoid crashing Claude
- LEDGER.md in conductor/sessions/active/ is gitignored (personal state), archive/*.md is committed (shared history)
- Stale ledgers (>24h) are auto-archived on SessionStart to avoid confusion from old context
- FTS5 snippet function: `snippet(handoffs_fts, 2, '>>>', '<<<', '...', 50)` for match highlighting
- artifact-cleanup.py parses dates from filenames (YYYY-MM-DD-HH-MM-trigger.md), not frontmatter
- Concurrent sessions on same codebase may conflict - documented as known limitation (last writer wins)
- Amp Code doesn't support hooks - use manual `continuity load/save/handoff` commands

## Patterns

- **Multi-Platform Architecture**: Common layer (skill + scripts + sessions/) with platform-specific hooks (Claude: TypeScript, Amp: AGENTS.md instructions, Codex: manual only)
- **Global Hooks by Default**: ~/.claude/hooks/ for personal productivity, repo hooks optional via --repo-hooks
- **Handoff as Snapshot**: YAML frontmatter (date, session_id, trigger, status) + markdown body for context transfer
- **Graceful Degradation**: Missing directories created on demand, missing LEDGER.md starts fresh, hooks never block Claude
- **FTS5 Triggers**: SQLite triggers (handoffs_ai, handoffs_ad, handoffs_au) keep FTS5 index in sync with handoffs table
- **Single TypeScript Entry Point**: continuity.ts with command-line switch for hook type (SessionStart, PreCompact, PostToolUse, Stop)
