# Codex Skills Guide

## What Are Codex Skills?

Codex skills are markdown instruction files that the OpenAI Codex CLI loads to guide agent behavior during specific operations. They live in `.codex/skills/<name>/SKILL.md` and are automatically discovered and available to the agent.

Skills are distinct from system prompts or CODEX.md configuration. They are:

- **Task-specific**: each skill covers one operation (committing, pushing, landing PRs)
- **Invocable**: the agent or user triggers a skill when the matching operation is needed
- **Self-contained**: each skill has its own directory with all required files
- **Portable**: skills copy between repositories without modification (project-specific details live in WORKFLOW.md)

### Codex Skills vs Maestro Skills

| Aspect | Codex Skills | Maestro Skills |
|--------|-------------|----------------|
| Runtime | Codex CLI (OpenAI) | Claude Code (Anthropic) |
| Location | `.codex/skills/<name>/SKILL.md` | Loaded via `maestro skill <name>` |
| Activation | During Symphony sessions or manual Codex runs | On-demand via skill command |
| Scope | Single operation (commit, push, land) | Workflow phase (design, implement, review) |
| Project config | WORKFLOW.md | `.maestro/context/` files |

## The Symphony Skill Set

Symphony bundles six skills that form a complete autonomous development pipeline. Each skill handles one phase of the issue-to-merge workflow.

### Skill Dependency Graph

```
                    +---------+
                    | linear  |  <-- Queries/updates Linear issues
                    +----+----+
                         |
                         | (reads issue, updates status)
                         v
    +--------+     +-----+------+     +--------+
    |  pull   |    |   commit   |     | debug  |
    +----+----+    +-----+------+     +--------+
         |               |             (diagnostic,
         | (resolves      | (creates    no deps)
         |  conflicts)    |  commits)
         v               v
    +---------+     +-----+------+
    |  (main) |     |    push    |
    +---------+     +-----+------+
                          |
                          | (creates/updates PR)
                          v
                    +-----+------+
                    |    land    |
                    +-----+------+
                          |
                          | (monitors CI, merges)
                          v
                    +-----+------+
                    |  (merged)  |
                    +------------+
```

### Per-Skill Reference

#### `commit` -- Create Well-Formed Commits

**Purpose**: Stage changes and create a git commit with a well-structured message derived from the session context (what the agent did and why).

**When it activates**: After the agent has made code changes and needs to persist them.

**What it produces**: A single git commit with:
- Conventional commit type prefix (`feat:`, `fix:`, `refactor:`, etc.)
- Concise summary line (under 72 characters)
- Body with rationale and change summary

**Key behaviors**:
- Uses session history to generate the commit message (not just the diff)
- Stages only relevant files (not unrelated changes)
- Validates the commit with pre-commit hooks if configured
- Groups related changes into a single atomic commit

**Configuration**: No project-specific configuration needed. The skill reads WORKFLOW.md for commit conventions if present.

---

#### `debug` -- Diagnose Stuck Runs

**Purpose**: Troubleshoot Symphony/Codex operational issues by reading logs, correlating events, and identifying root causes.

**When it activates**: When a Symphony run is stuck, producing errors, or behaving unexpectedly.

**What it produces**: Diagnostic report with root cause and suggested fix.

**Key behaviors**:
- Reads Codex session logs from `~/.codex/sessions/`
- Correlates events across Symphony logs, Codex logs, and Linear activity
- Identifies common failure patterns (API timeouts, auth failures, merge conflicts)
- Provides actionable fix recommendations

**Log sources**:
- `~/.codex/sessions/` -- Codex agent session logs
- Symphony terminal output -- orchestrator-level events
- Linear activity -- issue status transitions, comments
- GitHub -- PR status, CI results, review comments

**Correlation keys**: Session ID, issue identifier, branch name, PR number. Use these to trace an event across all log sources.

---

#### `land` -- Monitor and Merge PRs

**Purpose**: Shepherd a PR from creation to merge. Monitors CI status, handles review feedback, resolves merge conflicts, and squash-merges when all checks pass.

**When it activates**: After `push` creates or updates a PR.

**What it produces**: A merged PR (squash-merge into the primary branch).

