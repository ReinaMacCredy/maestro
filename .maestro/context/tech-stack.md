# Tech Stack

## Languages
- Markdown (skill definitions, specs, plans, documentation)
- Bash (hook scripts, validation, session management)
- JSON (plugin metadata, settings, state files)

## Frameworks
- Claude Code skills format (SKILL.md + reference/ pattern)
- Claude Code agent definitions (.claude/agents/*.md)
- Claude Code hooks system (.claude/hooks/hooks.json)

## Tools & Infrastructure
- Package manager: npx (skills installer via `npx skills add`)
- Version management: jq (plugin.json / marketplace.json sync)
- Changelog: git-cliff (conventional commit changelog generation)
- CI/CD: GitHub Actions (validate.yml, release.yml)
- Testing: bash scripts (test-hooks.sh, bash -n syntax checks)
- Version control: git (conventional commits, git notes for task summaries)
