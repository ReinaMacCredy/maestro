# Product: Maestro

## Vision

A plugin that provides structured AI-assisted development capabilities through portable, reusable skills that work across multiple AI coding agents.

## Target Users

- **Developers using AI agents**: Claude Code, Amp, OpenAI Codex, Cursor, GitHub Copilot, VS Code agents
- **Teams adopting AI workflows**: Organizations building structured AI development practices
- **Skill authors**: Developers creating reusable agent capabilities

## Problems Solved

1. **Context loss across sessions**: AI agents lose context after compaction; Beads provides persistent issue tracking
2. **Unstructured planning**: Features get built without specs; Conductor enforces plan-before-code
3. **Inconsistent quality**: No enforcement of TDD, code review; skills encode best practices
4. **Tool fragmentation**: Different agents need different formats; Agent Skills standard provides interoperability

## Key Goals

- [ ] Provide production-ready skills for planning (Conductor), tracking (Beads), and execution (TDD)
- [ ] Support all major Agent Skills-compatible tools
- [ ] Enable context-driven development with persistent memory
- [ ] Package organizational knowledge into portable, version-controlled skills
- [~] Double Diamond design process with Party Mode multi-agent feedback
- [x] BMAD v6 integration with 16 expert agents (completed 2025-12-27)
- [x] State consolidation: 3→1 state files per track, session state in LEDGER.md (completed 2025-12-27)
- [x] Continuity-Conductor integration: auto-load/handoff at implement/finish (completed 2025-12-27)
- [x] Skill integration: 15→6 skills, 9 merged into conductor/references/ (completed 2025-12-28)
- [x] ~~maestro-core: Central orchestration skill with 5-level hierarchy, HALT/DEGRADE policies (completed 2025-12-29)~~ (removed - routing centralized in AGENTS.md)
- [x] Auto-continuity: Session continuity automatic via workflow entry points for all agents (completed 2025-12-29)
- [x] Auto Oracle Design Review: 6-dimension design audit at CP4 (DELIVER) with platform detection (completed 2026-01-02)
- [x] MCPorter Toolboxes: CLI generation from MCP servers via MCPorter, stored in `toolboxes/` (completed 2026-01-08)
- [x] Unified DS Pipeline: 8-phase pipeline (DS+PL merged), research consolidated from 5→2 hooks (~35s vs ~95s) (completed 2026-01-08)
- [x] Skills Architecture Refactor: Clear ownership (design→designing, beads→tracking), gerund naming, skill-creator+writing-skills merged into creating-skills (completed 2026-01-08)

## Success Metrics

- Skills load correctly across Claude Code, Amp, Codex, Cursor
- Workflows produce consistent, documented outputs
- Users can resume work after context compaction via Beads
- Skills are discoverable and self-documenting

## Non-Goals

- Runtime code execution (skills are instructions + resources, not code)
- Building a new AI agent (skills extend existing agents)
- Replacing existing issue trackers (Beads supplements, doesn't replace GitHub Issues)
