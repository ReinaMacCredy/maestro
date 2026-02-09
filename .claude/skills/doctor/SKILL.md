---
name: doctor
description: Diagnose and fix Maestro installation issues
argument-hint: "[--fix] [--check <name>]"
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, AskUserQuestion
disable-model-invocation: true
---

# Doctor — Maestro Health Check

> Diagnose installation issues, detect configuration drift, and auto-fix common problems.

## Arguments

- `[--fix]` — Auto-fix issues that have safe automatic remediation.
- `[--check <name>]` — Run only one check.

Valid check names:
- `agent-teams`
- `hooks`
- `state-dirs`
- `stale-state`
- `plugin`
- `claude-md`
- `permissions`
- `orphaned-teams`

If `--check` is provided with an unknown name, stop and show valid options.

## Hard Rules

- Run all checks unless `--check <name>` is specified.
- Classify each result as **OK**, **WARN**, or **CRITICAL**.
- Keep checks Maestro-specific. Do not run oh-my-claudecode diagnostics.
- In `--fix` mode, auto-apply only safe fixes. For destructive fixes, ask first.

## Health Checks

Run each check below and capture status + details.

### 1. Agent Teams (`agent-teams`)

Check that `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` is set to `"1"` in `~/.claude/settings.json`.

```bash
jq -r '.env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS // empty' ~/.claude/settings.json
```

- **OK**: Value is `"1"`
- **CRITICAL**: Missing or not `"1"` (Agent Teams disabled)

**--fix remediation**:
- Safe. Add/update `env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` to `"1"` in `~/.claude/settings.json`.
- Preserve existing settings keys.

### 2. Hooks Integrity (`hooks`)

Verify `.claude/hooks/hooks.json` is valid JSON and all referenced scripts exist and pass shell syntax check.

```bash
jq . .claude/hooks/hooks.json > /dev/null
```

For each script command reference in hooks.json, validate:
```bash
bash -n <script_path>
```

- **OK**: hooks.json valid, scripts exist, all `bash -n` checks pass
- **CRITICAL**: hooks.json missing or invalid JSON
- **WARN**: One or more script paths missing, or script has syntax error

**--fix remediation**:
- No automatic mutation of hooks definitions.
- If only execute bit is missing, rely on Check 7 fix.
- Otherwise report manual remediation needed.

### 3. State Directories (`state-dirs`)

Check required Maestro state directories:
- `.maestro/plans/`
- `.maestro/archive/`
- `.maestro/wisdom/`
- `.maestro/drafts/`
- `.maestro/handoff/`
- `.maestro/research/`

- **OK**: All directories exist
- **WARN**: One or more directories missing

**--fix remediation**:
- Safe. Create missing directories with:

```bash
mkdir -p .maestro/plans .maestro/archive .maestro/wisdom .maestro/drafts .maestro/handoff .maestro/research
```

### 4. Stale State (`stale-state`)

Check for stale files:
- Handoff files (`.maestro/handoff/*.json`) older than 24 hours where `status` is `"executing"` or `"designing"`
- Draft files (`.maestro/drafts/*`) older than 48 hours

```bash
find .maestro/handoff -name "*.json" -mtime +0 2>/dev/null
find .maestro/drafts -type f -mtime +2 2>/dev/null
```

For handoff candidates, read JSON and include only statuses `executing` or `designing`.

- **OK**: No stale files
- **WARN**: Stale files found (list file paths)

**--fix remediation**:
- Potentially destructive (file deletion). Ask for confirmation with AskUserQuestion before removing.
- If approved, remove stale handoff/draft files and report each deleted path.

### 5. Plugin Manifest (`plugin`)

Verify `.claude-plugin/plugin.json` is valid JSON.

```bash
jq . .claude-plugin/plugin.json > /dev/null
```

- **OK**: Valid JSON
- **CRITICAL**: Missing file or invalid JSON

**--fix remediation**:
- No safe automatic fix. Report manual intervention required.

### 6. CLAUDE.md Freshness (`claude-md`)

Check project `CLAUDE.md` for expected Maestro markers:

```bash
grep -q "## Commands" CLAUDE.md
grep -q "## Architecture" CLAUDE.md
```

- **OK**: Both markers present
- **WARN**: Missing one or both markers (file may be outdated)

**--fix remediation**:
- No automatic rewrite. Report manual refresh needed.

### 7. Script Permissions (`permissions`)

Check that all scripts in `.claude/scripts/*.sh` are executable.

```bash
find .claude/scripts -name "*.sh" ! -perm -u+x
```

- **OK**: All scripts executable
- **WARN**: One or more non-executable scripts found (list paths)

**--fix remediation**:
- Safe. Run `chmod +x` on each non-executable script.

### 8. Orphaned Teams (`orphaned-teams`)

Check for team directories that may be stale:

```bash
ls ~/.claude/teams/ 2>/dev/null
```

Treat team directories as suspicious if they do not correspond to active work in the current session.

- **OK**: No directories found, or only currently active teams
- **WARN**: Potential orphaned team directories found (list names)

**--fix remediation**:
- Do not auto-delete team directories.
- Recommend running `/reset` for cleanup.
- If the user explicitly requests deletion, ask confirmation first.

## Report Format

Display results as:

```markdown
## Maestro Doctor Report

### Summary
[HEALTHY / ISSUES FOUND (N warnings, M critical)]

### Checks

| # | Check | Status | Details |
|---|-------|--------|---------|
| 1 | Agent Teams | OK | Enabled in settings.json |
| 2 | Hooks Integrity | OK | hooks.json valid, scripts valid |
| 3 | State Directories | WARN | Missing: research/ |
| 4 | Stale State | OK | No stale files |
| 5 | Plugin Manifest | OK | Valid JSON |
| 6 | CLAUDE.md | OK | All markers present |
| 7 | Script Permissions | OK | All scripts executable |
| 8 | Orphaned Teams | WARN | Found team dirs: ... |
```

If issues exist, append:
- `### Issues Found` (bullet list)
- `### Recommended Actions` (manual next steps)

## Auto-Fix (`--fix` flag)

When `--fix` is present and issues were found:

1. List all fixable issues.
2. Apply safe fixes automatically:
   - Agent Teams env entry
   - Missing state directories
   - Script execute permissions
3. For destructive fixes (stale file deletions), ask for confirmation first.
4. Re-run affected checks after each applied fix.
5. End with:

```markdown
### Auto-Fix Result
Fixed N issues. M issues require manual intervention.
```

If no fixable issues are found, say so explicitly.
