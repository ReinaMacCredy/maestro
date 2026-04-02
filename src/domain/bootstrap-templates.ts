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
- \`.maestro/skills/\` contains project-local worker skills
- \`.maestro/missions/\`, \`.maestro/sessions/\`, and \`.maestro/handoffs/\` contain runtime state
- \`skills/built-in/\` contains shipped built-in fallback skills

## Worker Skill Lookup

1. \`.maestro/skills/{workerType}/SKILL.md\`
2. \`skills/built-in/{workerType}/SKILL.md\`

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
