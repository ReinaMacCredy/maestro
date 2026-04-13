# Environment

Environment variables, dependencies, and repository-scoped setup notes for the Mission Control implementation.

**What belongs here:** Required tools, runtime storage locations, env/config notes, platform caveats.
**What does NOT belong here:** Command shortcuts or service processes (use `.factory/services.yaml`).

---

## Required Tools

- **Bun** for runtime, build, and tests
- **TypeScript / `tsc`** via project devDependencies
- **Git** because integration tests and several commands require running inside a repository

## Optional Tools

- **CASS** remains optional and only matters for existing handoff search flows; Mission Control itself must not depend on it
- Other agent CLIs may exist on the machine, but Mission Control must not spawn them directly

## Product Runtime Storage

- Mission Control runtime state belongs under `.maestro/missions/{missionId}/`
- Per-project worker skills for generated prompts belong under `.maestro/skills/{workerType}/SKILL.md`
- Existing handoff/session data remains under `.maestro/handoffs/`

## Repository Infrastructure

- `.factory/` in this repository is mission infrastructure for workers and validators, not product runtime storage
- `.factory/` should stay committed; `.maestro/missions/` should stay ignored
- `.factory/skills/` is authoring/reference material for repo-local worker guidance
- Runtime worker prompt lookup should resolve `.maestro/skills/{workerType}/SKILL.md` first, then `skills/built-in/{workerType}/SKILL.md`

## Environment Variables

No new environment variables are required for Mission Control.

## Platform Notes

- Current validation readiness was confirmed on macOS with 10 CPU cores and 64 GB RAM
- Mission Control is a CLI-only feature; it should not assume browser tooling or local service ports
