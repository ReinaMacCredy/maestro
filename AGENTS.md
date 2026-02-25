# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is
Maestro is an AI agent workflow skillpack focused on plan-first delivery, track-based implementation, and post-change verification. It targets Claude Code, Amp, and other agent runtimes through a shared skills layout and compatibility paths.

## Tech Stack
- Markdown-driven skill definitions (`skills/*/SKILL.md`)
- Bash scripts for hooks and workflow automation (`.claude/scripts/*.sh`, `scripts/*.sh`)
- JSON configuration for plugin metadata and hooks (`.claude-plugin/*.json`, `.claude/hooks/hooks.json`)
- GitHub Actions for validation and release automation (`.github/workflows/*.yml`)

## How to Build, Test, and Run
- Install all skills locally: `npx skills add ReinaMacCredy/maestro`
- List installable skills: `npx skills add ReinaMacCredy/maestro --list`
- Install a subset of skills/agents: `npx skills add ReinaMacCredy/maestro --skill planning --agent claude-code --agent amp`
- Run hook smoke tests: `bash scripts/test-hooks.sh`
- Check shell script syntax: `bash -n scripts/*.sh`
- Validate release metadata versions match:
  `PLUGIN_VERSION=$(jq -r '.version' .claude-plugin/plugin.json); MARKETPLACE_VERSION=$(jq -r '.plugins[0].version' .claude-plugin/marketplace.json); [ "$PLUGIN_VERSION" = "$MARKETPLACE_VERSION" ]`
- Preview unreleased changelog: `git-cliff --unreleased --strip header`
- Regenerate changelog: `git-cliff -o CHANGELOG.md`

## Tooling
- `skills.sh` via `npx skills add` is the primary install path for this repository.
- `git-cliff` drives changelog generation in CI and local release prep.
- `jq` is required for version sync checks and release metadata updates.

## Rules
- Treat `skills/` as the canonical skill source; keep compatibility paths aligned with it.
- Keep `.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json` versions in sync.
- Use conventional commit prefixes (`feat:`, `fix:`, etc.) so changelog and release automation classify changes correctly.
- When changing hook behavior in `.claude/scripts/`, update and run `scripts/test-hooks.sh` before finishing.
- Run syntax and smoke checks before opening or merging a PR.

## Reference Docs
- `.maestro/context/building_the_project.md` -- installation, packaging, changelog, and release commands with source references.
- `.maestro/context/running_tests.md` -- local validation/test commands and CI parity checks.
