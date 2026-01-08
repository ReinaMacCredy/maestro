# Spec: MCPorter Toolboxes Integration

## Overview

Add `toolboxes/` directory for CLI tools generated from MCP servers using MCPorter. Agent Mail is the first conversion.

## Requirements

### Functional

1. **Toolboxes Directory**
   - Create `toolboxes/` as shared resource
   - Accessible from all skills

2. **Agent Mail CLI**
   - Generate `agent-mail.js` from Agent Mail MCP
   - Support all Agent Mail tools (send_message, fetch_inbox, register_agent, etc.)
   - Use env var `${AGENT_MAIL_TOKEN}` for auth

3. **MCPorter Config**
   - Create `mcporter.json` for server definitions
   - Support env var interpolation for secrets

4. **Documentation**
   - Add `toolboxes.md` to maestro-core references
   - Document CLI usage patterns
   - Document how to add future MCP servers

### Non-Functional

1. **No secrets in repo** - Auth via env vars only
2. **Regenerable** - Keep .ts template for regeneration
3. **Portable** - Works on any system with Node.js

## Interfaces

### CLI Invocation

```bash
toolboxes/agent-mail.js <tool> [args...]
```

### Argument Syntax (MCPorter standard)

```bash
# Colon-delimited
agent-mail.js send_message to:BlueLake subject:"Hello"

# Equals-delimited
agent-mail.js send_message to=BlueLake subject="Hello"

# Function-call style
agent-mail.js 'send_message(to: "BlueLake", subject: "Hello")'
```

### Config Schema

```json
{
  "mcpServers": {
    "<name>": {
      "type": "http",
      "url": "<mcp-endpoint>",
      "headers": {
        "Authorization": "Bearer ${ENV_VAR}"
      }
    }
  }
}
```

## Out of Scope

- Replacing MCP runtime entirely (CLI is alternative, not replacement)
- Auto-generating CLIs on install (manual generation)
- CI/CD integration for CLI updates

## Dependencies

- MCPorter (`npx mcporter`)
- Node.js runtime
- Agent Mail MCP server (for generation)

## Acceptance Criteria

1. [ ] `toolboxes/` directory exists
2. [ ] `mcporter.json` config present with agent-mail definition
3. [ ] `agent-mail.js` CLI generated and executable
4. [ ] CLI can send messages when Agent Mail server is running
5. [ ] `toolboxes.md` documentation in maestro-core
6. [ ] Pattern documented for future MCP conversions
