---
name: maestro:setup
description: "Scaffolds project context (product, tech stack, coding guidelines, product guidelines, workflow) and initializes track registry. Use for first-time project onboarding."
argument-hint: "[--reset]"
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, AskUserQuestion
disable-model-invocation: true
---

# Maestro Setup -- Project Context Scaffolding

Interview the user to create persistent project context documents. These files are referenced by all `maestro:*` skills for deeper project understanding.

## Arguments

`$ARGUMENTS`

- `--reset`: Delete all existing context files and start fresh.
- Default (no args): Run setup interview. If context already exists, offer to update or skip.

---

## CRITICAL: Tool Call Discipline

You must validate the success of every tool call. If any tool call fails, halt immediately, announce the failure, and await instructions.

When using AskUserQuestion, do not repeat the question in plain text before calling the tool.

---

## Step 1: Handle --reset

If `$ARGUMENTS` contains `--reset`:

1. Check if `.maestro/context/` exists
2. If it does, confirm with the user:
   ```
   AskUserQuestion(
     questions: [{
       question: "This will delete all project context files in .maestro/context/, the tracks registry, and the setup state file. Are you sure?",
       header: "Reset",
       options: [
         { label: "Yes, reset everything", description: "Delete context files, tracks.md, and setup_state.json; start fresh" },
         { label: "Cancel", description: "Keep existing context" }
       ],
       multiSelect: false
     }]
   )
   ```
3. If confirmed: run `rm -rf .maestro/context/ .maestro/tracks.md .maestro/setup_state.json` and report "Context reset. Run `/maestro:setup` to create new context."
4. Stop.

## Step 2: Check Setup State (Resume Protocol)

Check for an existing `.maestro/setup_state.json`:

```bash
cat .maestro/setup_state.json 2>/dev/null || echo "{}"
```

If the file exists and contains a `last_successful_step` value, the previous run was interrupted. Ask:

```
AskUserQuestion(
  questions: [{
    question: "A previous setup run was interrupted after step \"<last_successful_step>\". What would you like to do?\n\nCompleted steps will be skipped automatically.",
    header: "Resume Setup",
    options: [
      { label: "Resume from where I left off", description: "Skip already-completed steps" },
      { label: "Start over", description: "Ignore previous progress and run all steps" }
    ],
    multiSelect: false
  }]
)
```

If "Start over": delete `.maestro/setup_state.json` and treat `last_successful_step` as empty.

If "Resume": retain `last_successful_step` and skip steps whose names appear in the completed set below.

**Step name registry** (used for skip logic):
- `check_existing_context`
- `detect_maturity`
- `create_context_directory`
- `product_definition`
- `tech_stack`
- `coding_guidelines`
- `product_guidelines`
- `workflow_config`
- `tracks_registry`
- `style_guides`
- `index_md`
- `first_track`

A step is skipped if its name sorts at or before `last_successful_step` in the registry order above.

**State write helper**: After completing each major step, write:
```bash
echo '{"last_successful_step": "<step_name>"}' > .maestro/setup_state.json
```

## Step 3: Check Existing Context

_Skip this step if `last_successful_step` >= `check_existing_context`._

```
Glob(pattern: ".maestro/context/*.md")
```

If context files already exist, ask:

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

- **View**: Read and display each file in `.maestro/context/`, then stop.
- **Cancel**: Stop.
- **Update**: Continue to Step 4.

Write state: `check_existing_context`

## Step 4: Detect Project Maturity

_Skip this step if `last_successful_step` >= `detect_maturity`._

Classify the project as **Brownfield** (existing) or **Greenfield** (new).

**Brownfield indicators** (check in order, stop at first match):
1. `package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`, `build.gradle`, `pom.xml`, `Gemfile`, `composer.json` exists
2. `src/`, `app/`, or `lib/` directory contains code files
3. `.git` directory exists with more than 5 commits

**Greenfield**: None of the above indicators found.

**For Brownfield projects**:

