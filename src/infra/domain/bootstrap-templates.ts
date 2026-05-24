export type BootstrapOverwritePolicy =
  // Default: under `--reset-templates` or interactive confirm, replace the file.
  | "force"
  // Once emitted, never touch the file again — even under `--reset-templates`.
  | "never"
  // The file's harness-owned region is only the managed block; the rest is
  // user-owned. `stepDropTemplates` skips when the file exists; the managed
  // block step rewrites just its markers via agent-block helpers.
  | "managed-block";

export interface BootstrapTemplateFile {
  readonly path: string;
  readonly content: string;
  readonly executable?: boolean;
  readonly overwritePolicy?: BootstrapOverwritePolicy;
}

export const PROJECT_BOOTSTRAP_TEMPLATES: readonly BootstrapTemplateFile[] = [
  {
    path: ".maestro/AGENTS.md",
    content: `# Maestro Project Bootstrap

This project uses Maestro as a long-running agent harness. This file is a TOC,
not an encyclopedia — read it as pointers and open the linked docs as needed.

## Where to read

| Topic | Doc |
|---|---|
| Read order, lane policy, daily commands | \`.maestro/MAESTRO.md\` |
| Harness positioning + principles | \`docs/harness-positioning.md\` |
| Verb reference | \`docs/cli-reference.md\` |
| Witness ladder + evidence kinds | \`docs/witness-levels.md\` |
| Risk classes + policy | \`docs/risk-class-derivation.md\`, \`docs/policy-format.md\` |
| Schedule recipes (external triggers only) | \`docs/schedule-recipes.md\` |
| Architecture lints | \`docs/architecture-lints.md\` |

## Layout

- \`.maestro/bootstrap/\` — committed bootstrap assets (\`init.sh\`, services, library, validation)
- \`.maestro/skills/\` — project-local agent skills
- \`.maestro/missions/\` / \`.maestro/sessions/\` — runtime state (handoff packets live globally)
- \`.maestro/tasks/contracts/\` + \`.maestro/tasks/contract-templates/\` — versioned contracts and reusable drafts
- \`skills/built-in/\` — shipped built-in fallback skills

## Daily loop (one-liners)

- Pre-flight risk: \`maestro intake --paths <paths>\`
- Plan check: \`maestro plan check --task <id> --plan-file <path>\`
- Contract lifecycle: contracts are auto-created on \`maestro task claim <id>\`; inspect via \`maestro contract show --task <id>\` and amend via \`maestro contract amend --task <id> --reason "..."\`
- Verdict: \`maestro verdict request --task <id>\`
- Recovery: \`maestro recover --task <id>\`
- Convergence oracle: \`maestro ralph review --task <id>\`

## Agent skill lookup

1. \`.maestro/skills/{agentType}/SKILL.md\`
2. \`skills/built-in/{agentType}/SKILL.md\`

## Project conventions

Repo-level code conventions, build commands, and feature boundaries live in the
project root \`AGENTS.md\` and \`CLAUDE.md\`. This file holds only the harness
pointer surface.
`,
  },
  {
    path: ".maestro/MAESTRO.md",
    content: `# Maestro Project State — Read Order

For any agent picking up work in this repo, read in order:

1. \`.maestro/MAESTRO.md\` (this file) — read order, lane policy, daily commands.
2. \`AGENTS.md\` (repo root) — code conventions, feature boundaries, build/test commands.
3. \`.maestro/tasks/NOW.md\` — what is currently in flight.
4. \`maestro status --json\` — live state across missions, tasks, pending loosenings.
5. \`.maestro/policies/*.yaml\` — risk, autopilot, release, sensitive-paths, owners.
6. \`.maestro/specs/<id>/spec.json\` — acceptance criteria for the active mission, if any.

If two sources conflict, the lower-numbered file is operational; the higher-numbered file is informational.

## Before code: run \`maestro intake\`

Pre-flight risk classification before writing code. \`maestro intake --paths <paths> [--flag <flag> ...]\` returns a lane (\`tiny\` | \`normal\` | \`high-risk\`), the derived risk class, and the recommended next step. Use it as the entry point for any non-trivial change.

- \`tiny\` — patch directly, run validation, close with reason.
- \`normal\` — \`maestro spec new\` then \`maestro task from-spec\`, then \`maestro plan check\`.
- \`high-risk\` — Spec acceptance criteria plus threat-model evidence required.

## Two outputs per task

Every task close should answer two questions:

1. **Product delta** — what changed in user-facing or product behavior?
2. **Harness delta** — what should we change so the next agent has it easier? (memory ratchet, skill update, \`maestro doctor\` finding, friction note in this file). Answer "none" if truly nothing.

If the harness delta is non-trivial, capture it before the close so the next session inherits it.

## Daily commands

\`\`\`bash
maestro status --json                                 # what is in flight
maestro intake --paths <paths> [--flag <flag>]        # pre-code risk classifier
maestro task from-spec <path>                         # materialize a task from an authored spec
maestro mission decompose <pln-id> --file -           # heavy-mode: batch-create child tasks
maestro plan check --task <id> --plan-file <path>     # plan-time consistency check
maestro doctor                                        # scaffold + init.sh + verdict freshness
\`\`\`
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
    # kind can be 'manual' (human verification) or 'receipt-hint' (auto-tick
    # from --verified-by tags at completion). Use receipt-hint when the
    # criterion text is short and matches a --verified-by tag exactly.
# Optional: cap how many times the contract may be structurally amended
# (adding files to scope, changing intent, adding/removing criteria).
# Marking criteria met/unmet is workflow progress and does NOT count.
# amendmentBudget:
#   maxAmendments: 2
#   maxPathsPerAmendment: 5
#   forbiddenAmendmentPaths: []
# Optional: cap retries, wall-clock seconds, and tokens for this task.
# When any limit is exceeded, the next verdict request returns BLOCK.
# costBudget:
#   maxRetries: 3
#   maxWallClockSeconds: 1800
#   maxTokens: 100000
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
paths:
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
  {
    path: ".maestro/policies/owners.yaml",
    content: `# Decision-authority roles for Maestro policy enforcement.
# Each role is a list of GitHub usernames or team handles (e.g., "@org/team").
# Empty lists default to "any maintainer" (resolved via CODEOWNERS or repo-admin
# status if 'gh' CLI is available at runtime).
#
# - policy_approver: approves Policy file changes (L3+).
# - ratchet_approver: approves Ratchet promotions (L7+).
# - sensitive_waiver: signs off on changes to sensitive paths (L5+).
policy_approver: []
ratchet_approver: []
sensitive_waiver: []
`,
  },
  {
    path: ".maestro/policies/risk.yaml",
    content: `# Risk class derivation policy for Maestro Trust Substrate.
# Maps deterministic diff signals to a derived RiskClass.
# Rows are evaluated in order; first match wins.
# See ROADMAP §"Risk Class Enumeration" for the normative table.
#
# Valid derived_class values: low, medium, high, critical
kind: risk
id: risk-policy-default
version: "1"
rows:
  - signal: diff-intersects-sensitive-security
    derived_class: critical
    description: >-
      Diff intersects sensitive_paths.security set
      (auth/**, secrets/**, permissions/**, payments/**)
  - signal: diff-modifies-dependency-manifests
    derived_class: high
    description: >-
      Diff modifies dependency manifests
      (package.json, bun.lock, Cargo.toml, requirements.txt, etc.)
  - signal: diff-modifies-migrations
    derived_class: high
    description: >-
      Diff includes database migration files
      (paths matching policies/migration_paths)
  - signal: diff-modifies-ci-workflows
    derived_class: high
    description: >-
      Diff modifies CI workflow files
      (.github/workflows/**, .circleci/**, .gitlab-ci.yml)
  - signal: diff-modifies-policy-files
    derived_class: high
    description: >-
      Diff modifies policies/, ratchets/, or owners.yaml in .maestro/
  - signal: diff-modifies-build-config
    derived_class: medium
    description: >-
      Diff modifies build configuration
      (tsconfig.json, bunfig.toml, vite.config.*, etc.)
  - signal: diff-source-only
    derived_class: medium
    description: >-
      Any source code change not matched by the above rows
      (default for source changes)
  - signal: diff-docs-only
    derived_class: low
    description: >-
      Diff is docs-only, comment-only, or formatting-only
`,
  },
  {
    path: ".maestro/policies/autopilot.yaml",
    content: `# Autopilot policy for Maestro Trust Substrate.
# Controls whether Maestro may auto-merge and what witness level is required,
# per risk class. Disabled by default — enable only after L6 is shipped.
#
# auto_merge_allowed: whether maestro may auto-merge for each risk class.
# required_witness_level: minimum evidence trust level required before merge.
#
# Valid required_witness_level values:
#   witnessed-by-maestro
#   witnessed-by-ci
#   agent-claimed-locally
#   agent-claimed-and-not-reproducible
kind: autopilot
id: autopilot-policy-default
version: "1"
auto_merge_allowed:
  low: false      # Enable when L6 is active and evidence quality is verified
  medium: false   # Only eligible if all evidence is witnessed-by-maestro or witnessed-by-ci
  high: false     # Ineligible by default; human review required at L5
  critical: false # Always ineligible; human review required regardless of evidence
required_witness_level:
  low: witnessed-by-maestro
  medium: witnessed-by-maestro
  high: witnessed-by-maestro
  critical: witnessed-by-maestro
`,
  },
  {
    path: ".maestro/docs/HARNESS.md",
    content: `# Harness

This document explains the **harness** — the development infrastructure layer that wraps the product code in this repository.

## Product Delta vs Harness Delta

Every change in this repo falls into one of two buckets, and many touch both.

**Product Delta** — changes that deliver user-facing value:

- New features, bug fixes, API endpoints
- UI components, business logic
- User-facing documentation

**Harness Delta** — changes that improve the development process itself:

- New validation rules, policy updates
- Workflow improvements, skill enhancements
- Process documentation, risk flags

A single PR can carry both. The \`harness-delta\` evidence kind exists so the harness improvements are not invisible — they show up alongside product evidence in the same task.

## Work Types

Six classifications, mutually exclusive. See \`FEATURE_INTAKE.md\` for the decision tree.

- \`new-spec\` — new feature with no existing implementation
- \`spec-slice\` — part of an existing spec or feature area
- \`change-request\` — modify, fix, or refine existing behavior
- \`initiative\` — large cross-domain work, multiple tasks
- \`maintenance\` — deps, configuration, tooling
- \`harness-improvement\` — improve the development harness itself

## How this fits the rest of the system

- \`maestro intake\` returns the work type as part of its result. Run it before claiming a task.
- The \`harness-delta\` evidence kind is recorded when a task closes and the work touched \`.maestro/\`, \`policies/\`, \`skills/\`, or \`hooks/\`.
- See \`VALIDATION_LADDER.md\` for how harness changes get verified.
`,
  },
  {
    path: ".maestro/docs/FEATURE_INTAKE.md",
    content: `# Feature Intake Guide

Before claiming a task or creating a plan, run \`maestro intake --paths <paths> --json\` to classify the work.

## Classification Decision Tree

\`\`\`text
Start with IntakeResult and intendedPaths.

┌─ Any path under .maestro/ | policies/ | skills/ | hooks/?
│    └─ yes → harness-improvement
│
├─ multi-domain flag OR paths span 3+ top-level dirs?
│    └─ yes → initiative
│
├─ All paths are manifests / .github/** / root config?
│    └─ yes → maintenance
│
├─ None of the paths exist yet?
│    └─ yes → new-spec
│
├─ All paths share one src/features/<one>/ root?
│    └─ yes → spec-slice
│
└─ else → change-request
\`\`\`

First-match wins. A path that lands in \`.maestro/\` is \`harness-improvement\` even if other paths in the same call look like a \`spec-slice\`.

> **Note for brownfield codebases**: \`spec-slice\` requires the \`src/features/<name>/\` layout. Projects that organize code as \`src/<domain>/\` (without \`features/\`) will get \`change-request\` instead — that's the intended fallback. The two work types share most lane-driven next-steps, so adoption doesn't require restructuring.

## Common Patterns

| User Request | Work Type | Rationale |
|---|---|---|
| "Add new API endpoint" | spec-slice | Extending existing API surface |
| "Fix bug in login" | change-request | Modifying existing behavior |
| "Update dependencies" | maintenance | Chore-type work |
| "New auth system" | initiative | Multi-domain, large scope |
| "Add risk policy" | harness-improvement | Harness modification |

## Acting on the result

The intake response includes \`recommendedNextStep\` (lane-derived) and \`recommendedNextSteps\` (work-type + lane derived). Prefer the latter — it's more specific.

\`IntakeResult.harnessImpact\` is \`true\` whenever any path falls under \`.maestro/\`, \`policies/\`, \`skills/\`, or \`hooks/\` — independent of the work type. When it's true, plan to record a \`harness-delta\` evidence row at task close.
`,
  },
  {
    path: ".maestro/docs/VALIDATION_LADDER.md",
    content: `# Validation Ladder

The harness-experimental project models verification as a 7-rung ladder. Maestro's canonical verification protocol (\`maestro-verify\`) covers all 7 rungs but groups them under 6 steps.

## The 7-Rung Ladder

1. **Format** — code formatting checks (prettier, etc.)
2. **Lint** — static analysis (eslint, architecture lint, etc.)
3. **Type** — type checking (\`tsc --noEmit\`, etc.)
4. **Integration** — integration tests
5. **E2E** — end-to-end tests, compiled-binary tests
6. **Platform** — platform-specific tests, deploy readiness
7. **Release** — final verdict, release checks

## Mapping to \`maestro-verify\`

- **Plan** → Pre-validation (read spec, contracts, prior evidence)
- **Implement** → Code changes
- **Verify** → Rungs 1–5 (format / lint / type / integration / e2e)
- **ProofMap** → Evidence coverage check
- **Verdict** → Rungs 6–7 (platform / release)
- **Branch** → Action based on verdict (merge, rollback, retry)

## Harness-Specific Validation

For \`harness-improvement\` work types, additional checks apply:

- Policy schema validation via \`maestro policy check\`
- Skill self-tests via \`bun run check:bundled-skills\` and \`bun run check:skills\`
- Contract amendment evidence when contracts change
- One \`harness-delta\` evidence row per task that touched \`.maestro/\`, \`policies/\`, \`skills/\`, or \`hooks/\`
`,
  },
  {
    path: "AGENTS.md",
    overwritePolicy: "managed-block",
    content: `# Project Conventions

Repo-level conventions for agents working in this codebase. The harness pointer surface
lives in \`.maestro/AGENTS.md\`; this file holds code conventions, build commands, and
feature boundaries.

## Build / test / verify

Fill in the commands an agent should run before claiming a task done:

\`\`\`bash
# build:   <how to build>
# test:    <how to run tests>
# lint:    <if any>
# format:  <if any>
\`\`\`

## Layout

- \`src/\` — application source
- \`tests/\` — automated tests
- \`.maestro/\` — harness state (read \`.maestro/AGENTS.md\` first)

## Conventions

- Match existing code style; use established libraries before adding new ones.
- Surgical edits only — touch what the task requires.
- Bump the relevant version when behavior changes.

## See also

- \`.maestro/MAESTRO.md\` — read order, lane policy, daily commands
- \`.maestro/docs/HARNESS.md\` — product-delta vs harness-delta model
- \`.maestro/docs/FEATURE_INTAKE.md\` — work-type classification decision tree
- \`.maestro/docs/VALIDATION_LADDER.md\` — 7-step verification protocol
`,
  },
  {
    path: ".maestro/policies/release.yaml",
    content: `# Release gate policy for Maestro Trust Substrate.
# Controls release-time enforcement rules.
# See ROADMAP L8 for the full release gate specification.
#
# require_signed_commits: block release if any commit in the range is unsigned.
# require_proof_map_complete: block release if the proof map has unfilled entries.
kind: release
id: release-policy-default
version: "1"
require_signed_commits: false     # Tighten at L8 when signing is enforced repo-wide
require_proof_map_complete: false # Tighten at L8 when proof maps are required
`,
  },
  // Cold-start trigger at the repo root; distinct from `.maestro/bootstrap/init.sh`
  // (project bootstrapper for dependency installs).
  {
    path: "init.sh",
    executable: true,
    overwritePolicy: "never",
    content: `#!/usr/bin/env bash
# Project init -- emitted once by \`maestro setup\` and never overwritten.
# Edit freely; Maestro will not touch this file again unless you delete it.
set -euo pipefail

# Check maestro is available
if ! command -v maestro &> /dev/null; then
  echo "maestro not found in PATH." >&2
  echo "Install maestro or ensure it's in your PATH." >&2
  echo "If installed to a custom location, add it to PATH or set MAESTRO_BIN." >&2
  exit 1
fi

# Health gate -- exits non-zero if .maestro/ scaffold is broken.
maestro doctor

# Cold-start view -- one-screen resume snapshot.
maestro status
`,
  },
];
