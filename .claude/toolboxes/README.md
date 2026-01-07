# Toolboxes

CLI tools generated from MCP servers using [MCPorter](https://github.com/steipete/mcporter).

## Available Tools

| CLI | Source | Description |
|-----|--------|-------------|
| `agent-mail/agent-mail.js` | mcp-agent-mail | Agent coordination and messaging |

## Usage

```bash
# Run any tool
.claude/toolboxes/<tool>/<tool>.js <command> [args...]

# Example: send message
.claude/toolboxes/agent-mail/agent-mail.js send_message \
  project_key:/path/to/project \
  sender_name:BlueLake \
  to:GreenCastle \
  subject:"Hello" \
  body_md:"Test message"

# Get help
.claude/toolboxes/agent-mail/agent-mail.js --help
```

## Argument Syntax

MCPorter CLIs support multiple argument styles:

```bash
# Colon-delimited
agent-mail.js fetch_inbox agent_name:BlueLake

# Equals-delimited
agent-mail.js fetch_inbox agent_name=BlueLake

# Function-call style
agent-mail.js 'fetch_inbox(agent_name: "BlueLake")'
```

## Adding New Tools

1. Add server to `mcporter.json`:
   ```json
   {
     "mcpServers": {
       "new-server": {
         "type": "http",
         "url": "https://example.com/mcp",
         "headers": {
           "Authorization": "Bearer ${ENV_VAR}"
         }
       }
     }
   }
   ```

2. Generate CLI:
   ```bash
   mkdir -p .claude/toolboxes/new-server
   npx mcporter generate-cli new-server \
     --output .claude/toolboxes/new-server/new-server.ts \
     --bundle .claude/toolboxes/new-server/new-server.js
   chmod +x .claude/toolboxes/new-server/new-server.js
   ```

3. Update this README

## Regenerating Tools

Each `.ts` file contains regeneration metadata:

```bash
npx mcporter generate-cli --from .claude/toolboxes/agent-mail/agent-mail.js
```

## Requirements

- Node.js (for running CLIs)
- MCPorter (`npx mcporter` or `npm install -g mcporter`)
- MCP server running (only needed during generation)
