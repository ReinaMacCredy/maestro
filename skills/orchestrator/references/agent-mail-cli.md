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
  --human-key /path/to/project \
  --program claude-code \
  --model opus-4.5
```

### Messaging

```bash
# Send message
toolboxes/agent-mail/agent-mail.js send-message \
  --project-key /path/to/project \
  --sender-name BlueLake \
  --to '["GreenCastle"]' \
  --subject "Status update" \
  --body-md "Work complete"

# Fetch inbox
toolboxes/agent-mail/agent-mail.js fetch-inbox \
  --project-key /path/to/project \
  --agent-name BlueLake

# Reply to message
toolboxes/agent-mail/agent-mail.js reply-message \
  --project-key /path/to/project \
  --message-id 123 \
  --sender-name BlueLake \
  --body-md "Acknowledged"
```

### Agent Management

```bash
# Register agent
toolboxes/agent-mail/agent-mail.js register-agent \
  --project-key /path/to/project \
  --program claude-code \
  --model opus-4.5 \
  --task-description "Working on feature X"

# Who is an agent
toolboxes/agent-mail/agent-mail.js whois \
  --project-key /path/to/project \
  --agent-name BlueLake
```

### File Reservations

```bash
# Reserve files
toolboxes/agent-mail/agent-mail.js file-reservation-paths \
  --project-key /path/to/project \
  --agent-name BlueLake \
  --paths '["src/api/*.py"]' \
  --ttl-seconds 3600

# Release reservations
toolboxes/agent-mail/agent-mail.js release-file-reservations \
  --project-key /path/to/project \
  --agent-name BlueLake
```

## Argument Syntax

The CLI uses standard `--flag value` format:

```bash
# Flag with value (space-separated)
command --flag value

# JSON for arrays/objects
command --paths '["file1.ts", "file2.ts"]'

# Boolean flags
command --include-bodies true

# String values with spaces (use quotes)
command --body-md "Message with spaces"
```

### Parameter Naming

CLI flags use kebab-case (hyphens), which map to the MCP's snake_case parameters:

| MCP Parameter | CLI Flag |
|---------------|----------|
| `project_key` | `--project-key` |
| `sender_name` | `--sender-name` |
| `body_md` | `--body-md` |
| `include_bodies` | `--include-bodies` |
| `ttl_seconds` | `--ttl-seconds` |

## Environment Variables

- `AGENT_MAIL_TOKEN` - Bearer token for authentication (if required)

## See Also

- [Agent Mail MCP Documentation](../../orchestrator/references/agent-mail.md)
- [Orchestrator Workflow](../../orchestrator/references/workflow.md)
