# Design: Documentation Rewrite

## Problem
Current docs (README, SETUP_GUIDE, TUTORIAL, REFERENCE) are out of sync with project state after Skills Architecture Refactor (gerund naming, 10-phase pipeline, BMAD v6 integration).

## Decision
Full rewrite of all 4 docs prioritizing Claude Code + Amp users.

## Key Changes
1. **Skill naming**: design→designing, beads→tracking, writing-skills→creating-skills
2. **Pipeline**: 10-phase unified (not 8-phase), DS+PL merged
3. **maestro-core**: Thin shim, AGENTS.md owns routing
4. **New topics**: MCPorter toolboxes, BMAD/Oracle behavior, skill authoring

## Structure

| Doc | Purpose | ~Lines |
|-----|---------|--------|
| README.md | First impression, quick install | 100 |
| SETUP_GUIDE.md | Complete setup for CC+Amp | 180 |
| TUTORIAL.md | End-to-end workflow guide | 450 |
| REFERENCE.md | Full command/trigger reference | 300 |

## Principles
- Centralize full tables in REFERENCE, others link
- Claude Code + Amp primary, others secondary
- 10-phase pipeline as canonical
- No redundant content between docs
