# Toolboxes

CLI tools generated from MCP servers using [MCPorter](https://github.com/steipete/mcporter).

## Location

`toolboxes/`

## Available Tools

| CLI | Source | Description |
|-----|--------|-------------|
| `agent-mail/agent-mail.js` | mcp-agent-mail | Agent coordination and messaging |

## Usage

```bash
# From project root
toolboxes/<tool>/<tool>.js <command> [args...]

# Example
toolboxes/agent-mail/agent-mail.js health-check
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

1. Add to `toolboxes/mcporter.json`
2. Run:
   ```bash
   mkdir -p toolboxes/<name>
   npx mcporter generate-cli <name> --bundle toolboxes/<name>/<name>.js
   chmod +x toolboxes/<name>/<name>.js
   ```

## Regenerating

```bash
npx mcporter generate-cli --from toolboxes/<tool>/<tool>.js
```

## See Also

- [toolboxes/README.md](../../../toolboxes/README.md) - Full documentation
- Load the [orchestrator skill](../../orchestrator/SKILL.md) for Agent Mail CLI reference
