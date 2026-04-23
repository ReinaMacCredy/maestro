---
name: maestro-setup
description: Set up a repository for Maestro-guided, human-in-the-loop agent work. Use when a project needs Maestro-owned context docs, evidence-first onboarding, root AGENTS.md guidance, language style guides, or a setup report before implementation work begins.
---

# Maestro Setup

Use this skill to create or refresh Maestro-owned project context. This is the
canonical v1 setup behavior. Future `maestro setup` CLI commands must match this
skill's behavior rather than inventing a separate setup model.

## Core Contract

- Skill-first, CLI-second. Do not add or assume a `maestro setup` CLI command.
- Non-interactive by default. Do not ask questions during ordinary setup.
- Evidence-first. Infer from repo files and mark uncertain facts as TODO.
- Keep substantial setup content under `.maestro/context/`.
- Keep root `AGENTS.md` short: only add or update the managed Maestro pointer block.
- Preserve user content outside managed markers.
- Overwrite `.maestro/setup-report.md` on each run.
- Do not fetch from the web. Use frozen snapshots in `reference/styleguides/`.

## Managed Markers

Use these exact markers.

Root `AGENTS.md` block:

```md
<!-- maestro-setup:start -->
...
<!-- maestro-setup:end -->
```

Generated context section:

```md
<!-- maestro-setup:generated:start -->
...
<!-- maestro-setup:generated:end -->
```

If a managed block exists, replace only the block body. If it does not exist,
append a new block. Never rewrite the whole file unless the file is new.

## Setup Flow

### 1. Inspect Current State

Read:

- root `AGENTS.md`, `CLAUDE.md`, `README*`, and package manifests when present
- manifests such as `package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`,
  `pom.xml`, `build.gradle`, `Makefile`, `CMakeLists.txt`, `Package.swift`
- config files that reveal lint, format, test, build, and deployment behavior
- top-level source/test directories and representative file extensions
- existing `.maestro/context/` docs when present

Record every evidence source used for `.maestro/setup-report.md`.

### 2. Detect Languages

Detect languages from manifests first, then file extensions. Copy only matched
frozen snapshots from `reference/styleguides/` to
`.maestro/context/code_styleguides/`.

Use this map:

| Guide | Signals |
|---|---|
| `angularjs.md` | AngularJS dependency, `angular.module`, Angular 1 style files |
| `common-lisp.md` | `.lisp`, `.lsp`, `.asd` |
| `cpp.md` | `.cc`, `.cpp`, `.cxx`, `.hh`, `.hpp`, `CMakeLists.txt` |
| `csharp.md` | `.cs`, `.csproj`, `.sln` |
| `go.md` | `go.mod`, `.go` |
| `html-css.md` | `.html`, `.css`, `.scss`, `.sass`, `.less` |
| `javascript.md` | `.js`, `.jsx`, JavaScript package metadata |
| `java.md` | `.java`, `pom.xml`, `build.gradle` |
| `json.md` | `.json`, JSON schema files |
| `markdown.md` | `.md`, `.mdx` |
| `objective-c.md` | `.m`, `.mm`, `.h` with Objective-C patterns |
| `python.md` | `.py`, `pyproject.toml`, `requirements.txt`, `setup.py` |
| `r.md` | `.R`, `.r`, `DESCRIPTION` |
| `shell.md` | `.sh`, `.bash`, `.zsh`, shell shebangs |
| `swift.md` | `.swift`, `Package.swift`, Xcode Swift targets |
| `typescript.md` | `.ts`, `.tsx`, `tsconfig.json` |
| `vimscript.md` | `.vim`, `.vimrc`, Vim plugin metadata |
| `xml.md` | `.xml`, `.xsd`, XML schemas |

If Dart or Kotlin is detected, do not copy external guides in v1. Mention that
external Dart/Kotlin references were detected but intentionally excluded.

### 3. Create Or Refresh Context Docs

Ensure `.maestro/context/` exists. Create or update these files using the
templates in `reference/context-templates/`:

- `index.md`
- `architecture.md`
- `product-sense.md`
- `quality-gates.md`
- `security.md`
- `workflow.md`
- `planning.md`

Each new file must contain:

1. a generated block using the context managed markers
2. a `## User Notes` section outside the managed block

For existing files, update only the generated block. Preserve all user-written
content outside it.

### 4. Update Root AGENTS.md

Create or update the root `AGENTS.md` managed block so agents know to reflect on
`.maestro/context/index.md` before non-trivial work.

The block must say:

- load `.maestro/context/index.md` first
- open only the specific context docs relevant to the task
- follow detected language guides under `.maestro/context/code_styleguides/`
- preserve user content outside managed setup sections
- if context docs conflict with closer repo instructions, follow the closer
  instruction file and report the conflict

### 5. Write Setup Report

Overwrite `.maestro/setup-report.md` from `reference/setup-report-template.md`.

The report must include:

- timestamp
- files created, updated, and skipped
- languages detected and guides copied
- evidence sources used
- TODOs left in generated docs
- warnings
- recommended next action

### 6. Final Response

Report:

- what was created or updated
- detected languages
- copied guide files
- notable TODOs or warnings
- the next command or action the user should take

Do not claim setup is complete unless root `AGENTS.md`, context docs,
language guides, and `.maestro/setup-report.md` have all been checked.

## Context Doc Intent

- `architecture.md`: current repo shape, owned modules, boundaries, known
  pressure points.
- `product-sense.md`: audience, product goals, UX/product guardrails.
- `quality-gates.md`: build/test/lint/typecheck commands and review bar.
- `security.md`: trust boundaries, sensitive files, approval gates.
- `workflow.md`: human/agent delivery loop, review expectations, completion bar.
- `planning.md`: thin policy bridge to `maestro-brainstorm`, `maestro-plan`,
  `.maestro/plans/`, `maestro-task`, and handoff.
- `index.md`: map of all context docs and copied style guides.

## Future CLI Contract

The later CLI should mirror this skill exactly:

- `maestro setup --dry-run --json`
- `maestro setup --json`
- `maestro setup check --json`
- `maestro setup languages --json`

Do not design extra CLI behavior while running this skill.