**Key behaviors**:
- Uses `land_watch.py` for async monitoring of CI, reviews, and head changes
- Handles review feedback by reading comments, making changes, and pushing fixes
- Resolves merge conflicts by pulling main and resolving
- Squash-merges with a clean commit message when all checks are green
- Updates Linear issue status to `Merging` during merge, then closes the issue

**Dependencies**:
- `push` (the PR must exist)
- `linear` (for status updates)
- `python3` and `gh` CLI (for `land_watch.py`)

**Exit codes from `land_watch.py`**:
| Code | Meaning | Action |
|------|---------|--------|
| 0 | All clear -- CI green, no comments | Proceed to merge |
| 2 | Review comments detected | Address feedback, push fixes |
| 3 | CI checks failed | Read failure logs, fix, push |
| 4 | PR head updated | Pull latest, rebase if needed |
| 5 | Merge conflicts | Pull main, resolve conflicts |

---

#### `linear` -- Linear GraphQL Operations

**Purpose**: Query and update Linear issues using raw GraphQL through Symphony's `linear_graphql` tool.

**When it activates**: When the agent needs to read issue details, update status, post comments, or attach PRs to issues.

**What it produces**: Linear API responses (issue data, status updates, comments).

**Key behaviors**:
- Uses Symphony's `linear_graphql` client tool (not direct API calls)
- Reuses Symphony's configured Linear auth for the session
- Sends one GraphQL operation per tool call
- Treats top-level `errors` array as failure even if the tool call succeeded

**Important**: This skill only works during active Symphony sessions. The `linear_graphql` tool is injected by Symphony's app-server. Outside of Symphony, use the Linear MCP server or web UI.

**Common operations**:
- Query issue by key/identifier
- Move issue to a different status
- Create/edit comments on issues
- Attach GitHub PRs to issues
- Query team workflow states

---

#### `pull` -- Pull and Resolve Conflicts

**Purpose**: Pull upstream changes from the primary branch and resolve any merge conflicts.

**When it activates**: Before starting work (to ensure a clean base) or when `land_watch.py` reports merge conflicts.

**What it produces**: A clean working tree with upstream changes integrated.

**Key behaviors**:
- Fetches and merges from the primary branch
- Attempts automatic conflict resolution
- For code conflicts: analyzes both sides and chooses the correct resolution
- For ambiguous conflicts: asks the user (minimizes human interruption)
- After resolution: validates build/test still pass

**Conflict resolution priority**:
1. Automated resolution for trivial conflicts (whitespace, import ordering)
2. Semantic resolution for code conflicts (understanding intent of both sides)
3. User escalation only when both sides have substantive, incompatible changes

---

#### `push` -- Push and Create PRs

**Purpose**: Push committed changes to the remote and create or update a pull request.

**When it activates**: After `commit` has created one or more commits ready to push.

**What it produces**: A pushed branch with an open PR (or updated existing PR).

**Key behaviors**:
- Creates a feature branch if not already on one
- Pushes commits to the remote
- Creates a new PR or updates the existing PR description
- Sets PR title from the commit message or issue title
- Adds Linear issue link to PR description
- Requests reviewers if configured

**Dependencies**:
- `commit` (commits must exist to push)
- `gh` CLI (for PR creation)

## Customization Patterns

### Adding project-specific skills

Create a new directory in `.codex/skills/` with a `SKILL.md`:

```
.codex/skills/my-custom-skill/
  SKILL.md
  helper-script.py  (optional)
```

The SKILL.md follows the same format as the bundled skills. Include:
- YAML front matter with `name` and `description`
- Clear steps with commands
- Input/output documentation

### Modifying existing skills

To customize a bundled skill for your project:

1. Edit the SKILL.md in `.codex/skills/<name>/SKILL.md`
2. Add project-specific commands, conventions, or restrictions
3. Keep the core structure intact -- the skill name and overall flow should remain recognizable

Common customizations:
- **commit**: Add project-specific commit message conventions or scopes
- **push**: Add custom PR template path or reviewer assignments
- **land**: Adjust CI check expectations or merge strategy (squash vs rebase)
- **pull**: Add post-pull build/test validation commands

### Skill loading order

Codex loads skills from `.codex/skills/` at session start. Skills are available throughout the session. The agent selects the appropriate skill based on the current operation.

WORKFLOW.md takes precedence over skill defaults. If WORKFLOW.md specifies a build command, it overrides any default in the skill files.
