export interface BootstrapTemplateFile {
  readonly path: string;
  readonly content: string;
  readonly executable?: boolean;
}

/**
 * Static agent block for initialized projects. It documents the shared
 * conductor surfaces plus the native handoff launcher that replaced the
 * old queue-based create/list/pickup flow.
 */
export const AGENT_INSTRUCTION_BLOCK = `## Maestro Conductor (shared score)

Projects with \`.maestro/\` hold mission and memory state that all agents share.

**See what is in flight:**
\`\`\`bash
maestro status --json
maestro mission list --json
maestro feature list --mission <id> --json
\`\`\`

**Read an agent prompt (with injected memory):**
\`\`\`bash
maestro feature prompt <featureId> --mission <id>
\`\`\`

**Shared task workflow:**
\`\`\`bash
maestro status --json
maestro task ready --json --compact --limit 5
\`\`\`

Task rows live in \`.maestro/tasks/tasks.jsonl\`.
Live resume state lives in \`.maestro/tasks/continuations/active/<taskId>.json\`.
Completed resume state moves to \`.maestro/tasks/continuations/completed/<taskId>.json\`.
Local per-task history lives in \`.maestro/tasks/local-history/<taskId>.jsonl\`.
Task continuation is the source of truth for normal resume. Standalone handoff packets are for cross-agent transfer.

\`.maestro/tasks/candidates/\` and \`.maestro/tasks/continuations/completed/\` are local-only per-machine state (gitignored). They grow over time as you close tasks; prune them manually when the count gets large:
\`\`\`bash
maestro task prune --dry-run           # preview (default: keep newest 500 per kind)
maestro task prune                     # keep newest 500 per kind
maestro task prune --keep 1000         # override the cap
maestro task prune --candidates-only   # or --continuations-only
maestro task prune --all               # purge everything in those two dirs
\`\`\`

If you are taking new work:
\`\`\`bash
maestro task claim <id>
maestro task update <id> --status in_progress
\`\`\`

When a new session starts with an active task, Maestro session hooks automatically
inject a small continuation pointer into the agent's context.
Saying \`continue\` or \`resume\` in chat then loads the full continuation state
(current state, next action, active decisions, recent history).
These are plain chat intents, not Maestro CLI commands.
Use \`maestro task show <id>\` to read the raw task and continuation state directly.

**Keep resume state fresh while working:**
\`\`\`bash
maestro task update <id> --current-state "..."
maestro task update <id> --next-action "..."
maestro task update <id> --add-decision "keep api stable"
maestro task update <id> --remove-decision "old constraint"
\`\`\`

Use these when the task meaningfully changes:
- current state changed
- next action changed
- an important decision or constraint changed
- blockers appeared or were cleared

**Blockers and completion:**
\`\`\`bash
maestro task block <blockerId> <blockedId...>
maestro task unblock <blockerId> <blockedId...>
maestro task update <id> --status completed --reason "<one-line outcome>"
maestro task reopen <id>
\`\`\`

Rules:
- A task cannot move to \`in_progress\` or \`completed\` while unresolved blockers remain.
- Completion \`--reason\` is persisted verbatim as shared context for future sessions. Keep it short, factual, and free of secrets.
- Reopen a completed task before resuming work on it.

## Task Contracts (optional, recommended for non-trivial work)

Before starting work:
  1. maestro task claim <id>
  2. maestro task contract new <id>
       intent: 1-3 sentences on what you will change and why
       scope.filesExpected: globs you expect to touch
       scope.filesForbidden: globs you commit not to touch
       doneWhen: bullets that signal completion
  3. maestro task contract lock <id>

Project-local draft templates can live under \`.maestro/tasks/contract-templates/\`.
Use \`maestro task contract new <id> --from default\` to load \`.maestro/tasks/contract-templates/default.md\`.

Useful contract commands:
  - maestro task contract edit <id>
  - maestro task contract show <id>
  - maestro task contract verdict <id>
  - maestro task contract list
  - maestro task contract discard <id>
  - maestro task contract amend <id> --reason "..."
  - maestro task contract reopen <id>
  - maestro task contract criteria mark <id> <criterionId> --met
  - maestro task contract criteria add <id> "..."
  - maestro task contract criteria remove <id> <criterionId>
Use \`--session <id>\` on contract new/edit/lock/discard/amend/criteria commands when the owning task is already claimed outside the current shell.

At completion the declared scope is diffed against actual changes.
Out-of-scope files are signal, not failure, unless strict mode is on.
After a contracted task completes, \`maestro task contract show <id>\` includes the stored verdict.
Reopening a completed task reactivates its contract, clears the stored verdict, and preserves amendment history. Previously amended contracts reopen as amended.

**Create a standalone Codex or Claude handoff packet:**
\`\`\`bash
maestro handoff "Implement <featureId> for mission <id>" \\
  [--agent codex|claude]     # default: codex
  [--task-id <id>]           # link the packet to a specific task
  [--model <model>]          # default: codex=gpt-5.4, claude=opus
  [--worktree [slug]]        # create/reuse sibling git worktree
  [--base <branch>]          # base branch for --worktree
  [--name <title>]           # display name for the launch
  [--wait]                   # foreground: block until the agent exits
  [--json]                   # machine-readable launch descriptor
\`\`\`
Handoff packets are portable transfer artifacts built from the active task continuation summary plus recent local task history.
They run detached by default: the launcher returns immediately with a handoff id and the external agent keeps running in the background.
Use \`--wait\` only when you need to block until the agent exits.

Each handoff persists under \`.maestro/launches/<id>/\`:
- \`prompt.md\` -- the self-contained briefing sent to the agent
- \`output.log\` -- live stdout/stderr from the agent process
- \`launch.json\` -- launch status, timing, agent, model, task linkage, and pickup metadata

**Pick up a standalone handoff packet:**
\`\`\`bash
maestro handoff pickup [--id <handoff-id>] [--agent codex|claude --session <id>] [--json]
\`\`\`

Pickup behavior:
- picking up a packet immediately takes over the linked task
- task ownership switches to the current session
- any active task contract lock follows the new owner
- the picked-up packet is consumed for live work

**Capture a correction rule for future sessions:**
\`\`\`bash
maestro memory-correct "use bun not npm" --trigger "package,install,npm"
\`\`\`

**Report feature progress:**
\`\`\`bash
maestro feature update <featureId> --mission <id> --status <status> --report @report.json
\`\`\`

**Coordinate shared task work:**
\`\`\`bash
# Create a task (defaults to 'pending'; add --status in_progress to start immediately)
maestro task create "Title" [--description "..."] [--type task|bug|feature|epic|chore] \\
  [--priority 0-4] [--labels a,b] [--parent <id>] [--blocked-by <id1,id2>] \\
  [--status pending|in_progress]

# Discover, claim, work, complete
maestro task ready --json --compact --limit 5
maestro task show <id>                                       # inspect task details and resume state
maestro task claim <id>                                      # session auto-detected; --session <id> for explicit override
                                                             # add --contract-required to force the reminder note
                                                             # or --no-contract to suppress it for one claim
maestro task update <id> --status in_progress                # auto-claims if unowned
maestro task update <id> --current-state "..." --next-action "..."
maestro task update <id> --add-decision "keep api stable"
maestro task update <id> --remove-decision "old constraint"
maestro task update <id> --status completed --reason "<one-line outcome>" \\
  [--summary "<receipt summary>"] [--surprise "<gotcha>"] [--verified-by <name>] [--strict] [--no-contract]
maestro task reopen <id>

# Discover context
maestro task similar <id>                                    # past tasks with keyword overlap (title + receipt)
maestro task mine                                            # tasks owned by current session
maestro task stuck [--older-than 4h]                         # in_progress tasks with no activity

# Liveness + silent mode
maestro task heartbeat <id>                                  # bump lastActivityAt so the claim doesn't age out
maestro task claim <id> [--stale-after 4h]                   # takes over an aged-out claim from a dead session;
                                                             # active contracts follow the new owner unless policy blocks reclaim
maestro task update <id> ... --silent                        # print '<id> <marker>' only; MAESTRO_TASK_SILENT=1 opts in

# Release or re-wire
maestro task unclaim <id>
maestro task release-owned <sessionId>                       # release tasks owned by a dead/stale session;
                                                             # accepts bare ids or canonical owner ids like \`claude-code-pickup-1\`.
                                                             # Manual \`claude-*\` operator sessions are preserved by \`task ready\`.
maestro task block <blockerId> <blockedId...>                # blockerId must finish before blockedId is ready
maestro task unblock <blockerId> <blockedId...>
maestro task delete <id> [--session <id>] [--force]         # remove a task; claimed tasks require the owner session or --force
maestro task prune [--keep N] [--candidates-only|--continuations-only] [--all] [--dry-run] [--json]
                                                             # bound local candidates + completed continuations; default keep newest 500 per kind
\`\`\`

\`.maestro/tasks/NOW.md\` is refreshed after every task mutation; \`cat\` it for a short in-progress/ready/stuck view anchored to the current state. Active task contracts add a one-line scope/progress summary under in-progress work.

**Plan a batch of tasks upfront (one write, atomic):**
\`\`\`bash
cat <<'JSON' | maestro task plan --file - --start first --session $CODEX_THREAD_ID
{
  "batchId": "optional-string-for-idempotent-retry",
  "tasks": [
    {"name": "first",  "title": "Scaffold feature", "priority": 1},
    {"name": "second", "title": "Wire tests",       "blockedBy": ["first"]},
    {"name": "third",  "title": "Ship PR",          "blockedBy": ["second"]}
  ]
}
JSON
\`\`\`
The whole batch is created under one lock. Name slots in the same batch reference each other via \`blockedBy\` / \`parent\`; strings matching \`tsk-xxxxxx\` point to existing tasks. \`--start <name>\` claims and moves the named task to \`in_progress\` in the same command. Any validation error rejects the whole batch -- nothing is written unless every task is valid. Pass \`batchId\` to make retries idempotent (receipt persists under \`.maestro/tasks/batches/\`).

**Working model:**
- Use task continuation for same-task resume.
- Use handoff packets for Codex-to-Claude or Claude-to-Codex transfer.
- Keep \`current-state\`, \`next-action\`, and active decisions fresh so the next agent can continue without guessing.

**Recommended loop:**
1. Run \`maestro status\` and \`maestro task ready\`.
2. Claim or resume one task.
3. Keep the continuation summary updated as the task changes.
4. Use plain \`continue\` or \`resume\` in chat when returning to active work.
5. Use \`maestro handoff\` only when transferring work to another agent or session.
6. Complete or reopen the task explicitly so resume state stays correct.

**When to use**: Start every session with \`maestro status\` to see shared state. Use \`maestro feature prompt\` to read the current feature's briefing with memory context auto-injected. Use \`maestro task ready\` to inspect the queue, \`maestro task claim\` or \`maestro task update <id> --status in_progress\` to take ownership, keep the continuation summary fresh while you work, use plain \`continue\` or \`resume\` in chat to reload the latest task context, and use \`maestro handoff pickup\` when another agent handed work back to you.`;

