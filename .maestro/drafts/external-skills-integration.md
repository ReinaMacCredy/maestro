# External Skills Integration - Draft

> Status: Interview in progress
> Created: 2026-02-06

## Context

Maestro currently has an internal skill/agent system:
- 6 built-in agents (prometheus, orchestrator, kraken, spark, oracle, explore)
- Skills defined in `.claude/skills/maestro/SKILL.md`
- Triggered via `@` mentions (`@tdd`, `@spark`, etc.)
- Team-based workflow via Agent Teams

The user wants to integrate external skills from other sources.

## Confirmed Requirements

*(To be filled during interview)*

## Open Questions

1. **What external skills?** - Which specific skills/plugins do you want to integrate?
   - Anthropic official skills?
   - v0 (Vercel)?
   - Other Claude Code plugins?
   - Custom/internal skills?

2. **Integration direction** - How should the integration work?
   - Maestro calling external skills as part of workflows?
   - External skills calling Maestro agents?
   - Both directions?

3. **Trigger mechanism** - How should external skills be invoked?
   - Via @ mentions like internal agents?
   - Automatic routing based on task type?
   - Explicit selection in plans?

4. **Conflict resolution** - What happens when skills overlap?
   - Priority order?
   - User selection?
   - Merge capabilities?

5. **Scope boundaries** - What's explicitly out of scope?

## Technical Decisions

*(To be filled during interview)*

## Scope Boundaries

**IN:**
*(To be determined)*

**OUT:**
*(To be determined)*
