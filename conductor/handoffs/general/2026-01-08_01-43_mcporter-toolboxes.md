# Handoff: MCPorter Toolboxes Integration

**Created:** 2026-01-08 01:43
**Track:** mcporter-toolboxes (COMPLETED)
**Trigger:** manual (post-finish)

## Summary

Completed integration of MCPorter into Maestro for generating CLI tools from MCP servers. Agent Mail is the first converted tool.

## What Was Done

1. **Design Session (ds)** - Explored MCPorter integration options
2. **Created `toolboxes/`** - New shared directory for generated CLIs
3. **Generated Agent Mail CLI** - `toolboxes/agent-mail/agent-mail.js`
4. **Added documentation** - `toolboxes.md` in maestro-core, `agent-mail-cli.md` in orchestrator
5. **Ran `/conductor-finish`** - Track archived, learnings extracted

## Key Files

```
toolboxes/
├── agent-mail/
│   ├── agent-mail.js     # Generated CLI (1.4MB)
│   └── agent-mail.ts     # Source template
├── mcporter.json         # Config for all servers
└── README.md

skills/maestro-core/references/toolboxes.md
skills/orchestrator/references/agent-mail-cli.md
conductor/archive/mcporter-toolboxes/  # Archived track
```

## How to Use Agent Mail CLI

```bash
# Health check
toolboxes/agent-mail/agent-mail.js health-check

# Register agent
toolboxes/agent-mail/agent-mail.js register-agent \
  --project-key "/path/to/project" \
  --program "claude-code" \
  --model "opus-4.5"

# Send message
toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "/path/to/project" \
  --sender-name "BlueLake" \
  --to '["GreenCastle"]' \
  --subject "Hello" \
  --body-md "Test"
```

## Adding Future MCP Servers

```bash
# 1. Add to mcporter.json
# 2. Generate CLI
mkdir -p toolboxes/<name>
npx mcporter generate-cli <server> \
  --bundle toolboxes/<name>/<name>.js
chmod +x toolboxes/<name>/<name>.js
```

## Pending Actions

- [ ] Commit changes (`git add . && git commit`)
- [ ] Push to remote

## Learnings

- MCPorter CLI uses `--kebab-case` for arguments
- Arrays must be JSON strings: `--to '["Agent1"]'`
- Generated .js files are ~1.4MB bundled (normal)
- MCP server must be running during generation only

## Next Session

Ready to commit and push. No blockers.
