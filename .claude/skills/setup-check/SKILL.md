---
name: setup-check
description: Verify Maestro plugin prerequisites — Agent Teams flag, jq, trace MCP, PSM tools, directories, and hooks.
allowed-tools: Read, Bash, Glob, Write, AskUserQuestion
disable-model-invocation: true
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

### 3. Trace MCP Toolbox

Verify trace toolbox prerequisites exist:
- `toolboxes/trace/` directory
- `toolboxes/trace/trace.js` entrypoint

Use `Glob` checks. PASS only if both exist.

**On FAIL**: Report missing paths and instruct user to run setup/build for the trace toolbox before enabling trace MCP.

### 4. tmux Installed (PSM)

Run `which tmux` — `/psm` requires tmux sessions.

**On FAIL**: Detect OS and show install command:
- macOS: `brew install tmux`
- Linux (Debian/Ubuntu): `sudo apt-get install tmux`

### 5. gh CLI Installed (PSM)

Run `which gh` — `/psm` uses GitHub CLI for PR/issue operations.

**On FAIL**: Detect OS and show install command:
- macOS: `brew install gh`
- Linux (Debian/Ubuntu): `sudo apt-get install gh`

Also remind user to authenticate after install:
```bash
gh auth login
```

### 6. .maestro Directory Structure

Verify these directories exist:
- `.maestro/plans/`
- `.maestro/drafts/`
- `.maestro/wisdom/`
- `.maestro/research/`

**On FAIL**:
```
AskUserQuestion(
  questions: [{
    question: "Missing .maestro directories. Create them now?",
    header: "Fix dirs",
    options: [
      { label: "Yes, create", description: "Run mkdir -p .maestro/plans .maestro/drafts .maestro/wisdom .maestro/research" },
      { label: "Skip", description: "Continue without creating directories" }
    ],
    multiSelect: false
  }]
)
```

If yes: `Bash("mkdir -p .maestro/plans .maestro/drafts .maestro/wisdom .maestro/research")`. Re-check to confirm.

### 7. Hook Symlinks

Check that all symlinks in `.claude/scripts/` point to valid targets:
```bash
ls -la .claude/scripts/
```

Report any broken symlinks. No auto-fix for symlinks — they require manual attention.

### 8. Plugin Manifest

Validate the plugin manifest parses correctly:
```bash
cat .claude-plugin/plugin.json | jq .
```

### 9. Hooks Configuration

Validate hooks config parses correctly:
```bash
cat .claude/hooks/hooks.json | jq .
```

### 10. Script Permissions

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
- Trace MCP toolbox: PASS/FAIL
- tmux: PASS/FAIL
- gh CLI: PASS/FAIL
- Directories: PASS/FAIL
- Symlinks: PASS/FAIL
- Plugin manifest: PASS/FAIL
- Hooks config: PASS/FAIL
- Script permissions: PASS/FAIL

Overall: READY / NOT READY (N issues to fix)
```
