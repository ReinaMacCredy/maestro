# Tech Stack

## Format

- **Agent Skills Specification**: Open standard for portable agent capabilities
  - SKILL.md files with YAML frontmatter
  - References directory for bundled resources
  - Plugin manifest in `.claude-plugin/`

## Languages

- **Markdown**: Primary format for skills, commands, documentation
- **YAML**: Frontmatter metadata in SKILL.md files
- **JSON**: Plugin manifests, configuration files
- **JavaScript**: Shared utilities in `lib/` (e.g., `skills-core.js`)

## Directory Structure

```
maestro/
├── skills/                # Skill directories (SKILL.md + references/)
├── commands/              # Slash command definitions (.md)
├── agents/                # Agent definitions
├── workflows/             # Workflow definitions
├── hooks/                 # Lifecycle hooks
├── lib/                   # Shared utilities
├── templates/             # Templates
├── conductor/             # Planning output (tracks, plans, archive)
├── .beads/                # Beads issue tracking data
├── .claude-plugin/        # Plugin manifest
└── .codex/                # Codex configuration
```

## Compatible Agents

| Agent | Installation Method |
|-------|---------------------|
| Claude Code | `/plugin install` or SETUP_GUIDE.md |
| Amp | SETUP_GUIDE.md agent prompt |
| OpenAI Codex | `$skill-installer ReinaMacCredy/maestro` |
| Cursor | SETUP_GUIDE.md agent prompt |
| GitHub Copilot | Agent Skills support (emerging) |

## Key Dependencies

- **bd CLI**: Beads issue tracking command-line tool
- **bv CLI**: Beads Village multi-agent coordination
- **Git**: Version control for skills and `.beads/` state

## Validation

```bash
# Validate plugin manifest
cat .claude-plugin/plugin.json | jq .

# Validate SKILL.md frontmatter (manual review)
head -20 skills/*/SKILL.md
```

## References

- [Agent Skills Specification](https://agentskills.io/specification)
- [Agent Skills GitHub](https://github.com/agentskills/agentskills)
- [Example Skills](https://github.com/anthropics/skills)
