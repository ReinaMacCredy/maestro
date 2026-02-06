---
name: setup-check
description: Verify Maestro plugin prerequisites — Agent Teams flag, jq, directories, symlinks.
allowed-tools: Read, Bash, Glob
---

# Setup Check

Run through each check below. Report PASS or FAIL for each item with details.

## Checks

### 1. Agent Teams Flag

Read `~/.claude/settings.json` and verify it contains:
```json
{
  "env": {
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
  }
}
```

### 2. jq Installed

Run `which jq` — hooks require jq for JSON processing.

### 3. .maestro Directory Structure

Verify these directories exist:
- `.maestro/plans/`
- `.maestro/drafts/`
- `.maestro/wisdom/`

If missing, report which ones and suggest:
```bash
mkdir -p .maestro/plans .maestro/drafts .maestro/wisdom
```

### 4. Hook Symlinks

Check that all symlinks in `.claude/scripts/` point to valid targets:
```bash
ls -la .claude/scripts/
```

Report any broken symlinks.

### 5. Plugin Manifest

Validate the plugin manifest parses correctly:
```bash
cat .claude-plugin/plugin.json | jq .
```

### 6. Hooks Configuration

Validate hooks config parses correctly:
```bash
cat .claude/hooks/hooks.json | jq .
```

## Output

End with a summary:
```
## Setup Status
- Agent Teams: PASS/FAIL
- jq: PASS/FAIL
- Directories: PASS/FAIL
- Symlinks: PASS/FAIL
- Plugin manifest: PASS/FAIL
- Hooks config: PASS/FAIL

Overall: READY / NOT READY (N issues to fix)
```
