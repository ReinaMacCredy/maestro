# Claude Code Setup Templates

Complete templates for configuring Claude Code in any project.

## Contents

| File | Purpose |
|------|---------|
| [CLAUDE.md](CLAUDE.md) | Project context template |
| [AGENTS.md](AGENTS.md) | Agent workflow instructions |
| [SETUP.md](SETUP.md) | Setup guide |
| [.claude/rules/safety.md](.claude/rules/safety.md) | Safety constraints |
| [.claude/skills/example-skill/SKILL.md](.claude/skills/example-skill/SKILL.md) | Skill template |
| [.claude/commands/audit.md](.claude/commands/audit.md) | Command template |

## Quick Start

```bash
# Copy to your project
cp -r templates/claude-code-setup/* /path/to/your/project/

# Then customize:
# 1. Edit CLAUDE.md with your project details
# 2. Edit AGENTS.md with your workflow
# 3. Add rules to .claude/rules/
# 4. Create skills in .claude/skills/
# 5. Add commands to .claude/commands/
```

See [SETUP.md](SETUP.md) for detailed instructions.

## Configuration Layers

```
CLAUDE.md           → Project context (auto-loaded)
AGENTS.md           → Workflow instructions (auto-loaded)
.claude/rules/      → Constraints (auto-loaded, path-filtered)
.claude/skills/     → Capabilities (on-demand discovery)
.claude/commands/   → Slash commands (user-triggered)
```
