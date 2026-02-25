---
name: maestro:setup
description: "Scaffolds project context (product, tech stack, coding guidelines, product guidelines, workflow) and initializes track registry. Use for first-time project onboarding."
argument-hint: "[--reset]"
---

# Maestro Setup -- Project Context Scaffolding

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Interview the user to create persistent project context documents. These files are referenced by all `maestro:*` skills for deeper project understanding.

## Arguments

`$ARGUMENTS`

- `--reset`: Delete all existing context files and start fresh.
- Default (no args): Run setup interview. If context already exists, offer to update or skip.

---

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

---

## Step 1: Handle --reset

If `$ARGUMENTS` contains `--reset`:

1. Check if `.maestro/context/` exists
2. If it does, confirm with the user:

   Ask the user: "This will delete all project context files in .maestro/context/, the tracks registry, and the setup state file. Are you sure?"
   Options:
   - **Yes, reset everything** -- Delete context files, tracks.md, and setup_state.json; start fresh
   - **Cancel** -- Keep existing context

3. If confirmed: run `rm -rf .maestro/context/ .maestro/tracks.md .maestro/setup_state.json` and report "Context reset. Run `/maestro:setup` to create new context."
4. Stop.

## Step 2: Check Setup State (Resume Protocol)

Check for an existing `.maestro/setup_state.json`:

```bash
cat .maestro/setup_state.json 2>/dev/null || echo "{}"
```

If the file exists and contains a `last_successful_step` value, the previous run was interrupted. Ask:

Ask the user: "A previous setup run was interrupted after step \"{last_successful_step}\". What would you like to do?\n\nCompleted steps will be skipped automatically."
Options:
- **Resume from where I left off** -- Skip already-completed steps
- **Start over** -- Ignore previous progress and run all steps

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

Search for files matching `.maestro/context/*.md`.

If context files already exist, ask:

Ask the user: "Project context already exists. What would you like to do?"
Options:
- **Update** -- Re-run setup and overwrite existing files
- **View** -- Show current context files and exit
- **Cancel** -- Keep existing context unchanged

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

   Ask the user: "There are uncommitted changes in this repository. How would you like to proceed?"
   Options:
   - **Continue anyway** -- Proceed with the scan; changes are noted
   - **Abort** -- Stop setup so I can commit or stash first

   If "Abort": stop.

2. Ask explicit scan permission:

   Ask the user: "May I perform a read-only scan of your codebase to infer project context? No files will be modified."
   Options:
   - **Yes, scan the codebase** -- Read-only analysis to pre-fill answers
   - **No, I'll answer manually** -- Skip the scan; ask all questions without pre-fill

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

   Ask the user: "No git repository found. Initialize one now?"
   Options:
   - **Yes, run git init** -- Initialize a new git repository in this directory
   - **Skip** -- Continue without git

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

Ask the user: "How would you like to provide the product definition?"
Options:
- **Interactive** -- Answer questions step by step
- **Autogenerate** -- I'll infer everything from the codebase analysis

If **Autogenerate**: use inferences from Step 4 to populate all fields and skip the detailed questions below. Go directly to writing `product.md`.

If **Interactive**: ask the following questions sequentially (one at a time). Limit to 3 questions max.

**Question 1** -- Project purpose:

Ask the user: "What does this project do? (one sentence)"
Options:
- **{inferred purpose}** -- Based on README/package analysis
- **Let me describe it** -- Type your own description

For greenfield or when no inference is available, provide only a single "Let me describe it" option.

**Question 2** -- Target users:

Ask the user: "Who are the primary users?"
Options:
- **Developers** -- Library, CLI tool, or developer-facing API
- **End users** -- Web app, mobile app, or consumer-facing product
- **Internal team** -- Internal tool, admin dashboard, or ops tooling

**Question 3** -- Key features (skip if brownfield with clear README):

Ask the user: "What are the 2-3 most important features or capabilities?"
Options:
- **Auto-generate from analysis** -- I'll infer from the codebase
- **Let me list them** -- Type your own list

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

Ask the user: "How would you like to provide the tech stack?"
Options:
- **Interactive** -- Review and confirm the detected stack or enter it manually
- **Autogenerate** -- I'll infer the full tech stack from config files

If **Autogenerate**: use inferences from Step 4. Skip detailed questions and go directly to writing `tech-stack.md`.

If **Interactive**:

**For Brownfield**: Present the inferred stack for confirmation:

Ask the user: "Is this your tech stack?\n\n{inferred stack summary}"
Options:
- **Yes, correct** -- Use the detected tech stack
- **Needs changes** -- Let me correct or add to it

**For Greenfield**: Ask directly:

Ask the user: "What tech stack will this project use? (languages, frameworks, database, etc.)"

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

