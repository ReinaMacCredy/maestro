# AGENTS.md Template

## Output Template

The generated `AGENTS.md` file MUST be under 100 lines and follow this structure:

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

## Template Rules

Apply these filters to every line:

1. **Universality test**: Does this line help in EVERY session? If not, move it to a progressive disclosure file.
2. **No codebase overviews**: No directory trees, no "the codebase is organized as...".
3. **No code style rules**: If a linter or formatter enforces it, do not repeat it.
4. **No code snippets**: Use `file:line` references to point to authoritative context.
5. **Pointers over copies**: Reference files, don't inline their content.
6. **HOW gets priority**: Build/test/run commands are the most impactful content. Be thorough and precise.
7. **Omit empty sections**: If a section has no content, remove it entirely.

## Research Background

These rules come from evaluations of agent context files (arXiv: 2602.11988, HumanLayer blog):

- Auto-generated context files **reduce** success rates ~3% while increasing cost 20%+. Human-written files improve completion only ~4%. Quality matters more than quantity.
- Codebase overviews and directory listings do not help agents navigate faster.
- Tools mentioned in AGENTS.md get used **160x more often** than unmentioned ones. The HOW section is the highest-leverage category.
- Instruction-following decays with rule count. Every line must earn its place.
- LLMs bias toward instructions at the peripheries of the prompt (beginning and end).
- Never send an LLM to do a linter's job -- use deterministic tools.
- LLMs are in-context learners -- if code follows patterns, agents follow them without being told.

## Progressive Disclosure Files

Create well-named files in `.maestro/context/` for task-specific details that did not pass the universality test.

### Always Create (if relevant content exists)

- **`building_the_project.md`** -- detailed build configuration, environment setup, prerequisite installation, build flags, environment variables, dev server configuration
- **`running_tests.md`** -- test commands, running single tests, test file patterns, coverage commands, test environment setup, fixture/mock patterns, CI test configuration

### Reuse Existing maestro:setup Files

If `.maestro/context/guidelines.md` or `.maestro/context/product-guidelines.md` exist (from maestro:setup), reference them in the ## Reference Docs section -- do not create duplicates.

### Create If Discovered

- **`code_conventions.md`** -- style rules and patterns NOT enforced by linters, naming conventions, file organization patterns (skip if `guidelines.md` already covers this)
- **`service_architecture.md`** -- service boundaries, API contracts, inter-service communication, deployment topology (for monorepos / microservices)
- **`database_schema.md`** -- database type, migration tool, schema patterns, seed data, connection configuration
- Other files as warranted by what the exploration discovers -- use descriptive snake_case names

### File Content Rules

- **Pointers over copies**: Use `file:line` references to point to authoritative context (e.g., "See `Makefile:12-25` for build targets"). Do not paste code snippets.
- **Actionable content only**: Commands, references, patterns. No prose overviews.
- **Keep files focused**: Each file covers one topic. Aim for 20-60 lines per file.
