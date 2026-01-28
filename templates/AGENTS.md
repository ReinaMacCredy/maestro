# templates

## Purpose
Project scaffolding and configuration templates for new Maestro-enabled projects.

## Key Directories

| Directory | Purpose |
|-----------|---------|
| claude-code-setup/ | Core configuration scaffolding for new projects |
| code_styleguides/ | Language-specific coding standards |

## Key Files

| File | Purpose |
|------|---------|
| claude-code-setup/CLAUDE.md | Template for project context (architecture, tech stack) |
| claude-code-setup/AGENTS.md | Template for agent workflow rules |
| claude-code-setup/SETUP.md | Environment initialization instructions |
| workflow.md | Authoritative development lifecycle definition |

## Code Styleguides

| File | Language/Topic |
|------|----------------|
| general.md | Universal principles (readability, simplicity) |
| go.md | Go-specific conventions |
| python.md | Python-specific conventions |
| javascript.md | JavaScript-specific conventions |
| typescript.md | TypeScript-specific conventions |
| html-css.md | HTML/CSS conventions |

## Patterns

- **Configuration Layers**: CLAUDE.md (memory), AGENTS.md (workflow), .claude/rules/ (constraints), skills/ (capabilities)
- **TDD Micro-Loop**: Red (fail) -> Green (pass) -> Refactor
- **Plan-Driven**: All work tracks against plan.md

## Dependencies

- **Internal**: Used by /conductor-setup to initialize new projects
- **External**: None (pure documentation/templates)

## Notes for AI Agents

- Copy relevant styleguide to project's AGENTS.md "Code Quality" section
- workflow.md defines the strict TDD protocol - follow exactly
- Templates establish the "truth hierarchy" for project configuration
- Customize templates per-project, don't modify originals
