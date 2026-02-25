---
name: init
description: "Generates AGENTS.md file using the WHAT/WHY/HOW framework. Explores the codebase and produces a minimal (<100 line) context file with progressive disclosure."
argument-hint: "[--reset]"
---

# AGENTS.md -- Context File Generator

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Generate a minimal, high-impact `AGENTS.md` context file for this repository using the WHAT/WHY/HOW framework. See `reference/AGENTS.md` for the template, rules, and progressive disclosure specs.

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

## Arguments

`$ARGUMENTS`

- `--reset`: Regenerate everything from scratch -- overwrite AGENTS.md and all context files created by this skill, then re-run the full exploration.
- Default (no args): Generate AGENTS.md and context files. If AGENTS.md already exists, overwrite it directly.

---

## Step 1: Handle --reset

If `$ARGUMENTS` contains `--reset`:

1. Check which `.maestro/context/` files were created by this skill (not by `maestro:setup`). The skill-created files use snake_case names like `building_the_project.md`, `running_tests.md`, `code_conventions.md`, `service_architecture.md`, `database_schema.md`, etc. The `maestro:setup` files use kebab-case: `product.md`, `tech-stack.md`, `guidelines.md`, `product-guidelines.md`, `workflow.md`, `index.md`.
2. Delete the skill-created context files (preserve `maestro:setup` files).
3. Delete `./AGENTS.md` if it exists.
4. Report what was deleted.
5. Continue to Step 2 to regenerate everything from scratch.

---

## Step 2: Explore the Codebase

Read-only exploration. Do NOT ask the user for permission to explore -- just do it.

### 2a: Check for Maestro Context (pre-fill)

Search for `.maestro/context/product.md`.

If it exists, `maestro:setup` has been run. Read these files for pre-fill data:
- `.maestro/context/product.md` -- purpose, users, features
- `.maestro/context/tech-stack.md` -- languages, frameworks, tools
- `.maestro/context/guidelines.md` -- coding conventions
- `.maestro/context/workflow.md` -- build/test methodology

Store findings as pre-fill. Do NOT ask questions the context already answers.

### 2b: Explore the Codebase

Regardless of whether maestro context exists, explore the codebase to discover or verify:

1. **Project identity**: Read `README.md`, `package.json` / `pyproject.toml` / `Cargo.toml` / `go.mod` / `build.gradle` / `pom.xml` / `Gemfile` / `composer.json` (whichever exists).
2. **Build and test commands**: Read the package manifest scripts section, `Makefile`, `justfile`, `Taskfile.yml`, CI config (`.github/workflows/*.yml`, `.gitlab-ci.yml`), `docker-compose.yml`.
3. **Existing CLAUDE.md**: Read `CLAUDE.md` if it exists -- extract any rules worth preserving.
4. **Existing AGENTS.md**: Read `./AGENTS.md` if it exists -- note what it covers before overwriting.
5. **Tooling**: Detect non-obvious tool choices (bun vs npm, uv vs pip, pnpm vs yarn, custom wrappers).
6. **Linter/formatter configs**: Check for `.eslintrc*`, `prettier*`, `biome.json`, `ruff.toml`, `.rubocop.yml`, `clippy.toml`, `.editorconfig`. Note what they enforce (used by template rules to avoid duplication).
7. **Architecture signals**: Monorepo structure (`packages/`, `apps/`, `crates/`, `services/`), database configs, API patterns.

The agent decides what to read based on what it finds. This is exploration, not a rigid checklist -- adapt to the project.

### 2c: Synthesize Findings

Organize discoveries into these categories (internal notes, not output):
- **WHAT**: Project purpose, tech stack, key dependencies
- **WHY**: Why the project exists, who it's for
- **HOW**: Build commands, test commands, dev server, lint commands, non-obvious tooling
- **RULES**: Behavioral rules that apply to every session
- **TASK-SPECIFIC**: Details that belong in progressive disclosure files (test patterns, architecture details, database schema, etc.)

---

## Step 3: Draft AGENTS.md

Use the template and rules from `reference/AGENTS.md`. The output file MUST be under 100 lines.

---

## Step 4: Draft Progressive Disclosure Files

Use the progressive disclosure guidance from `reference/AGENTS.md` to create well-named files in `.maestro/context/`.

---

## Step 5: Write Files

1. Create `.maestro/context/` if it does not exist:
   ```bash
   mkdir -p .maestro/context
   ```

2. Write `./AGENTS.md` (overwrite if exists).

3. Write each progressive disclosure file to `.maestro/context/`.

4. Display summary:
   ```
   AGENTS.md generated.

   - ./AGENTS.md ({line_count} lines)
   - .maestro/context/building_the_project.md
   - .maestro/context/running_tests.md
   {additional files as created}

   Next steps:
   - Review ./AGENTS.md and edit manually for accuracy
   - /maestro:AGENTS.md --reset  -- regenerate from scratch
   ```

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:AGENTS.md` -- **You are here.** Generate AGENTS.md context file
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks

AGENTS.md is an optional context enhancement that complements `/maestro:setup`. While setup creates project-level context files, this skill generates a codebase-oriented AGENTS.md with progressive disclosure. Run it after setup to give all agents richer context about the repository structure.
