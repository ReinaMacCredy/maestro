---
name: setup-check
description: "Fast prerequisite gate for Maestro runtime readiness. Use before `/design` or `/work` to verify required local tooling and flags."
allowed-tools: Read, Bash, Glob, AskUserQuestion
disable-model-invocation: true
---

# Setup Check

Run only the quick checks below. Report PASS or FAIL for each item with details.

This command is a lightweight readiness gate only.
- Do not run deep integrity diagnostics here.
- Do not mutate repo files except optional `mkdir -p` for missing `.maestro/` directories when the user approves.
- Do not perform destructive cleanup or deletion in this flow.
- For deep diagnostics and broad remediation, direct the user to `doctor` (primary diagnostic flow).
- Keep guidance runtime-accurate: use only local file/CLI checks available in Amp (`Read`, `Bash`, `Glob`, `AskUserQuestion`).
- Do not assume unavailable Agent Teams APIs (`spawn_agent`, `send_input`, `request_user_input`, `TeamCreate`).

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

Run `which jq` — Maestro hooks and diagnostics rely on jq for JSON processing.

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

## Out of Scope (Handled by `doctor`)

Do not run these checks in `setup-check`:
- Hook/script integrity and shell syntax validation
- Plugin or hooks JSON manifest parsing
- Broken symlink auditing in `.claude/scripts/`
- Script execute-bit audits/fixes
- Stale state cleanup and orphaned team checks
- CLAUDE.md freshness checks

If the user asks for deeper diagnosis or auto-remediation, run `doctor` (optionally with `--fix`).

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

Overall: READY / NOT READY (N issues to fix)

Next step: run `doctor` for deep diagnostics/remediation.
```