1. Check for uncommitted changes and warn if any exist:
   ```bash
   git status --porcelain 2>/dev/null
   ```
   If the output is non-empty, report: "[!] Uncommitted changes detected. Consider committing or stashing before scanning to ensure a clean baseline." Then ask:
   ```
   AskUserQuestion(
     questions: [{
       question: "There are uncommitted changes in this repository. How would you like to proceed?",
       header: "Uncommitted Changes",
       options: [
         { label: "Continue anyway", description: "Proceed with the scan; changes are noted" },
         { label: "Abort", description: "Stop setup so I can commit or stash first" }
       ],
       multiSelect: false
     }]
   )
   ```
   If "Abort": stop.

2. Ask explicit scan permission:
   ```
   AskUserQuestion(
     questions: [{
       question: "May I perform a read-only scan of your codebase to infer project context? No files will be modified.",
       header: "Scan Permission",
       options: [
         { label: "Yes, scan the codebase", description: "Read-only analysis to pre-fill answers" },
         { label: "No, I'll answer manually", description: "Skip the scan; ask all questions without pre-fill" }
       ],
       multiSelect: false
     }]
   )
   ```

3. If permission granted, scan using efficient file listing:
   - Use `git ls-files` to enumerate tracked files (respects `.gitignore`):
     ```bash
     git ls-files 2>/dev/null | head -200
     ```
   - Read key files to infer context:
     - `README.md` (if exists) -- project purpose
     - Package manifest (`package.json` / `pyproject.toml` / `Cargo.toml` / `go.mod`) -- dependencies, tech stack
     - `CLAUDE.md` (if exists) -- existing conventions
     - Linter/formatter configs (`.eslintrc*`, `prettier*`, `ruff.toml`, `clippy.toml`) -- code standards
   - For any file larger than 1 MB: read only the first 20 lines and last 20 lines.
   - Store inferences for use in subsequent questions.

4. Announce: "Detected an existing project. I'll use my analysis to pre-fill answers where possible."

**For Greenfield projects**:
1. Announce: "New project detected. I'll help you define the project context from scratch."
2. If no `.git` directory exists, offer to initialize:
   ```
   AskUserQuestion(
     questions: [{
       question: "No git repository found. Initialize one now?",
       header: "Git Init",
       options: [
         { label: "Yes, run git init", description: "Initialize a new git repository in this directory" },
         { label: "Skip", description: "Continue without git" }
       ],
       multiSelect: false
     }]
   )
   ```
   If yes: `git init`

Write state: `detect_maturity`

## Step 5: Create Context Directory

_Skip this step if `last_successful_step` >= `create_context_directory`._

```bash
mkdir -p .maestro/context
```

Write state: `create_context_directory`

## Step 6: Product Definition Interview

_Skip this step if `last_successful_step` >= `product_definition`._

Generate `.maestro/context/product.md` -- what the project is, who it's for, what it does.

**For each question below, first present the Interactive vs Autogenerate choice:**

```
AskUserQuestion(
  questions: [{
    question: "How would you like to provide the product definition?",
    header: "Product Definition -- Mode",
    options: [
      { label: "Interactive", description: "Answer questions step by step" },
      { label: "Autogenerate", description: "I'll infer everything from the codebase analysis" }
    ],
    multiSelect: false
  }]
)
```

If **Autogenerate**: use inferences from Step 4 to populate all fields and skip the detailed questions below. Go directly to writing `product.md`.

If **Interactive**: ask the following questions sequentially (one at a time). Limit to 3 questions max.

**Question 1** -- Project purpose:
```
AskUserQuestion(
  questions: [{
    question: "What does this project do? (one sentence)",
    header: "Purpose",
    options: [
      { label: "{inferred purpose}", description: "Based on README/package analysis" },
      { label: "Let me describe it", description: "Type your own description" }
    ],
    multiSelect: false
  }]
)
```

For greenfield or when no inference is available, provide only a single "Let me describe it" option.

**Question 2** -- Target users:
```
AskUserQuestion(
  questions: [{
    question: "Who are the primary users?",
    header: "Users",
    options: [
      { label: "Developers", description: "Library, CLI tool, or developer-facing API" },
      { label: "End users", description: "Web app, mobile app, or consumer-facing product" },
      { label: "Internal team", description: "Internal tool, admin dashboard, or ops tooling" }
    ],
    multiSelect: false
  }]
)
```

