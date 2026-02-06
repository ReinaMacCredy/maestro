# Troubleshooting

Common issues and solutions for Maestro.

## Agent Teams Not Working

**Symptom**: `/design` or `/work` fails with "unknown tool: Teammate" or similar.

**Cause**: Agent Teams experimental flag not enabled.

**Fix**: Add to `~/.claude/settings.json`:
```json
{
  "env": {
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
  }
}
```

Restart Claude Code after changing settings.

## jq Not Found

**Symptom**: Hook scripts fail silently or produce errors about `jq`.

**Cause**: `jq` is not installed. Maestro hooks use jq to parse JSON payloads.

**Fix**:
```bash
# macOS
brew install jq

# Ubuntu/Debian
sudo apt-get install jq

# Windows (scoop)
scoop install jq
```

## Stale Team Directories

**Symptom**: `/design` or `/work` fails to create a team, or agent coordination is confused.

**Cause**: Interrupted sessions leave orphaned team directories in `~/.claude/teams/`.

**Fix**: Run `/reset` to clean up orphaned teams, or manually:
```bash
rm -rf ~/.claude/teams/<stale-team-name>
rm -rf ~/.claude/tasks/<stale-team-name>
```

## Missing .maestro Directories

**Symptom**: Plans or wisdom don't persist between sessions.

**Cause**: The `.maestro/` directory structure wasn't created.

**Fix**:
```bash
mkdir -p .maestro/plans .maestro/drafts .maestro/wisdom
```

## Broken Symlinks

**Symptom**: Hooks don't fire or produce "command not found" errors.

**Cause**: Symlinks in `.claude/scripts/` point to missing targets.

**Fix**: Run `/setup-check` to identify broken symlinks, then recreate:
```bash
cd .claude/scripts
ln -sf ../../scripts/<script-name>.sh <script-name>.sh
```

## Orchestrator Editing Files Directly

**Symptom**: Orchestrator attempts to Write/Edit files and gets blocked by the hook.

**Cause**: This is expected behavior -- the `orchestrator-guard.sh` hook prevents direct edits.

**Fix**: No fix needed. The orchestrator should delegate to kraken or spark teammates.

## Plan Validation Warnings

**Symptom**: After writing a plan, you see warnings about missing sections.

**Cause**: The `plan-validator.sh` hook checks for required sections (Objective, Scope, Tasks, Verification).

**Fix**: Add the missing sections to your plan file. Use `/plan-template` to scaffold a plan with all required sections.

## Workers Editing Plans

**Symptom**: Workers (kraken/spark) get blocked when trying to modify plan files.

**Cause**: The `plan-protection.sh` hook prevents worker agents from modifying `.maestro/plans/`.

**Fix**: Only prometheus and orchestrator should modify plans. If a plan needs changes, have the team lead do it.

## No Wisdom Being Accumulated

**Symptom**: `.maestro/wisdom/` stays empty after `/work` cycles.

**Cause**: The orchestrator may not be extracting wisdom at the end of execution.

**Fix**: Ensure the `/work` command completes fully. Wisdom extraction happens during the orchestrator's cleanup phase. Check if the execution was interrupted.

## Hook Scripts Not Executable

**Symptom**: Hooks defined in hooks.json don't run.

**Cause**: Script files lack execute permission.

**Fix**:
```bash
chmod +x scripts/*.sh
```
