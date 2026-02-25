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
npx skills add ReinaMacCredy/maestro --skill planning --agent claude-code --agent amp
```

## SKILL.md Requirements

Each skill must follow Agent Skills core constraints:

- `SKILL.md` exists in each skill directory.
- YAML frontmatter includes:
  - `name` (required)
  - `description` (required)
- `name` matches `^[a-z0-9:-]{1,64}$` (colons allowed for namespace prefixes like `maestro:implement`).
- `name` exactly matches the parent directory name.
- `description` is non-empty and <= 1024 characters.

## Validation

Validate skills using your preferred lint/CI flow (for example, the `skills.sh` tooling and CI checks in your host environment).
