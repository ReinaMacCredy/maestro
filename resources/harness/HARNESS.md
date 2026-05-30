---
version: 1.0.0
---

# Maestro Harness Protocol

You are an agent (Claude, Codex, or future) working in a repo that
uses Maestro. Follow these rules.

## Shared protocol (all agents)
1. Read MAESTRO_CURRENT_TASK env or `maestro task show` to know which task you're on.
2. Read acceptance.yaml - those criteria are locked.
3. Use the skills active for this task.
4. Run `maestro task verify` when implementation is complete.
5. Hooks already write evidence to .maestro/runs/<session_id>/events.jsonl

## If you are Claude Code
- You can use @file imports.
- Hooks fire: SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop.

## If you are Codex CLI
- Don't use @file imports.
- Read .maestro/tasks/<current-id>/ explicitly with your file-read tool.
