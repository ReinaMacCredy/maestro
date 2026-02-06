# Customization

How to adapt Maestro to your project.

## Agent Models

Each agent has a default model set in its frontmatter. To change:

1. Open `.claude/agents/<agent-name>.md`
2. Change the `model` field in the frontmatter

| Agent | Default | Alternative |
|-------|---------|-------------|
| prometheus | sonnet | opus (more thorough interviews) |
| orchestrator | sonnet | opus (better coordination) |
| kraken | sonnet | opus (complex implementations) |
| spark | sonnet | haiku (faster for simple fixes) |
| oracle | opus | sonnet (cheaper, less thorough) |
| explore | sonnet | haiku (faster searches) |
| wisdom-synthesizer | haiku | sonnet (deeper analysis) |
| progress-reporter | haiku | sonnet (more detailed reports) |

## Test Runner

Maestro defaults to detecting your project's test runner. To override, add to your `CLAUDE.md`:

```markdown
## Testing
- Test command: `bun test`
- Test file pattern: `*.test.ts`
```

Kraken will read `CLAUDE.md` and follow these conventions.

## Hook Customization

### Disabling a Hook

Remove or comment out the hook entry in `.claude/hooks/hooks.json` and `.claude/settings.json`.

### Adding Custom Hooks

Add entries to the `hooks` arrays in `hooks.json`:

```json
{
  "matcher": "Write",
  "hooks": [
    { "type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/scripts/your-hook.sh" }
  ]
}
```

Mirror the entry in `.claude/settings.json` with `$CLAUDE_PROJECT_DIR` paths.

### Hook Script Pattern

All hooks follow this pattern:

```bash
#!/bin/bash
input=$(cat)  # Read stdin JSON payload

# Extract fields with jq
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null)

# Your logic here

# PreToolUse: approve or block
echo '{"decision":"approve"}'
# OR
echo '{"decision":"block","reason":"Why it was blocked"}'

# PostToolUse: advisory message
echo '{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"Your message"}}'
```

## Wisdom Files

Wisdom files in `.maestro/wisdom/` are Markdown files with learnings from completed work cycles. You can:

- **Add manual wisdom**: Create `.maestro/wisdom/<topic>.md` with patterns you want agents to remember
- **Edit existing wisdom**: Refine auto-generated wisdom files
- **Organize by topic**: Use descriptive filenames like `testing-patterns.md`, `api-conventions.md`

## Plan Templates

Customize the plan template by modifying the `/plan-template` skill at `.claude/skills/plan-template/SKILL.md`.

Add project-specific sections:
- **Migration steps** for database changes
- **API documentation** for endpoint changes
- **Rollback plan** for production deployments

## Project Conventions Skill

The `project-conventions` skill auto-discovers your project setup. To improve detection, ensure your project has standard config files:

- `package.json` / `pyproject.toml` / `Cargo.toml`
- Linter configs (`.eslintrc`, `biome.json`, etc.)
- `CLAUDE.md` with project-specific rules