export const PROJECT_BOOTSTRAP_TEMPLATES: readonly BootstrapTemplateFile[] = [
  {
    path: ".maestro/AGENTS.md",
    content: `# Maestro Project Bootstrap

This project uses Maestro for local bootstrap and runtime orchestration.

## Layout

- \`.maestro/bootstrap/\` contains committed project bootstrap assets
- \`.maestro/skills/\` contains project-local agent skills
- \`.maestro/missions/\`, \`.maestro/sessions/\`, and \`.maestro/launches/\` contain runtime state
- \`.maestro/tasks/contracts/\` stores one task contract JSON per task plus an append-only index
- \`.maestro/tasks/contract-templates/\` stores reusable contract draft YAML templates such as \`default.md\`
- \`skills/built-in/\` contains shipped built-in fallback skills

## Task Contracts

- Create and lock a task contract before non-trivial work:
  - \`maestro task contract new <id>\`
  - \`maestro task contract lock <id>\`
- Reusable drafts live under \`.maestro/tasks/contract-templates/\`; \`maestro task contract new <id> --from default\` loads \`.maestro/tasks/contract-templates/default.md\`.
- Inspect or clean up contract drafts:
  - \`maestro task contract edit <id>\`
  - \`maestro task contract show <id>\`
  - \`maestro task contract verdict <id>\`
  - \`maestro task contract list\`
  - \`maestro task contract discard <id>\`
  - \`maestro task contract reopen <id>\`
- Amend a locked contract with a recorded reason:
  - \`maestro task contract amend <id> --reason "..." \`
- Manage criteria directly while the contract is active:
  - \`maestro task contract criteria mark <id> <criterionId> --met\`
  - \`maestro task contract criteria add <id> "..." \`
  - \`maestro task contract criteria remove <id> <criterionId>\`
- Use \`--session <id>\` on new/edit/lock/discard/amend/criteria commands when the owning task is already claimed outside the current shell.
- Completion can enforce contracts with \`maestro task update <id> --status completed --strict\`.
- Claiming can remind or require contract setup with \`maestro task claim <id> --contract-required\`; use \`--no-contract\` to suppress the note for a single claim.
- Use \`--no-contract\` only when config requires a contract but the task intentionally has none.
- After completion, \`task contract show\` includes the stored verdict.
- Set \`contracts.overlapPolicy: annotate\` to allow overlapping active contracts while still recording the overlap in verdicts.
- Reopening a completed task reactivates its contract, clears the stored verdict, and preserves amendment history. Previously amended contracts reopen as amended.
- Deleting a task removes its linked contract file and appends a \`task_deleted\` discard record to the contract index.
- \`.maestro/tasks/NOW.md\` adds a one-line contract status summary for active contracted work.
- Stale reclaim inherits active contract ownership by default; set \`contracts.staleReclaimContractPolicy: block\` to refuse it.
- Handoff pickup transfers active contract ownership with the linked task.

## Shared Task Loop

- Inspect active work with:
  - \`maestro status --json\`
  - \`maestro task ready --json --compact --limit 5\`
  - \`maestro task show <id>\`
- Claim and start work with:
  - \`maestro task claim <id>\`
  - \`maestro task update <id> --status in_progress\`
  - \`maestro task claim <id> --contract-required\`
  - \`maestro task claim <id> --no-contract\`
- Keep resume state fresh while working:
  - \`maestro task update <id> --current-state "..." --next-action "..."\`
  - \`maestro task update <id> --add-decision "keep api stable"\`
  - \`maestro task update <id> --remove-decision "old constraint"\`
- Complete with a receipt when useful:
  - \`maestro task update <id> --status completed --reason "<one-line outcome>"\`
  - \`maestro task update <id> --status completed --reason "<one-line outcome>" --summary "<receipt summary>" --surprise "<gotcha>" --verified-by <name>\`
  - add \`--strict\` to block completion on a broken contract verdict
- Discover context and stalled work with:
  - \`maestro task similar <id>\`
  - \`maestro task mine\`
  - \`maestro task stuck [--older-than 4h]\`
- Keep claims alive or recover stale ownership with:
  - \`maestro task heartbeat <id>\`
  - \`maestro task claim <id> [--stale-after 4h]\`
  - \`maestro task update <id> ... --silent\` or \`MAESTRO_TASK_SILENT=1\`
- Bound local-only task artifacts with:
  - \`maestro task prune --dry-run\`
  - \`maestro task prune [--keep N] [--candidates-only|--continuations-only] [--all]\`
- \`.maestro/tasks/NOW.md\` is refreshed after task mutations; \`cat\` it for a short in-progress/ready/stuck view.

## Agent Skill Lookup

1. \`.maestro/skills/{agentType}/SKILL.md\`
2. \`skills/built-in/{agentType}/SKILL.md\`

## Bootstrap Assets

- \`.maestro/bootstrap/init.sh\` is the local setup script
- \`.maestro/bootstrap/services.yaml\` defines commands and service helpers
- \`.maestro/bootstrap/library/\` stores reusable local guidance
- \`.maestro/bootstrap/validation/\` stores local validation/reference artifacts
`,
  },
  {
    path: ".maestro/tasks/contract-templates/default.md",
    content: `intent: >
  State what will change and why in 1-3 sentences.
scope:
  filesExpected:
    - src/**
  filesForbidden: []
doneWhen:
  - text: Describe the observable signal that proves the task is done.
    kind: manual
`,
  },
  {
    path: ".maestro/bootstrap/init.sh",
    executable: true,
    content: `#!/bin/bash
set -euo pipefail

echo "== Maestro Bootstrap Init =="

if [ -f package.json ]; then
  if command -v bun >/dev/null 2>&1; then
    echo "[ok] bun $(bun --version)"
    if [ ! -d "node_modules" ]; then
      echo "[...] Installing dependencies with bun"
      bun install
    else
      echo "[ok] node_modules already present"
    fi
  else
    echo "[!] package.json detected but bun is not installed"
    echo "    Install bun or customize .maestro/bootstrap/init.sh for this project"
  fi
fi

echo "[ok] Bootstrap init completed"
`,
  },
  {
    path: ".maestro/bootstrap/services.yaml",
    content: `commands:
  install: echo "Customize commands.install in .maestro/bootstrap/services.yaml"
  test: echo "Customize commands.test in .maestro/bootstrap/services.yaml"
  typecheck: echo "Customize commands.typecheck in .maestro/bootstrap/services.yaml"
  build: echo "Customize commands.build in .maestro/bootstrap/services.yaml"
  lint: echo "Customize commands.lint in .maestro/bootstrap/services.yaml"
  missionControlJson: echo "Customize commands.missionControlJson in .maestro/bootstrap/services.yaml"
  missionControlPreview: echo "Customize commands.missionControlPreview in .maestro/bootstrap/services.yaml"

services: {}
`,
  },
  {
    path: ".maestro/bootstrap/library/architecture.md",
    content: `# Architecture

Use this document for project-specific architecture notes that agents should read before implementation.

## System Overview

Describe the major components, boundaries, and invariants of this project.

## Main Components

### Domain

Document core entities, invariants, and validation rules.

### Use Cases

Document business workflows and orchestration logic.

### Adapters

Document storage, network, and framework boundaries.
`,
  },
  {
    path: ".maestro/bootstrap/library/environment.md",
    content: `# Environment

Use this document for required tools, environment variables, and local setup notes.

## Required Tools

- Document the tools required for this repository

## Runtime Layout

- \`.maestro/bootstrap/\` is the committed bootstrap layer
- \`.maestro/skills/\` is the local runtime skill layer
- \`.maestro/missions/\`, \`.maestro/sessions/\`, and \`.maestro/launches/\` are runtime state

## Environment Variables

- Document required environment variables and safe defaults here
`,
  },
  {
    path: ".maestro/bootstrap/library/user-testing.md",
    content: `# User Testing

Use this document for project-specific validation guidance.

## Validation Surfaces

- List CLI, API, UI, or TUI surfaces that matter for this project

## Validation Tools

- Document the commands or tools used to verify each surface

## Concurrency and Isolation

- Note whether validators need temp repos, isolated databases, or other guardrails
`,
  },
  {
    path: ".maestro/bootstrap/validation/README.md",
    content: `# Validation References

Store reusable validation notes, reference flows, or review artifacts here when they help future agents.

Suggested contents:

- flow snapshots
- review findings
- validation playbooks
- command transcripts worth preserving
`,
  },
];