**Question 3** -- Key features (skip if brownfield with clear README):
```
AskUserQuestion(
  questions: [{
    question: "What are the 2-3 most important features or capabilities?",
    header: "Features",
    options: [
      { label: "Auto-generate from analysis", description: "I'll infer from the codebase" },
      { label: "Let me list them", description: "Type your own list" }
    ],
    multiSelect: false
  }]
)
```

**Write `product.md`**:

```markdown
# Product Definition

## Purpose
{user's answer to Q1, or autogenerated}

## Target Users
{user's answer to Q2, or autogenerated}

## Key Features
{user's answer to Q3, or inferred list}
```

Write to `.maestro/context/product.md`.

Write state: `product_definition`

## Step 7: Tech Stack Interview

_Skip this step if `last_successful_step` >= `tech_stack`._

Generate `.maestro/context/tech-stack.md` -- languages, frameworks, tools.

**First, present the Interactive vs Autogenerate choice:**

```
AskUserQuestion(
  questions: [{
    question: "How would you like to provide the tech stack?",
    header: "Tech Stack -- Mode",
    options: [
      { label: "Interactive", description: "Review and confirm the detected stack or enter it manually" },
      { label: "Autogenerate", description: "I'll infer the full tech stack from config files" }
    ],
    multiSelect: false
  }]
)
```

If **Autogenerate**: use inferences from Step 4. Skip detailed questions and go directly to writing `tech-stack.md`.

If **Interactive**:

**For Brownfield**: Present the inferred stack for confirmation:
```
AskUserQuestion(
  questions: [{
    question: "Is this your tech stack?\n\n{inferred stack summary}",
    header: "Tech Stack",
    options: [
      { label: "Yes, correct", description: "Use the detected tech stack" },
      { label: "Needs changes", description: "Let me correct or add to it" }
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
      { label: "Let me describe it", description: "Type your tech stack" }
    ],
    multiSelect: false
  }]
)
```

**Write `tech-stack.md`**:

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

Write state: `tech_stack`

## Step 8: Coding Guidelines Interview

_Skip this step if `last_successful_step` >= `coding_guidelines`._

Generate `.maestro/context/guidelines.md` -- coding conventions, design principles, non-functional requirements.

**First, present the Interactive vs Autogenerate choice:**

```
AskUserQuestion(
  questions: [{
    question: "How would you like to define coding guidelines?",
    header: "Coding Guidelines -- Mode",
    options: [
      { label: "Interactive", description: "Select from common principles and conventions" },
      { label: "Autogenerate", description: "I'll infer from CLAUDE.md, linter configs, and conventions" }
    ],
    multiSelect: false
  }]
)
```

If **Autogenerate**: infer from `CLAUDE.md`, linter/formatter configs found in Step 4, and project conventions. Skip the question below and write the file.

If **Interactive**:

```
AskUserQuestion(
  questions: [{
    question: "Any specific coding guidelines or principles for this project?",
    header: "Coding Guidelines",
    options: [
      { label: "TDD-first", description: "Test-driven development, high coverage" },
      { label: "Move fast", description: "Ship quickly, iterate later" },
      { label: "Security-first", description: "Input validation, audit logging, secure defaults" },
      { label: "Accessibility-first", description: "WCAG compliance, semantic HTML, screen reader support" },
      { label: "Let me describe", description: "Type custom guidelines" }
    ],
    multiSelect: true
  }]
)
```

**Write `guidelines.md`**:

```markdown
# Coding Guidelines

## Development Principles
- {selected principles}

## Conventions
- {inferred from CLAUDE.md or user input}

## Non-Functional Requirements
- {performance, security, accessibility, etc.}
```

Write to `.maestro/context/guidelines.md`.

Write state: `coding_guidelines`

## Step 9: Product Guidelines Interview

_Skip this step if `last_successful_step` >= `product_guidelines`._

Generate `.maestro/context/product-guidelines.md` -- prose style, branding, UX principles, voice/tone. This is separate from coding guidelines.

**First, present the Interactive vs Autogenerate choice:**

