# Universal Agent Skills

This repository is packaged for the open Agent Skills ecosystem (`agentskills.io`) and the `skills.sh` installer.

## Canonical Layout

- `skills/` is the canonical, agent-agnostic source of truth.
- `.claude/skills` is kept as a compatibility path for Claude Code.
- `.agents/skills` is kept as a compatibility path for Amp/OpenCode/Replit and other universal-agent conventions.
- `.github/skills` points to `skills/` for GitHub Copilot compatibility.

## Install with `skills.sh`

```bash
# List skills in this repository
npx skills add ReinaMacCredy/maestro --list

# Install all skills for detected local agents
npx skills add ReinaMacCredy/maestro

# Install a specific skill for specific agents
npx skills add ReinaMacCredy/maestro --skill plan-maestro --agent claude-code --agent amp
```

## SKILL.md Requirements

Each skill must follow Agent Skills core constraints:

- `SKILL.md` exists in each skill directory.
- YAML frontmatter includes:
  - `name` (required)
  - `description` (required)
- `name` matches `^[a-z0-9-]{1,64}$`.
- `name` exactly matches the parent directory name.
- `description` is non-empty and <= 1024 characters.

## Validation

Run the repo validator before publishing:

```bash
./scripts/validate-agent-skills.sh
```

This validates all skills under `skills/*/SKILL.md`.
