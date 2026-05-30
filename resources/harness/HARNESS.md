---
version: 1.1.0
---

# Maestro Harness Protocol

You are an agent (Claude, Codex, or future) working in a repo that
uses Maestro. Follow these rules.

## Shared protocol (all agents)
1. Read MAESTRO_CURRENT_TASK env or `maestro task show` to know which task you're on.
2. Read acceptance.yaml - those criteria are locked.
3. Use the skills active for this task.
4. Run `maestro task verify` when implementation is complete.
5. Hooks auto-record your tool calls to .maestro/runs/<session_id>/events.jsonl across all six lifecycle events (SessionStart, UserPromptSubmit, PreToolUse, PermissionRequest, PostToolUse, Stop). `maestro task verify` matches your `--claim` values against that recorded evidence, so every claim must name a real action you took - an empty or unbacked claim fails verification.

## Task commands (the loop)

State flow: draft -> exploring -> ready -> in_progress -> needs_verification -> verified
(reject -> rejected; abandon / supersede are terminal; block overlays any state)

Orient and find work:

    maestro query matrix                          # task x state x proof overview
    maestro task list --ready                     # claimable work (ready + unblocked)
    maestro task show <id>                         # task detail: state, claim, blockers (or set MAESTRO_CURRENT_TASK)

Make a task claimable (intake):

    maestro task create "<title>" [--feature F --risk R]   # -> draft
    maestro task explore <id>                      # -> exploring
    maestro task accept <id>                       # locks acceptance -> ready

Execute:

    maestro task claim <id>                        # -> in_progress
    maestro task update <id> --claim "<evidence>"  # record progress as you work
    maestro task complete <id> --summary "<what>" --claim "<evidence>"   # -> needs_verification
    maestro task verify <id>                       # GATE: claims must be backed by recorded events/proof -> verified

When stuck:

    maestro task block <id> --reason "<why>" [--by task-NN|decision-NN|<external>]
    maestro task unblock <id> --blocker blk-NN     # use the blocker's own blk- id, not the target

Terminal verbs (reject / abandon / supersede), plus doctor and watch -> see the maestro-task skill.

## If you are Claude Code
- Read the task you're on with @file imports: `@.maestro/tasks/<id>/task.yaml` (state,
  locked status, full state history) and `@.maestro/tasks/<id>/acceptance.yaml` (the locked
  checks you must satisfy).
- The maestro-task skill auto-activates when `.maestro/` is present - use it for the full
  loop and the rarer verbs (reject / abandon / supersede / doctor / watch).

## If you are Codex CLI
- No @file imports: read `.maestro/tasks/<id>/task.yaml` and `acceptance.yaml` explicitly
  with your file-read tool. Resolve `<id>` from MAESTRO_CURRENT_TASK or `maestro task show`.
- The maestro-task skill documents the full loop and the rarer verbs.