```
AskUserQuestion(
  questions: [{
    question: "How would you like to define product guidelines (voice, tone, UX principles, branding)?",
    header: "Product Guidelines -- Mode",
    options: [
      { label: "Interactive", description: "Answer questions about brand voice and UX principles" },
      { label: "Autogenerate", description: "I'll generate sensible defaults based on the product type" },
      { label: "Skip", description: "No product guidelines needed for this project" }
    ],
    multiSelect: false
  }]
)
```

If **Skip**: write a minimal placeholder file and continue.

If **Autogenerate**: generate defaults appropriate for the product type (e.g., developer tool voice vs. consumer app voice). Skip detailed questions and write the file.

If **Interactive**:

**Question 1** -- Voice and tone:
```
AskUserQuestion(
  questions: [{
    question: "What is the voice and tone for written content (UI copy, docs, error messages)?",
    header: "Voice & Tone",
    options: [
      { label: "Professional and direct", description: "Clear, concise, no fluff. Suitable for developer tools." },
      { label: "Friendly and approachable", description: "Warm, conversational. Suitable for consumer apps." },
      { label: "Formal and authoritative", description: "Precise, structured. Suitable for enterprise/compliance." },
      { label: "Playful and energetic", description: "Fun, engaging. Suitable for consumer/gaming." },
      { label: "Let me describe", description: "Type custom voice/tone guidelines" }
    ],
    multiSelect: false
  }]
)
```

**Question 2** -- UX principles:
```
AskUserQuestion(
  questions: [{
    question: "What are the core UX principles?",
    header: "UX Principles",
    options: [
      { label: "Progressive disclosure", description: "Show only what's needed; reveal complexity on demand" },
      { label: "Zero-config defaults", description: "Work out of the box; power users can customize" },
      { label: "Accessible by default", description: "WCAG AA minimum; keyboard navigable; screen reader support" },
      { label: "Mobile-first", description: "Design for small screens first, scale up" },
      { label: "Let me describe", description: "Type custom UX principles" }
    ],
    multiSelect: true
  }]
)
```

**Question 3** -- Branding (skip if no UI):
```
AskUserQuestion(
  questions: [{
    question: "Any branding or visual identity constraints? (color palette, typography, logo usage)",
    header: "Branding",
    options: [
      { label: "No branding constraints", description: "Skip; no visual identity rules" },
      { label: "Let me describe", description: "Type branding guidelines or link to a style guide" }
    ],
    multiSelect: false
  }]
)
```

**Write `product-guidelines.md`**:

```markdown
# Product Guidelines

## Voice & Tone
{user's answer or autogenerated}

## UX Principles
- {selected principles}

## Branding
{branding constraints or "No branding constraints defined."}

## Writing Style
- Use active voice.
- Prefer short sentences (under 20 words).
- Error messages: state what happened, why, and how to fix it.
- Avoid jargon unless the audience is technical.
```

Write to `.maestro/context/product-guidelines.md`.

Write state: `product_guidelines`

## Step 10: Workflow Configuration

_Skip this step if `last_successful_step` >= `workflow_config`._

Generate `.maestro/context/workflow.md` -- task methodology, commit strategy, quality targets.

This file is the **source of truth** for how `/maestro:implement` executes tasks.

**First, present the Interactive vs Autogenerate choice:**

```
AskUserQuestion(
  questions: [{
    question: "How would you like to configure the workflow?",
    header: "Workflow -- Mode",
    options: [
      { label: "Interactive", description: "Answer questions about methodology and commit strategy" },
      { label: "Autogenerate", description: "Use recommended defaults (TDD, 80% coverage, per-task commits)" }
    ],
    multiSelect: false
  }]
)
```

If **Autogenerate**: use TDD, 80% coverage, per-task commits, git notes for summaries. Skip detailed questions and write the file.

If **Interactive**:

**Question 1** -- Methodology:
```
AskUserQuestion(
  questions: [{
    question: "What development methodology should tasks follow?",
    header: "Methodology",
    options: [
      { label: "TDD (Recommended)", description: "Write failing tests first, then implement. Red-Green-Refactor." },
      { label: "Ship-fast", description: "Implement first, add tests after. Faster but less rigorous." },
      { label: "Custom", description: "Define your own workflow" }
    ],
    multiSelect: false
  }]
)
```

