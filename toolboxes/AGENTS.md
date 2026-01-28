# toolboxes

## Purpose
CLI tools generated from MCP servers using MCPorter, providing agent coordination and autonomous execution capabilities.

## Key Directories

| Directory | Purpose |
|-----------|---------|
| agent-mail/ | Asynchronous messaging and coordination between agents |
| ralph/ | Autonomous while-loop agent for long-running implementation |

## Key Files

| File | Purpose |
|------|---------|
| mcporter.json | Configuration registry for generating CLI tools from MCP servers |
| agent-mail/agent-mail.js | Executable CLI for agent messaging |
| ralph/ralph.sh | Bash loop spawning fresh Amp instances |
| ralph/prompt.md | Instructions injected into each Ralph iteration |

## Patterns

- **MCPorter Generation**: Tools generated as TypeScript, bundled to executable JS
- **Argument Syntax**: Supports colon-delimited (key:value), equals (key=value), and function-call styles
- **Ralph Memory**: Context persisted via git history, progress.txt, prd.json, and AGENTS.md

## Usage

```bash
# Agent Mail - send message
toolboxes/agent-mail/agent-mail.js send_message \
  project_key:/path/to/project \
  sender_name:BlueLake \
  to:GreenCastle \
  subject:"Hello" \
  body_md:"Message content"

# Ralph - autonomous loop
toolboxes/ralph/ralph.sh [max_iterations]
```

## Dependencies

- **External**: Node.js (for CLI execution), MCPorter (for regeneration)
- **Ralph**: Requires Amp CLI installed and configured
- **Agent Mail**: Requires mcp-agent-mail server running during generation

## Notes for AI Agents

- Agent Mail provides inter-agent communication for multi-session coordination
- Ralph is triggered by ca (conductor autonomous) command in Maestro
- To add new tools, update mcporter.json and run generation commands
- The symlink .claude/toolboxes -> ../toolboxes provides access from .claude
