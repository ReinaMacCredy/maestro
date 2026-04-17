# WORKFLOW.md Customization Guide

The WORKFLOW.md file is the central configuration for Symphony. It tells the agent:

- What project it is working on
- How to find and claim issues
- How to build, test, and validate changes
- What rules and conventions to follow
- How to transition issues through the workflow

This guide explains each section of the WORKFLOW.md template and how to customize it for your project.

## File Structure

WORKFLOW.md has four major sections:

1. **YAML front matter** -- Machine-readable metadata (project slug, repo URL)
2. **Project context** -- Human-readable project description, build commands, rules
3. **Prerequisite** -- Linear connectivity requirements
4. **Ticket state machine** -- Step-by-step workflow the agent follows for each issue

## Section-by-Section Reference

### YAML Front Matter

```yaml
---
project_slug: my-project-abc123
repo_clone_url: https://github.com/org/repo.git
---
```

| Field | Required | Description |
|-------|----------|-------------|
| `project_slug` | Yes | Linear project slug -- used to filter issues to this project |
| `repo_clone_url` | Yes | Git clone URL -- used by Symphony to clone/checkout the repository |

These are the only machine-parsed fields. Everything else is read by the agent as natural language instructions.

### Project Context

```markdown
## Project context: My Project

My Project is a web application that does X, Y, and Z. It serves
developers who need to accomplish A and B.

Build and validation commands:
- `bun install` -- Install dependencies
- `bun run build` -- Build the project
- `bun test` -- Run tests
- `bun run lint` -- Run linter

Rules:
- Use TypeScript strict mode
- No `any` types
- All public functions must have JSDoc comments
- Tests required for all new features
```

#### Project description

Write 2-3 sentences that give the agent enough context to understand what the project does and who uses it. The agent uses this to:

- Write appropriate commit messages and PR descriptions
- Make reasonable assumptions when issue descriptions are ambiguous
- Understand the relative importance of different parts of the codebase

**Good description**: "Acme API is a REST service that powers the Acme mobile app. It handles user authentication, order management, and payment processing. The primary users are the mobile team who consume the API and the ops team who monitor it."

**Bad description**: "A web app." (Too vague -- the agent will make poor assumptions.)

#### Build and validation commands

List every command the agent needs to run to validate its changes. The agent runs these after making code changes and before committing.

**Format**: Markdown list with command and description.

**Recommended command set by tech stack**:

| Tech Stack | Commands |
|-----------|----------|
| TypeScript/Bun | `bun install`, `bun run build`, `bun test`, `bun run lint` |
| TypeScript/Node | `npm install`, `npm run build`, `npm test`, `npm run lint` |
| Rust | `cargo build`, `cargo test`, `cargo clippy` |
| Python | `uv sync`, `uv run pytest`, `uv run ruff check` |
| Go | `go build ./...`, `go test ./...`, `golangci-lint run` |
| Elixir | `mix deps.get`, `mix compile`, `mix test`, `mix credo` |
| Java/Gradle | `./gradlew build`, `./gradlew test`, `./gradlew check` |
| Java/Maven | `./mvnw compile`, `./mvnw test`, `./mvnw verify` |

**Tips**:
- Include dependency installation commands (the agent may start from a clean state)
- Include type checking if your project uses it (`tsc --noEmit`, `mypy`, etc.)
- Order commands from fastest to slowest (fail fast on simple errors)
- Include format checking if enforced (`prettier --check`, `black --check`, etc.)

#### Project rules

List the coding conventions and rules the agent must follow. These override the agent's default behavior.

**Format**: Markdown list, one rule per item.

**Good rules** (specific, actionable):
- "Use `bun` for all package operations, never `npm` or `yarn`"
- "All API endpoints must validate input with Zod schemas"
- "Error messages must be user-facing -- no stack traces in API responses"
- "No new dependencies without explicit approval in the issue"

**Bad rules** (vague, unenforceable):
- "Write clean code" (subjective)
- "Follow best practices" (undefined)
- "Be careful with security" (not actionable)

