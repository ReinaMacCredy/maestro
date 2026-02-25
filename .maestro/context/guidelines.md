# Coding Guidelines

## Development Principles
- Plan-first: no implementation without a spec and plan
- TDD-driven: write failing tests first, then implement
- Security-first: validate inputs, sanitize outputs, never log secrets
- Correctness over cleverness: smallest change that works

## Conventions
- `skills/` is the canonical source for skill definitions
- Plugin version sync: `.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json` must match
- Conventional commits: `{type}({scope}): {description}`
- Agents are lean identity + constraints, not workflow duplicates
- Commands (`.claude/commands/`) contain full workflow steps

## Non-Functional Requirements
- CLI-agnostic: skills must work across Claude Code, Amp, and other runtimes
- No new dependencies unless existing stack cannot solve the problem
- Atomic commits: don't mix formatting with behavior changes
- Shell scripts must pass `bash -n` syntax validation
