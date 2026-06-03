---
version: 1.5.0
---

# Maestro Harness Protocol

You are an agent (Claude, Codex, or future) working in a repo that
uses Maestro. Follow these rules.

## Shared protocol (all agents)
1. Start with `maestro status`; honor MAESTRO_CURRENT_TASK env or `maestro task show <id>` when a current task is set.
2. Read acceptance.yaml - those criteria are locked.
3. Use the skills active for this task.
4. Complete tasks with `maestro task complete <id> --summary "<what>" --claim "<claim>" --proof "<observed evidence>"`; Maestro records the proof and auto-runs verification.
5. Hooks auto-record your tool calls to .maestro/runs/<session_id>/events.jsonl across all six lifecycle events (SessionStart, UserPromptSubmit, PreToolUse, PermissionRequest, PostToolUse, Stop). Verification matches your `--claim` values against recorded or inline proof, so every claim must name a real action you took - an empty or unbacked claim fails verification.

## Task commands (the loop)

State flow: draft -> exploring -> ready -> in_progress -> needs_verification -> verified
(reject -> rejected; abandon / supersede are terminal; block overlays any state)

Orient and find work:

    maestro status                                # repo handoff and next action
    maestro task next                             # one best task action
    maestro task list --ready                     # claimable work (ready + unblocked)
    maestro task show <id>                        # task detail: state, claim, blockers

Make a task claimable (intake):

    maestro task create "<title>" [--feature F --risk R --check "<observable result>"]   # -> draft
    maestro task explore <id>                      # -> exploring
    maestro task accept <id>                       # locks acceptance -> ready

Execute:

    maestro task claim <id>                        # -> in_progress
    maestro task update <id> --claim "<evidence>"  # record progress as you work
    maestro task complete <id> --summary "<what>" --claim "<evidence>" --proof "<observed evidence>"   # auto-verifies
    maestro query proof <id>                       # recovery path when verification fails

When stuck:

    maestro task block <id> --reason "<why>" [--by task-NN|decision-NN|<external>]
    maestro task unblock <id> --blocker blk-NN     # use the blocker's own blk- id, not the target

Terminal verbs (reject / abandon / supersede), plus doctor and watch -> see the maestro-task skill.

## Design work (the brainstorm loop)

Before a cluster of tasks exists, design lands as a feature. Map the problem from the real
code first, then walk the open questions one at a time: lock each as a decision plus a note,
never batch-decide. Resume from the feature, not from memory.

    maestro feature new "<topic>"                  # topic = feature (proposed); scaffolds notes.md
    maestro feature set <id> --description "<problem>" --question "<q>" ...   # map: problem + open questions
    # walk ONE open question at a time; on each lock:
    maestro decision new "<the locked fork>"       # record the locked fork
    #   then append the reasoning to .maestro/features/<id>/notes.md as you decide,
    #   and re-issue --question with the remaining list (set replaces the field)
    maestro feature show <id>                      # resume point: open questions + notes so far
    # decisions locked -> you now know the contract; author it:
    maestro feature set <id> --acceptance "<criterion>" --area "<surface>"

Full method -> the maestro-design skill. Accept, tasks, ship, and notes.md mechanics -> the maestro-feature skill.

## Harness self-improvement (on request)

Maestro also surfaces recurring friction from the run log and task history as improvement
proposals. This is passive - review it only when asked, never auto-act.

    maestro harness list [--all]                   # backlog; --all adds the measured ledger
    maestro harness apply <id>                     # accept -> spawns a standalone task (give it a --check, run the loop above)
    maestro harness measure <id> [--force]         # re-run the detector to close the loop -> measured once the friction is gone

Full method -> the maestro-task skill.

## Orchestration (when work can fan out)

Parallel sub-agents pay off when contexts must stay clean or work is
independent. The recipes live in the skills; this is the menu:

    2+ independent ready tasks on a feature  -> feature fan-out      (maestro-feature)
    contested / high-stakes verification     -> adversarial fan-out  (maestro-verify)
    taste-based design fork (naming, UX)     -> generate-and-filter  (maestro-design)
    unstructured backlog to triage           -> intake triage        (maestro-task)
    unknown amount of work                   -> loop until done      (maestro-task)

Results land through the verbs (task / decision / event), never only in the
conversation. Claude Code: author a Workflow script. Codex: spawn parallel
sub-agents directly (multi_agent_v1; worktree threads when files overlap).

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
