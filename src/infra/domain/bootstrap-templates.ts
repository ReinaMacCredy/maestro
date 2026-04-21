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
maestro task update <id> --status in_progress                # auto-claims if unowned
maestro task update <id> --current-state "..." --next-action "..."
maestro task update <id> --add-decision "keep api stable"
maestro task update <id> --remove-decision "old constraint"
maestro task update <id> --status completed --reason "<one-line outcome>" \\
  [--summary "<receipt summary>"] [--surprise "<gotcha>"] [--verified-by <name>]
maestro task reopen <id>

# Discover context
maestro task similar <id>                                    # past tasks with keyword overlap (title + receipt)
maestro task mine                                            # tasks owned by current session
maestro task stuck [--older-than 4h]                         # in_progress tasks with no activity

# Liveness + silent mode
maestro task heartbeat <id>                                  # bump lastActivityAt so the claim doesn't age out
maestro task claim <id> [--stale-after 4h]                   # takes over an aged-out claim from a dead session
maestro task update <id> ... --silent                        # print '<id> <marker>' only; MAESTRO_TASK_SILENT=1 opts in

# Release or re-wire
maestro task unclaim <id>
maestro task release-owned <sessionId>                       # release tasks owned by a dead/stale session;
                                                             # accepts bare ids or canonical owner ids like \`claude-code-pickup-1\`.
                                                             # Manual \`claude-*\` operator sessions are preserved by \`task ready\`.
maestro task block <blockerId> <blockedId...>                # blockerId must finish before blockedId is ready
maestro task unblock <blockerId> <blockedId...>
\`\`\`

\`.maestro/tasks/NOW.md\` is refreshed after every task mutation; \`cat\` it for a short in-progress/ready/stuck view anchored to the current state.

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
- \`skills/built-in/\` contains shipped built-in fallback skills

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
