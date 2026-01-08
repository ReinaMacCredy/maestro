# Agent Mail CLI Reference

CLI tool for agent coordination via Agent Mail MCP, generated using MCPorter.

## Location

`toolboxes/agent-mail/agent-mail.js`

## Quick Start

```bash
# Check server status
toolboxes/agent-mail/agent-mail.js health-check

# List all tools
toolboxes/agent-mail/agent-mail.js --help
```

## Common Commands

### Session Setup

```bash
# Start a session (project + agent + inbox in one call)
toolboxes/agent-mail/agent-mail.js macro-start-session \
  human_key:/path/to/project \
  program:claude-code \
  model:opus-4.5
```

### Messaging

```bash
# Send message
toolboxes/agent-mail/agent-mail.js send-message \
  project_key:/path/to/project \
  sender_name:BlueLake \
  to:'["GreenCastle"]' \
  subject:"Status update" \
  body_md:"Work complete"

# Fetch inbox
toolboxes/agent-mail/agent-mail.js fetch-inbox \
  project_key:/path/to/project \
  agent_name:BlueLake

# Reply to message
toolboxes/agent-mail/agent-mail.js reply-message \
  project_key:/path/to/project \
  message_id:123 \
  sender_name:BlueLake \
  body_md:"Acknowledged"
```

### Agent Management

```bash
# Register agent
toolboxes/agent-mail/agent-mail.js register-agent \
  project_key:/path/to/project \
  program:claude-code \
  model:opus-4.5 \
  task_description:"Working on feature X"

# Who is an agent
toolboxes/agent-mail/agent-mail.js whois \
  project_key:/path/to/project \
  agent_name:BlueLake
```

### File Reservations

```bash
# Reserve files
toolboxes/agent-mail/agent-mail.js file-reservation-paths \
  project_key:/path/to/project \
  agent_name:BlueLake \
  paths:'["src/api/*.py"]' \
  ttl_seconds:3600

# Release reservations
toolboxes/agent-mail/agent-mail.js release-file-reservations \
  project_key:/path/to/project \
  agent_name:BlueLake
```

## Argument Syntax

```bash
# Colon style
command key:value

# Equals style
command key=value

# JSON for arrays/objects
command paths:'["file1.ts", "file2.ts"]'
```

## Environment Variables

- `AGENT_MAIL_TOKEN` - Bearer token for authentication (if required)

## See Also

- [Agent Mail MCP Documentation](../../orchestrator/references/agent-mail.md)
- [Orchestrator Workflow](../../orchestrator/references/workflow.md)
