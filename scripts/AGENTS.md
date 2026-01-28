# scripts

## Purpose
Environment setup, validation, and session lifecycle management scripts for the Maestro workflow.

## Key Files

| File | Purpose |
|------|---------|
| amp-session.sh | Amp agent session wrapper with auto-restart on exit code 42 |
| beads-metrics-summary.sh | Weekly reports on TDD cycles and task completion |
| install-global-hooks.sh | Compiles and installs TypeScript hooks to ~/.claude/hooks |
| install-codex.sh | Installs Maestro skills in Codex environment |
| validate-anchors.sh | CI utility verifying Markdown internal links |
| validate-links.sh | CI utility verifying Markdown file links exist |
| test-hooks.sh | Smoke tests for continuity hooks system |

## Key Directories

| Directory | Purpose |
|-----------|---------|
| atlas/ | (Reserved for Atlas automation - currently empty) |

## Patterns

- **Exit Code 42**: Special exit code triggers session restart for handoff workflow
- **Global Hooks**: TypeScript hooks compiled and installed to user's Claude config
- **CI Validation**: Link validation scripts run in CI to catch broken references

## Dependencies

- **External**: Bash, Node.js (for hook compilation)
- **Internal**: Hooks depend on .claude/hooks/ structure

## Notes for AI Agents

- amp-session.sh enables seamless context reload via /conductor-handoff
- Run validate-*.sh scripts before committing documentation changes
- The atlas/ directory is reserved for future Python tooling
- test-hooks.sh verifies ledger creation and handoff generation