Ask the user: "How would you like to define coding guidelines?"
Options:
- **Interactive** -- Select from common principles and conventions
- **Autogenerate** -- I'll infer from CLAUDE.md, linter configs, and conventions

If **Autogenerate**: infer from `CLAUDE.md`, linter/formatter configs found in Step 4, and project conventions. Skip the question below and write the file.

If **Interactive**:

Ask the user: "Any specific coding guidelines or principles for this project?" (select all that apply)
Options:
- **TDD-first** -- Test-driven development, high coverage
- **Move fast** -- Ship quickly, iterate later
- **Security-first** -- Input validation, audit logging, secure defaults
- **Accessibility-first** -- WCAG compliance, semantic HTML, screen reader support
- **Let me describe** -- Type custom guidelines

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

Ask the user: "How would you like to define product guidelines (voice, tone, UX principles, branding)?"
Options:
- **Interactive** -- Answer questions about brand voice and UX principles
- **Autogenerate** -- I'll generate sensible defaults based on the product type
- **Skip** -- No product guidelines needed for this project

If **Skip**: write a minimal placeholder file and continue.

If **Autogenerate**: generate defaults appropriate for the product type (e.g., developer tool voice vs. consumer app voice). Skip detailed questions and write the file.

If **Interactive**:

**Question 1** -- Voice and tone:

Ask the user: "What is the voice and tone for written content (UI copy, docs, error messages)?"
Options:
- **Professional and direct** -- Clear, concise, no fluff. Suitable for developer tools.
- **Friendly and approachable** -- Warm, conversational. Suitable for consumer apps.
- **Formal and authoritative** -- Precise, structured. Suitable for enterprise/compliance.
- **Playful and energetic** -- Fun, engaging. Suitable for consumer/gaming.
- **Let me describe** -- Type custom voice/tone guidelines

**Question 2** -- UX principles:

Ask the user: "What are the core UX principles?" (select all that apply)
Options:
- **Progressive disclosure** -- Show only what's needed; reveal complexity on demand
- **Zero-config defaults** -- Work out of the box; power users can customize
- **Accessible by default** -- WCAG AA minimum; keyboard navigable; screen reader support
- **Mobile-first** -- Design for small screens first, scale up
- **Let me describe** -- Type custom UX principles

**Question 3** -- Branding (skip if no UI):

Ask the user: "Any branding or visual identity constraints? (color palette, typography, logo usage)"
Options:
- **No branding constraints** -- Skip; no visual identity rules
- **Let me describe** -- Type branding guidelines or link to a style guide

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

Ask the user: "How would you like to configure the workflow?"
Options:
- **Interactive** -- Answer questions about methodology and commit strategy
- **Autogenerate** -- Use recommended defaults (TDD, 80% coverage, per-task commits)

If **Autogenerate**: use TDD, 80% coverage, per-task commits, git notes for summaries. Skip detailed questions and write the file.

If **Interactive**:

**Question 1** -- Methodology:

Ask the user: "What development methodology should tasks follow?"
Options:
- **TDD (Recommended)** -- Write failing tests first, then implement. Red-Green-Refactor.
- **Ship-fast** -- Implement first, add tests after. Faster but less rigorous.
- **Custom** -- Define your own workflow

**Question 2** -- Coverage target:

Ask the user: "What test coverage target for new code?"
Options:
- **80% (Recommended)** -- Good balance of coverage and velocity
- **90%** -- High coverage, slower velocity
- **60%** -- Basic coverage, maximum velocity
- **No target** -- Don't enforce coverage thresholds

**Question 3** -- Commit frequency:

Ask the user: "How often should implementation commit?"
Options:
- **Per-task (Recommended)** -- Atomic commit after each task completes. Fine-grained history.
- **Per-phase** -- Commit after each phase completes. Fewer, larger commits.

**Question 4** -- Summary storage:

Ask the user: "Where should task summaries be stored?"
Options:
- **Git notes (Recommended)** -- Attach detailed summaries as git notes on commits
- **Commit messages** -- Include full summary in the commit message body
- **Neither** -- No additional summaries beyond standard commit messages

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

Ask the user: "Copy code style guides to your project? (based on detected stack: {languages})\n\nAvailable guides: python, typescript, javascript, go, general, cpp, csharp, dart, html-css"
Options:
- **Yes, copy relevant guides** -- Copy style guides for detected languages to .maestro/context/code_styleguides/
- **Yes, copy all guides** -- Copy all 9 style guides
- **Skip** -- No code style guides needed

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

Ask the user: "Would you like to create the first track now? A track represents a feature, bug fix, or other unit of work."
Options:
- **Yes, create a track** -- I'll describe a feature or task to start
- **Skip** -- I'll create tracks later with /maestro:new-track

If **Skip**: continue to Step 15.

If **Yes**:

Ask the user: "Describe the feature, bug fix, or task for the first track. Be as specific as you like."

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
