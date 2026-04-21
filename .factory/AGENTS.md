# Droid Bootstrap Reference

Use this directory as committed bootstrap and agent-reference infrastructure for Droid-compatible workflows.

## Canonical Separation

- `.maestro/` is runtime state: missions, features, assertions, agents, checkpoints, sessions
- `.factory/` is committed repo infrastructure: init scripts, service manifests, agent-reference docs, and shared guidance
- `skills/built-in/` contains shipped built-in skills

## Skill Lookup

- Runtime agent prompt lookup resolves `.maestro/skills/{agentType}/SKILL.md` first
- If no project-local skill exists, runtime falls back to `skills/built-in/{agentType}/SKILL.md`
- `.factory/skills/` is reference material for authors and reviewers, not the runtime lookup path

## Mission Control

- Use `maestro mission-control --json` for machine-readable output
- Use `maestro mission-control --preview` for read-only terminal previews
- Interactive Mission Control should supervise runtime only when running in TTY mode

## Hooks

- Hooks should prefer `.maestro` when locating project state
- `pretooluse` may inject workflow reminders for `git commit`
- `posttooluse` should append lightweight events to `.maestro/sessions/events.jsonl`

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
