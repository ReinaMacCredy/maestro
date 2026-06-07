---
version: 1.8.2
---

# Maestro Harness Protocol

You are an agent working in a repo that
uses Maestro. Follow these rules.

## Shared protocol (all agents)
1. Start with `maestro status`; honor MAESTRO_CURRENT_TASK env or `maestro task show <id>` when a current task is set.
2. Read acceptance.yaml - those criteria are locked.
3. Use the skills active for this task.
4. Complete tasks with `maestro task complete <id> --summary "<what>" --claim "<claim>" --proof "<observed evidence>"`; Maestro records the proof and auto-runs verification.
5. Hooks auto-record your tool calls as proof. Verification matches each `--claim` against recorded or inline proof - an empty or unbacked claim fails.
6. When the user corrects your behavior, record it: `maestro event intervention --note "<what was wrong>" [--topic <slug>]`.

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

Design lands as a feature while `proposed`. Map the problem from real code, then walk
open questions ONE at a time - lock each as a decision + a notes.md line, never batch-decide.

    maestro feature new "<topic>"                  # scaffolds notes.md
    maestro feature set <id> --description "<problem>" --question "<q>" ...
    maestro decision new "<the locked fork>"       # per lock; drop the answered --question
    maestro feature show <id>                      # resume point
    maestro feature set <id> --acceptance "<criterion>" --area "<surface>"   # then author the contract

Full method -> the maestro-design skill; lifecycle -> maestro-feature.

## Harness self-improvement

Passive friction backlog: `maestro harness list / apply / measure`.
Over-threshold friction surfaced by status/next/complete: apply and claim it before new work, or dismiss it with a reason when noise.
Binary only counts and shows; the agent acts. Full method -> the maestro-task skill.

## Orchestration (when work can fan out)

Parallel sub-agents pay off when contexts must stay clean or work is
independent. The recipes live in the skills; this is the menu:

    2+ independent ready tasks on a feature  -> feature fan-out      (maestro-feature)
    contested / high-stakes verification     -> adversarial fan-out  (maestro-verify)
    taste-based design fork (naming, UX)     -> generate-and-filter  (maestro-design)
    unstructured backlog to triage           -> intake triage        (maestro-task)
    unknown amount of work                   -> loop until done      (maestro-task)

Results land through the verbs (task / decision / event), never only in conversation.
Claude Code: author a Workflow script. Codex: parallel sub-agents directly (worktree
threads when files overlap).
