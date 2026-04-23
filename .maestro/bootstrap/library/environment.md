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

- Other agent CLIs may exist on the machine. Mission Control itself must not spawn them directly, but `maestro handoff` can launch Codex or Claude when the operator requests a fresh handoff.

## Product Runtime Storage

- Mission Control runtime state belongs under `.maestro/missions/{missionId}/`
- Per-project agent skills for generated prompts belong under `.maestro/skills/{agentType}/SKILL.md`
- Native handoff launch artifacts live under `.maestro/launches/`
- Legacy handoff artifacts may still exist under `.maestro/handoffs/`, but they are no longer read

## Repository Infrastructure

- `.factory/` in this repository is mission infrastructure for agents and validators, not product runtime storage
- `.factory/` should stay committed; `.maestro/missions/` should stay ignored
- `.factory/skills/` is authoring/reference material for repo-local agent guidance
- Runtime agent prompt lookup should resolve `.maestro/skills/{agentType}/SKILL.md` first, then `skills/built-in/{agentType}/SKILL.md`

## Environment Variables

No new environment variables are required for Mission Control.

## Platform Notes

- Current validation readiness was confirmed on macOS with 10 CPU cores and 64 GB RAM
- Mission Control is a CLI-only feature; it should not assume browser tooling or local service ports
