# Design: MCPorter Toolboxes Integration

## Problem Statement

Maestro skills currently rely on MCP runtime for Agent Mail coordination. We want to convert MCP servers to standalone CLIs using MCPorter, stored in `toolboxes/`, accessible to all skills. Agent Mail is the first conversion.

## Solution Overview

Add a shared `toolboxes/` directory containing generated CLIs from MCP servers using MCPorter. Any skill can call these tools directly without MCP runtime dependency.

## Architecture

```
.claude/
├── skills/
│   ├── maestro-core/
│   │   └── references/
│   │       └── toolboxes.md      # How skills use toolboxes
│   └── orchestrator/
│       └── references/
│           └── agent-mail-cli.md # Agent Mail CLI reference
└── toolboxes/
    ├── agent-mail/               # Per-tool subfolder
    │   ├── agent-mail.js         # Generated CLI (executable)
    │   └── agent-mail.ts         # Source template (for regeneration)
    ├── mcporter.json             # MCPorter config for all servers
    └── README.md                 # Toolboxes overview
```

## Implementation Details

### 1. MCPorter Configuration

File: `toolboxes/mcporter.json`

```json
{
  "mcpServers": {
    "agent-mail": {
      "type": "http",
      "url": "http://127.0.0.1:8765/mcp/",
      "headers": {
        "Authorization": "Bearer ${AGENT_MAIL_TOKEN}"
      }
    }
  }
}
```

### 2. Generation Command

```bash
# Install mcporter (one-time)
npm install -g mcporter

# Generate Agent Mail CLI
mkdir -p toolboxes/agent-mail
npx mcporter generate-cli agent-mail \
  --name agent-mail \
  --output toolboxes/agent-mail/agent-mail.ts \
  --bundle toolboxes/agent-mail/agent-mail.js

# Make executable
chmod +x toolboxes/agent-mail/agent-mail.js
```

### 3. CLI Usage

```bash
# List available tools
toolboxes/agent-mail/agent-mail.js --help

# Send message
toolboxes/agent-mail/agent-mail.js send_message \
  project_key:/path/to/project \
  sender_name:BlueLake \
  to:GreenCastle \
  subject:"Status update" \
  body_md:"Work complete"

# Fetch inbox
toolboxes/agent-mail/agent-mail.js fetch_inbox \
  project_key:/path/to/project \
  agent_name:BlueLake \
  --json

# Register agent
toolboxes/agent-mail.js register_agent \
  project_key:/path/to/project \
  program:claude-code \
  model:opus-4.5
```

### 4. Skill Integration

Skills reference toolboxes via relative path from project root:

```markdown
## Agent Coordination

Use the Agent Mail CLI for messaging:

\`\`\`bash
toolboxes/agent-mail.js send_message \
  project_key:$PROJECT_KEY \
  sender_name:$AGENT_NAME \
  to:orchestrator \
  subject:"Task complete" \
  body_md:"Finished bead-123"
\`\`\`
```

### 5. Documentation Updates

#### maestro-core/references/toolboxes.md

```markdown
# Toolboxes

Shared CLI tools generated from MCP servers using MCPorter.

## Location

`toolboxes/`

## Available Tools

| CLI | Source MCP | Description |
|-----|------------|-------------|
| agent-mail.js | mcp-agent-mail | Agent coordination and messaging |

## Usage Pattern

\`\`\`bash
toolboxes/<tool>.js <command> arg1:value1 arg2:value2
\`\`\`

## Regenerating Tools

\`\`\`bash
npx mcporter generate-cli --from toolboxes/<tool>.js
\`\`\`
```

## Adding Future MCP Servers

To add a new MCP server (e.g., Linear):

1. Add to `toolboxes/mcporter.json`:
   ```json
   {
     "linear": {
       "type": "http",
       "url": "https://mcp.linear.app/mcp",
       "headers": {
         "Authorization": "Bearer ${LINEAR_API_KEY}"
       }
     }
   }
   ```

2. Generate CLI:
   ```bash
   npx mcporter generate-cli linear \
     --output toolboxes/linear.ts \
     --bundle toolboxes/linear.js
   ```

3. Document in skill references

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Location | `toolboxes/` | Sibling to skills, shared resource |
| Format | Bundled .js | Works with Node, no compile step |
| Auth | Env vars (`${VAR}`) | No secrets in repo |
| Ownership | All skills | Not a separate skill, integrated capability |
| CLI style | Direct execution | Matches MCPorter's pattern |

## Dependencies

- Node.js (for running generated CLIs)
- MCPorter (`npm install -g mcporter` or `npx mcporter`)
- Agent Mail MCP server running (for generation)

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Agent Mail server must be running for generation | Document requirement, add health check |
| CLI size may be large | MCPorter bundles are optimized, ~500KB typical |
| Auth token exposure | Use env vars, never commit tokens |

## Success Criteria

1. `agent-mail.js` CLI generated and working
2. Skills can call CLI without MCP runtime
3. Pattern documented for future MCP conversions
4. Regeneration command preserved in .ts file
