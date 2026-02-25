---
name: maestro:AGENTS.md
description: "Generates AGENTS.md file using the WHAT/WHY/HOW framework. Explores the codebase and produces a minimal (<100 line) context file with progressive disclosure."
argument-hint: "[--reset]"
---

# AGENTS.md -- Context File Generator

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Generate a minimal, high-impact `AGENTS.md` context file for this repository. The file follows the WHAT/WHY/HOW framework and stays under 100 lines. Task-specific details go into well-named files in `.maestro/context/` as progressive disclosure.

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

## Arguments

`$ARGUMENTS`

- `--reset`: Regenerate everything from scratch -- overwrite AGENTS.md and all context files created by this skill, then re-run the full exploration.
- Default (no args): Generate AGENTS.md and context files. If AGENTS.md already exists, overwrite it directly.

## Research Background

These rules come from evaluations of agent context files (arXiv: 2602.11988, HumanLayer blog):

- Auto-generated context files **reduce** success rates ~3% while increasing cost 20%+. Human-written files improve completion only ~4%. Quality matters more than quantity.
- Codebase overviews and directory listings do not help agents navigate faster.
- Tools mentioned in AGENTS.md get used **160x more often** than unmentioned ones. The HOW section is the highest-leverage category.
- Instruction-following decays with rule count. Every line must earn its place.
- LLMs bias toward instructions at the peripheries of the prompt (beginning and end).
- Never send an LLM to do a linter's job -- use deterministic tools.
- LLMs are in-context learners -- if code follows patterns, agents follow them without being told.

The skill applies these findings: keep the file short, focus on HOW, use pointers not copies, and apply the "helps in EVERY session" universality test to every line.

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

1. **Project identity**: Read `README.md` (first 50 lines), `package.json` / `pyproject.toml` / `Cargo.toml` / `go.mod` / `build.gradle` / `pom.xml` / `Gemfile` / `composer.json` (whichever exists).
2. **Build and test commands**: Read the package manifest scripts section, `Makefile`, `justfile`, `Taskfile.yml`, CI config (`.github/workflows/*.yml`, `.gitlab-ci.yml`), `docker-compose.yml`.
3. **Existing CLAUDE.md**: Read `CLAUDE.md` if it exists -- extract any rules worth preserving.
4. **Existing AGENTS.md**: Read `./AGENTS.md` if it exists -- note what it covers before overwriting.
5. **Tooling**: Detect non-obvious tool choices (bun vs npm, uv vs pip, pnpm vs yarn, custom wrappers).
6. **Linter/formatter configs**: Check for `.eslintrc*`, `prettier*`, `biome.json`, `ruff.toml`, `.rubocop.yml`, `clippy.toml`, `.editorconfig`. If linters enforce style, do NOT duplicate those rules in AGENTS.md.
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

Compose the `AGENTS.md` file using the strict template below. The file MUST be under 100 lines.

### Template

```markdown
# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is
{1-3 sentences: purpose, target users, why it exists}

## Tech Stack
{bullet list: language, framework, key dependencies -- no versions unless critical}

## How to Build, Test, and Run
{exact commands: build, test, lint, dev server -- the highest-leverage section}
{include ALL common commands a developer needs: install deps, run tests, run single test, lint, format, build, dev server}

## Tooling
{non-obvious tool choices only: bun not npm, uv not pip, etc.}
{omit this section entirely if all tooling is standard/obvious}

## Rules
{3-7 behavioral rules that apply to EVERY session}
{example: "Never modify generated files in src/gen/"}
{example: "All API changes require migration script"}
{example: "Run tests before committing"}
{only include rules that are NOT enforced by linters/hooks}

## Reference Docs
{pointers to .maestro/context/*.md files with 1-line descriptions}
{example: - `.maestro/context/building_the_project.md` -- detailed build configuration and environment setup}
```

### Template Rules

Apply these filters to every line:

1. **Universality test**: Does this line help in EVERY session? If not, move it to a progressive disclosure file.
2. **No codebase overviews**: No directory trees, no "the codebase is organized as...".
3. **No code style rules**: If a linter or formatter enforces it, do not repeat it.
4. **No code snippets**: Use `file:line` references to point to authoritative context.
5. **Pointers over copies**: Reference files, don't inline their content.
6. **HOW gets priority**: Build/test/run commands are the most impactful content. Be thorough and precise.
7. **Omit empty sections**: If a section has no content, remove it entirely.

---

## Step 4: Draft Progressive Disclosure Files

Always create well-named files in `.maestro/context/` for task-specific details that did not pass the universality test.

### Always Create (if relevant content exists)

- **`building_the_project.md`** -- detailed build configuration, environment setup, prerequisite installation, build flags, environment variables, dev server configuration
- **`running_tests.md`** -- test commands, running single tests, test file patterns, coverage commands, test environment setup, fixture/mock patterns, CI test configuration

### Create If Discovered

- **`code_conventions.md`** -- style rules and patterns NOT enforced by linters, naming conventions, file organization patterns, import ordering preferences
- **`service_architecture.md`** -- service boundaries, API contracts, inter-service communication, deployment topology (for monorepos / microservices)
- **`database_schema.md`** -- database type, migration tool, schema patterns, seed data, connection configuration
- Other files as warranted by what the exploration discovers -- use descriptive snake_case names

### File Content Rules

- **Pointers over copies**: Use `file:line` references to point to authoritative context (e.g., "See `Makefile:12-25` for build targets"). Do not paste code snippets.
- **Actionable content only**: Commands, references, patterns. No prose overviews.
- **Keep files focused**: Each file covers one topic. Aim for 20-60 lines per file.

---

## Step 5: Present Draft for Approval

Present the full AGENTS.md content AND a summary of planned context files for approval. Embed the entire AGENTS.md draft directly in the question field (same pattern as `maestro:new-track` spec approval):

Ask the user: "Here is the drafted AGENTS.md and planned context files -- does it look correct?\n\n---\n{full AGENTS.md content}\n---\n\nContext files to create:\n{list each .maestro/context/ file with 1-line description}\n---"
Options:
- **Approved** -- Write the files
- **Needs revision** -- I'll tell you what to change

If revision needed: ask what to change, update, and re-present with full updated content. Max 3 revision loops.

---

## Step 6: Write Files

1. Create `.maestro/context/` if it does not exist:
   ```bash
   mkdir -p .maestro/context
   ```

2. Write `./AGENTS.md` (overwrite if exists).

3. Write each progressive disclosure file to `.maestro/context/`.

4. Report what was written:
   ```
   Files written:
   - ./AGENTS.md                                    -- {line count} lines
   - .maestro/context/building_the_project.md       -- {description}
   - .maestro/context/running_tests.md              -- {description}
   {additional files as created}
   ```

---

## Step 7: Commit

```bash
git add ./AGENTS.md .maestro/context/
git commit -m "chore(maestro): generate AGENTS.md with progressive disclosure"
```

Report the commit hash and summary.

---

## Summary Format

After commit, display:

```
AGENTS.md generated.

- ./AGENTS.md ({line_count} lines)
- {N} context files in .maestro/context/

Next steps:
- Review ./AGENTS.md and edit manually for accuracy
- /maestro:AGENTS.md --reset  -- regenerate from scratch
```
