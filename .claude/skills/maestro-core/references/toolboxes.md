# Toolboxes

CLI tools generated from MCP servers using [MCPorter](https://github.com/steipete/mcporter).

## Location

`.claude/toolboxes/`

## Available Tools

| CLI | Source | Description |
|-----|--------|-------------|
| `agent-mail/agent-mail.js` | mcp-agent-mail | Agent coordination and messaging |

## Usage

```bash
# From project root
.claude/toolboxes/<tool>/<tool>.js <command> [args...]

# Example
.claude/toolboxes/agent-mail/agent-mail.js health-check
```

## Argument Syntax

MCPorter CLIs support multiple styles:

```bash
# Colon-delimited
agent-mail.js send_message to:BlueLake subject:"Hello"

# Equals-delimited
agent-mail.js send_message to=BlueLake subject="Hello"

# Function-call style
agent-mail.js 'send_message(to: "BlueLake", subject: "Hello")'
```

## Adding New Tools

1. Add to `.claude/toolboxes/mcporter.json`
2. Run:
   ```bash
   mkdir -p .claude/toolboxes/<name>
   npx mcporter generate-cli <name> --bundle .claude/toolboxes/<name>/<name>.js
   chmod +x .claude/toolboxes/<name>/<name>.js
   ```

## Regenerating

```bash
npx mcporter generate-cli --from .claude/toolboxes/<tool>/<tool>.js
```

## See Also

- [.claude/toolboxes/README.md](../../../.claude/toolboxes/README.md) - Full documentation
- Load the [orchestrator skill](../../orchestrator/SKILL.md) for Agent Mail CLI reference
