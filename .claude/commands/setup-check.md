---
name: setup-check
description: Verify Maestro plugin prerequisites — Agent Teams flag, jq, directories, symlinks.
allowed-tools: Read, Bash, Glob, Write, AskUserQuestion
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

**On FAIL**: Show the user the exact JSON needed. Note: Cannot safely auto-edit global settings — provide instructions:
> Add the following to `~/.claude/settings.json` and restart Claude Code:
> ```json
> { "env": { "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1" } }
> ```

### 2. jq Installed

Run `which jq` — hooks require jq for JSON processing.

**On FAIL**: Detect OS and show install command:
```
AskUserQuestion(
  questions: [{
    question: "jq is not installed. Install it now?",
    header: "Install jq",
    options: [
      { label: "Show command", description: "Display the install command for your OS" },
      { label: "Skip", description: "Continue without jq (hooks may not work)" }
    ],
    multiSelect: false
  }]
)
```

If user wants to install, show: `brew install jq` (macOS) or `sudo apt-get install jq` (Linux). Re-check after install.

### 3. .maestro Directory Structure

Verify these directories exist:
- `.maestro/plans/`
- `.maestro/drafts/`
- `.maestro/wisdom/`

**On FAIL**:
```
AskUserQuestion(
  questions: [{
    question: "Missing .maestro directories. Create them now?",
    header: "Fix dirs",
    options: [
      { label: "Yes, create", description: "Run mkdir -p .maestro/plans .maestro/drafts .maestro/wisdom" },
      { label: "Skip", description: "Continue without creating directories" }
    ],
    multiSelect: false
  }]
)
```

If yes: `Bash("mkdir -p .maestro/plans .maestro/drafts .maestro/wisdom")`. Re-check to confirm.

### 4. Hook Symlinks

Check that all symlinks in `.claude/scripts/` point to valid targets:
```bash
ls -la .claude/scripts/
```

Report any broken symlinks. No auto-fix for symlinks — they require manual attention.

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

### 7. Script Permissions

Verify all scripts in `scripts/` are executable:
```bash
ls -la scripts/*.sh
```

**On FAIL**:
```
AskUserQuestion(
  questions: [{
    question: "Some scripts are missing execute permission. Fix now?",
    header: "Fix perms",
    options: [
      { label: "Yes, fix", description: "Run chmod +x scripts/*.sh" },
      { label: "Skip", description: "Continue without fixing permissions" }
    ],
    multiSelect: false
  }]
)
```

If yes: `Bash("chmod +x scripts/*.sh")`. Re-check to confirm.

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
