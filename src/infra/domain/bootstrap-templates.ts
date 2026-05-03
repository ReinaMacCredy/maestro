export interface BootstrapTemplateFile {
  readonly path: string;
  readonly content: string;
  readonly executable?: boolean;
}

export const PROJECT_BOOTSTRAP_TEMPLATES: readonly BootstrapTemplateFile[] = [
  {
    path: ".maestro/AGENTS.md",
    content: `# Maestro Project Bootstrap

This project uses Maestro for local bootstrap and runtime orchestration.

## Layout

- \`.maestro/bootstrap/\` contains committed project bootstrap assets
- \`.maestro/skills/\` contains project-local agent skills
- \`.maestro/missions/\` and \`.maestro/sessions/\` contain runtime state (handoff packets live globally at \`~/.maestro/handoff/\`)
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
    path: ".maestro/specs/.gitkeep",
    content: "",
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
- \`.maestro/missions/\` and \`.maestro/sessions/\` are runtime state (handoff packets live globally at \`~/.maestro/handoff/\`)

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
  {
    path: ".maestro/policies/sensitive-paths.yaml",
    content: `# Default sensitive-path globs for Maestro Trust Verifier.
# Paths matching these globs trigger checkSensitivePaths findings (advisory at L2,
# may gate at L7 per Rule 12). Extend or relax for your repo.
sensitive_paths:
  - "src/auth/**"
  - "src/payments/**"
  - "**/secrets/**"
  - "package.json"
  - "bun.lock"
  - ".github/workflows/**"
  - "**/migrations/**"
  - "**/permissions/**"
`,
  },
];