**Question 2** -- Coverage target:
```
AskUserQuestion(
  questions: [{
    question: "What test coverage target for new code?",
    header: "Coverage",
    options: [
      { label: "80% (Recommended)", description: "Good balance of coverage and velocity" },
      { label: "90%", description: "High coverage, slower velocity" },
      { label: "60%", description: "Basic coverage, maximum velocity" },
      { label: "No target", description: "Don't enforce coverage thresholds" }
    ],
    multiSelect: false
  }]
)
```

**Question 3** -- Commit frequency:
```
AskUserQuestion(
  questions: [{
    question: "How often should implementation commit?",
    header: "Commits",
    options: [
      { label: "Per-task (Recommended)", description: "Atomic commit after each task completes. Fine-grained history." },
      { label: "Per-phase", description: "Commit after each phase completes. Fewer, larger commits." }
    ],
    multiSelect: false
  }]
)
```

**Question 4** -- Summary storage:
```
AskUserQuestion(
  questions: [{
    question: "Where should task summaries be stored?",
    header: "Summaries",
    options: [
      { label: "Git notes (Recommended)", description: "Attach detailed summaries as git notes on commits" },
      { label: "Commit messages", description: "Include full summary in the commit message body" },
      { label: "Neither", description: "No additional summaries beyond standard commit messages" }
    ],
    multiSelect: false
  }]
)
```

**Write `workflow.md`** using the template from `reference/workflow-template.md`.

Write to `.maestro/context/workflow.md`.

Write state: `workflow_config`

## Step 11: Initialize Tracks Registry

_Skip this step if `last_successful_step` >= `tracks_registry`._

Create the tracks registry file that `/maestro:new-track` and `/maestro:status` will use.

```markdown
# Tracks Registry

> Managed by Maestro. Do not edit manually.
> Status markers: `[ ]` New | `[~]` In Progress | `[x]` Complete

---
```

Write to `.maestro/tracks.md`.

Write state: `tracks_registry`

## Step 12: Code Style Guides (Optional)

_Skip this step if `last_successful_step` >= `style_guides`._

If the tech stack includes languages with available style guides, offer to copy them.

Available guides in `reference/styleguides/`:
- `python.md` -- Python style (Google-based)
- `typescript.md` -- TypeScript style
- `javascript.md` -- JavaScript style
- `go.md` -- Go style (effective Go)
- `general.md` -- Universal coding principles
- `cpp.md` -- C++ style
- `csharp.md` -- C# style
- `dart.md` -- Dart/Flutter style
- `html-css.md` -- HTML and CSS style

```
AskUserQuestion(
  questions: [{
    question: "Copy code style guides to your project? (based on detected stack: {languages})\n\nAvailable guides: python, typescript, javascript, go, general, cpp, csharp, dart, html-css",
    header: "Style Guides",
    options: [
      { label: "Yes, copy relevant guides", description: "Copy style guides for detected languages to .maestro/context/code_styleguides/" },
      { label: "Yes, copy all guides", description: "Copy all 9 style guides" },
      { label: "Skip", description: "No code style guides needed" }
    ],
    multiSelect: false
  }]
)
```

If yes:
1. `mkdir -p .maestro/context/code_styleguides`
2. Copy relevant (or all) guide files from this skill's `reference/styleguides/` to `.maestro/context/code_styleguides/`

Write state: `style_guides`

## Step 13: Generate Index File

_Skip this step if `last_successful_step` >= `index_md`._

Generate `.maestro/context/index.md` -- a navigation file that links all context files and the tracks registry. Other skills use this for file resolution.

```markdown
# Maestro Context Index

> Auto-generated by `maestro:setup`. Update this file when adding new context documents.

## Context Files

| File | Purpose |
|------|---------|
| [product.md](product.md) | Product definition -- purpose, users, features |
| [tech-stack.md](tech-stack.md) | Technology stack -- languages, frameworks, tools |
| [guidelines.md](guidelines.md) | Coding guidelines -- principles, conventions, NFRs |
| [product-guidelines.md](product-guidelines.md) | Product guidelines -- voice, tone, UX, branding |
| [workflow.md](workflow.md) | Workflow configuration -- methodology, commits, coverage |
{if style guides: | [code_styleguides/](code_styleguides/) | Code style guides for detected languages |}

## Registry

| File | Purpose |
|------|---------|
| [../tracks.md](../tracks.md) | Tracks registry -- all feature and bug tracks |

## Usage

Skills resolve context by reading this index first, then loading the relevant files.
```

