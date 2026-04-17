export interface BootstrapTemplateFile {
  readonly path: string;
  readonly content: string;
  readonly executable?: boolean;
}

/**
 * Phase 1 strip: the old AGENT_INSTRUCTION_BLOCK described deleted
 * handoff-* commands. This replacement block advertises only the
 * mission/feature/memory surfaces that survive the v1.0.0 strip.
 * Phase 2 will extend it with the UKI handoff workflow.
 * The block is now static (no `{{agent}}` placeholder) because the
 * legacy `handoff-pickup --agent <slug>` flow is gone.
 */
export const AGENT_INSTRUCTION_BLOCK = `## Maestro Conductor (shared score)

Projects with \`.maestro/\` hold mission and memory state that all agents share.

**See what is in flight:**
\`\`\`bash
maestro status --json
maestro mission list --json
maestro feature list --mission <id> --json
\`\`\`

**Read a worker prompt (with injected memory):**
\`\`\`bash
maestro feature prompt <featureId> --mission <id>
\`\`\`

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
maestro task ready --json --limit 5
maestro task claim <id>                                      # session auto-detected; --session <id> for explicit override
maestro task update <id> --status in_progress                # auto-claims if unowned
maestro task update <id> --status completed --reason "<one-line outcome>"

# Release or re-wire
maestro task unclaim <id>
maestro task block <blockerId> <blockedId...>                # blockerId must finish before blockedId is ready
maestro task unblock <blockerId> <blockedId...>
\`\`\`

**Task contract (the two non-obvious rules):**
- A task cannot move to \`in_progress\` or \`completed\` while any id in its \`blockedBy\` list is unresolved. Resolve blockers first or create them as \`completed\` upstream.
- Completion \`--reason\` is persisted verbatim as shared context for future sessions. Keep it terse, factual, and free of secrets.

**When to use**: Start every session with \`maestro status\` to see shared state. Use \`maestro feature prompt\` to read the current feature's briefing with memory context auto-injected. Use \`maestro task ready\` to inspect the shared queue, and \`maestro task create ... --status in_progress\` or \`maestro task claim\` to take ownership.`;

export const PROJECT_BOOTSTRAP_TEMPLATES: readonly BootstrapTemplateFile[] = [
  {
    path: ".maestro/AGENTS.md",
    content: `# Maestro Project Bootstrap

This project uses Maestro for local bootstrap and runtime orchestration.

## Layout

- \`.maestro/bootstrap/\` contains committed project bootstrap assets
- \`.maestro/skills/\` contains project-local worker skills
- \`.maestro/missions/\`, \`.maestro/sessions/\`, and \`.maestro/handoffs/\` contain runtime state
- \`skills/built-in/\` contains shipped built-in fallback skills

## Worker Skill Lookup

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

Use this document for project-specific architecture notes that workers should read before implementation.

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
- \`.maestro/missions/\`, \`.maestro/sessions/\`, and \`.maestro/handoffs/\` are runtime state

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

Store reusable validation notes, reference flows, or review artifacts here when they help future workers.

Suggested contents:

- flow snapshots
- review findings
- validation playbooks
- command transcripts worth preserving
`,
  },
];