**Rules to consider adding**:
- Package manager preference
- Test framework and patterns
- Import ordering conventions
- Error handling patterns
- Logging conventions
- Security requirements (input validation, output sanitization)
- Performance constraints
- Accessibility requirements

### Prerequisite Section

```markdown
## Prerequisite: Linear MCP or `linear_graphql` tool is available

The agent should be able to talk to Linear, either via a configured
Linear MCP server or injected `linear_graphql` tool. If none are
present, stop and ask the user to configure Linear.
```

This section tells the agent to verify Linear connectivity before starting work. Do not modify this section unless you have a custom Linear integration.

### Ticket State Machine

The state machine is the core of the WORKFLOW.md. It defines the step-by-step process the agent follows for each Linear issue.

See `linear-setup.md` for the status definitions and transitions.

#### Step 0: Route by ticket state

The agent reads the current issue status and routes to the appropriate step:

| Current status | Route to |
|---------------|----------|
| Todo | Step 1 (start execution) |
| In Progress | Step 2 (continue execution) |
| Human Review | Step 3 (handle review) |
| Rework | Step 4 (address feedback) |

This routing allows the agent to resume work on an issue that was interrupted or is in a specific state.

#### Step 1: Start execution

The agent claims the issue by moving it to "In Progress", checks out a feature branch, and begins working.

**Customization points**:
- Branch naming convention (default: issue identifier as branch name)
- Pre-work setup commands (e.g., database migrations, env file generation)
- Issue analysis depth (how much context to gather before starting)

#### Step 2: Execution phase

The agent implements the changes described in the issue. This is the main work phase.

**Customization points**:
- Implementation strategy (TDD, ship-fast, etc.)
- Commit frequency (atomic commits vs single commit)
- Test requirements (unit, integration, e2e)
- Self-review checklist items
- Build validation commands (referenced from project context)

#### Step 3: Human Review handling

After pushing a PR, the agent monitors for review feedback and CI results.

**Customization points**:
- Review response time expectations
- Auto-merge on approval (enabled by default)
- Required number of approvals
- CI check requirements (which checks must pass)

#### Step 4: Rework handling

When a reviewer requests changes, the agent addresses the feedback.

**Customization points**:
- Feedback categorization (required changes vs suggestions)
- Conflict resolution with existing code
- Re-review triggers (when to re-request review)

## Multi-Repository Considerations

If your organization has multiple repositories using Symphony:

### Shared configuration

- Keep project-specific rules in each repo's WORKFLOW.md
- Share common rules via a template or include mechanism
- Use consistent status names across all projects (the same three custom statuses)

### Cross-repo dependencies

Symphony operates on one repository at a time. For cross-repo changes:

1. Create separate Linear issues for each repository
2. Link the issues in Linear (blocking/blocked-by)
3. Symphony will work on each issue independently
4. Coordinate merge order manually or via issue dependencies

### Monorepo considerations

For monorepos:

- Set build/test commands to scope to the affected package/module
- Add rules about which directories map to which teams/components
- Consider adding path-scoped validation (e.g., "if changing `packages/api/`, run `bun test --filter api`")

## Template Variables Reference

These placeholders are replaced during `maestro:symphony-setup`:

| Placeholder | Description | Example |
|---|---|---|
| `{{PROJECT_SLUG}}` | Linear project slug | `my-project-abc123` |
| `{{REPO_CLONE_URL}}` | Git clone URL | `https://github.com/org/repo.git` |
| `{{PROJECT_NAME}}` | Human-readable project name | `My Project` |
| `{{PROJECT_DESCRIPTION}}` | 2-3 sentence project description | See "Project description" above |
| `{{BUILD_AND_TEST_COMMANDS}}` | Build/test/lint commands as markdown list | See "Build and validation commands" above |
| `{{PROJECT_RULES}}` | Coding rules as markdown list | See "Project rules" above |

After setup, these placeholders no longer exist -- they are replaced with actual content. Edit WORKFLOW.md directly for future changes.