Write to `.maestro/context/index.md`.

Write state: `index_md`

## Step 14: First Track (Optional)

_Skip this step if `last_successful_step` >= `first_track`._

Offer to create the first track now so the project is immediately actionable.

```
AskUserQuestion(
  questions: [{
    question: "Would you like to create the first track now? A track represents a feature, bug fix, or other unit of work.",
    header: "First Track",
    options: [
      { label: "Yes, create a track", description: "I'll describe a feature or task to start" },
      { label: "Skip", description: "I'll create tracks later with /maestro:new-track" }
    ],
    multiSelect: false
  }]
)
```

If **Skip**: continue to Step 15.

If **Yes**:

```
AskUserQuestion(
  questions: [{
    question: "Describe the feature, bug fix, or task for the first track. Be as specific as you like.",
    header: "Track Description",
    options: [
      { label: "Let me describe it", description: "Type a description of the work" }
    ],
    multiSelect: false
  }]
)
```

Use the user's description to generate a track slug (kebab-case, max 5 words). Then create:

```bash
mkdir -p .maestro/tracks/{slug}
```

**`spec.md`** -- requirements and acceptance criteria:
```markdown
# {Track Title}

## Description
{user's description}

## Acceptance Criteria
- [ ] {criterion 1 -- inferred from description}
- [ ] {criterion 2 -- inferred from description}

## Out of Scope
- {anything explicitly excluded}
```

**`plan.md`** -- implementation steps:
```markdown
# Implementation Plan: {Track Title}

## Tasks
- [ ] {task 1}
- [ ] {task 2}

## Verification
- {how to verify this track is complete}
```

**`metadata.json`** -- machine-readable track metadata:
```json
{
  "slug": "{slug}",
  "title": "{Track Title}",
  "status": "new",
  "created": "{ISO 8601 date}",
  "description": "{user's description}"
}
```

**`index.md`** -- track navigation:
```markdown
# Track: {Track Title}

| File | Purpose |
|------|---------|
| [spec.md](spec.md) | Requirements and acceptance criteria |
| [plan.md](plan.md) | Implementation plan and tasks |
| [metadata.json](metadata.json) | Machine-readable track metadata |
```

Write all four files to `.maestro/tracks/{slug}/`.

Register the track in `.maestro/tracks.md`:
```markdown
## [ ] {Track Title}

> Path: `.maestro/tracks/{slug}/`
> Created: {date}

{user's description}
```

Write state: `first_track`

## Step 15: Summary and Commit

Display a summary of all generated files:

```
Setup complete.

Context files:
- .maestro/context/index.md             -- Navigation index (all context files)
- .maestro/context/product.md           -- Product definition
- .maestro/context/tech-stack.md        -- Technology stack
- .maestro/context/guidelines.md        -- Coding guidelines
- .maestro/context/product-guidelines.md -- Product guidelines (voice, UX, branding)
- .maestro/context/workflow.md          -- Task workflow configuration
{if style guides: - .maestro/context/code_styleguides/ -- Code style guides}
- .maestro/tracks.md                    -- Tracks registry
{if first track: - .maestro/tracks/{slug}/           -- First track: {title}}

These files are used by all maestro:* skills.

Next steps:
- /maestro:new-track <description>  -- Create a feature/bug track
- /maestro:implement                -- Start implementing the current track
- /maestro:setup --reset            -- Start over
```

Commit all generated files:
```bash
git add .maestro/context/ .maestro/tracks.md .maestro/setup_state.json
git commit -m "chore(maestro): scaffold project context files"
```

If a first track was created, also stage those files:
```bash
git add .maestro/tracks/
git commit -m "chore(maestro): add first track {slug}"
```

Remove the state file now that setup is complete (clean finish):
```bash
rm -f .maestro/setup_state.json
```
