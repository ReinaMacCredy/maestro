---
name: setup
description: Scaffold persistent project context — product definition, tech stack, and guidelines. Interviews you about your project and generates context files that all Maestro agents reference.
argument-hint: "[--reset]"
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, AskUserQuestion
disable-model-invocation: true
---

# Setup — Project Context Scaffolding

> Inspired by [Conductor](https://github.com/gemini-cli-extensions/conductor). Adapted for Maestro's architecture.

Interview the user to create persistent project context documents that all Maestro agents reference for deeper project understanding.

## Arguments

`$ARGUMENTS`

- `--reset`: Delete all existing context files and start fresh.
- Default (no args): Run setup interview. If context already exists, offer to update or skip.

## Step 1: Handle --reset

If `$ARGUMENTS` contains `--reset`:

1. Check if `.maestro/context/` exists
2. If it does, confirm with the user:
   ```
   AskUserQuestion(
     questions: [{
       question: "This will delete all project context files in .maestro/context/. Are you sure?",
       header: "Reset Context",
       options: [
         { label: "Yes, reset", description: "Delete all context files and start fresh" },
         { label: "Cancel", description: "Keep existing context" }
       ],
       multiSelect: false
     }]
   )
   ```
3. If confirmed: `rm -rf .maestro/context/` and report "Context reset. Run `/setup` to create new context."
4. Stop.

## Step 2: Check Existing Context

```
Glob(pattern: ".maestro/context/*.md")
```

If context files already exist, ask the user:

```
AskUserQuestion(
  questions: [{
    question: "Project context already exists. What would you like to do?",
    header: "Existing Context",
    options: [
      { label: "Update", description: "Re-run setup and overwrite existing files" },
      { label: "View", description: "Show current context files and exit" },
      { label: "Cancel", description: "Keep existing context unchanged" }
    ],
    multiSelect: false
  }]
)
```

**On View**: Read and display each file in `.maestro/context/`, then stop.
**On Cancel**: Stop.
**On Update**: Continue to Step 3.

## Step 3: Detect Project Maturity

Classify the project as **Brownfield** (existing) or **Greenfield** (new).

**Brownfield indicators** (check in order, stop at first match):
1. `package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`, `build.gradle`, `pom.xml` exists
2. `src/`, `app/`, or `lib/` directory contains code files
3. `.git` directory exists with commits (`git log --oneline -1` succeeds)

**Greenfield**: None of the above indicators found.

**For Brownfield projects**:
1. Announce: "Detected an existing project. I'll analyze it before asking questions."
2. Read key files to infer context:
   - `README.md` (if exists) — project purpose
   - `package.json` / `pyproject.toml` / `Cargo.toml` / `go.mod` — dependencies and tech stack
   - `CLAUDE.md` (if exists) — existing conventions
3. Store inferences for use in subsequent questions.

**For Greenfield projects**:
1. Announce: "New project detected. I'll help you define the project context from scratch."

## Step 4: Create Context Directory

```bash
mkdir -p .maestro/context
```

## Step 5: Product Definition Interview

Generate `product.md` — what the project is, who it's for, and what it does.

**For Brownfield**: Pre-fill answers from Step 3 analysis. Ask the user to confirm or correct.

Ask questions sequentially (one at a time). Limit to 3 questions max.

**Question 1** — Project purpose:
```
AskUserQuestion(
  questions: [{
    question: "What does this project do? (one sentence)",
    header: "Product Definition",
    options: [
      { label: "{inferred purpose if brownfield}", description: "Based on README/package.json analysis" },
      { label: "Other", description: "Type your own description" }
    ],
    multiSelect: false
  }]
)
```

For greenfield, omit the inferred option — just ask the open-ended question.

**Question 2** — Target users:
```
AskUserQuestion(
  questions: [{
    question: "Who are the primary users?",
    header: "Target Users",
    options: [
      { label: "Developers", description: "Library, CLI tool, or developer-facing API" },
      { label: "End users", description: "Web app, mobile app, or consumer-facing product" },
      { label: "Internal team", description: "Internal tool, admin dashboard, or ops tooling" },
      { label: "Other", description: "Type your own" }
    ],
    multiSelect: false
  }]
)
```

**Question 3** — Key features (optional — skip if brownfield with clear README):
```
AskUserQuestion(
  questions: [{
    question: "What are the 2-3 most important features or capabilities?",
    header: "Key Features",
    options: [
      { label: "Auto-generate from analysis", description: "I'll infer from the codebase" },
      { label: "Other", description: "Type your own list" }
    ],
    multiSelect: false
  }]
)
```

**Draft and write `product.md`**:

```markdown
# Product Definition

## Purpose
{user's answer to Q1}

## Target Users
{user's answer to Q2}

## Key Features
{user's answer to Q3, or inferred list}
```

Write to `.maestro/context/product.md`.

## Step 6: Tech Stack Interview

Generate `tech-stack.md` — languages, frameworks, tools.

**For Brownfield**: Infer the tech stack from config files (Step 3). Present for confirmation.

```
AskUserQuestion(
  questions: [{
    question: "Is this your tech stack?\n\n{inferred stack summary}",
    header: "Tech Stack",
    options: [
      { label: "Yes, correct", description: "Use the detected tech stack" },
      { label: "Needs changes", description: "Let me correct or add to it" },
      { label: "Other", description: "Type the full tech stack manually" }
    ],
    multiSelect: false
  }]
)
```

**For Greenfield**: Ask directly:

```
AskUserQuestion(
  questions: [{
    question: "What tech stack will this project use? (languages, frameworks, database, etc.)",
    header: "Tech Stack",
    options: [
      { label: "Other", description: "Type your tech stack" }
    ],
    multiSelect: false
  }]
)
```

**Draft and write `tech-stack.md`**:

```markdown
# Tech Stack

## Languages
- {language 1}
- {language 2}

## Frameworks
- {framework 1}
- {framework 2}

## Tools & Infrastructure
- Package manager: {manager}
- Database: {db, if applicable}
- CI/CD: {ci, if applicable}
```

Write to `.maestro/context/tech-stack.md`.

## Step 7: Guidelines Interview

Generate `guidelines.md` — coding conventions, design principles, non-functional requirements.

```
AskUserQuestion(
  questions: [{
    question: "Any specific guidelines or principles for this project?",
    header: "Project Guidelines",
    options: [
      { label: "Auto-generate from analysis", description: "I'll infer from CLAUDE.md, linter configs, and conventions" },
      { label: "TDD-first", description: "Test-driven development, high coverage" },
      { label: "Move fast", description: "Ship quickly, iterate later" },
      { label: "Security-first", description: "Input validation, audit logging, secure defaults" },
      { label: "Other", description: "Type your own guidelines" }
    ],
    multiSelect: true
  }]
)
```

**Draft and write `guidelines.md`**:

```markdown
# Project Guidelines

## Development Principles
- {selected principles}

## Conventions
- {inferred from CLAUDE.md or user input}

## Non-Functional Requirements
- {performance, security, accessibility, etc.}
```

Write to `.maestro/context/guidelines.md`.

## Step 8: Summary and Commit

1. Display a summary of all generated files:

```
## Project Context Created

**Files**:
- `.maestro/context/product.md` — Product definition
- `.maestro/context/tech-stack.md` — Technology stack
- `.maestro/context/guidelines.md` — Project guidelines

These files will be automatically injected into all Maestro agent contexts.

To update: `/setup`
To reset: `/setup --reset`
To view: `/setup` → View
```

2. Commit the context files:
```bash
git add .maestro/context/
git commit -m "chore(setup): scaffold project context files"
```
