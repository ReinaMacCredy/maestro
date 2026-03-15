name: maestro-symphony-setup
description: "Set up Symphony orchestration for any repository. Copies Codex skills, generates a customized WORKFLOW.md, and guides Linear status configuration. Works with any tech stack."
argument-hint: "<linear-project-slug> <repo-clone-url>"

# Symphony Setup -- Automated Project Onboarding

Set up OpenAI Symphony orchestration for the current repository so that Codex agents can autonomously pick up Linear issues, implement them, create PRs, and land them.

## Arguments

`$ARGUMENTS`

Two required arguments:
1. **Linear project slug** -- from the project URL (e.g., `my-project-abc123`). Right-click the project in Linear, copy URL, extract the slug after `/project/`.
2. **Repo clone URL** -- the git clone URL for this repository (e.g., `https://github.com/org/repo.git`).

If arguments are missing, check `git remote get-url origin` for the repo URL and ask the user for the Linear project slug.

## Prerequisites

- `LINEAR_API_KEY` environment variable must be set (Linear personal API token).
- `codex` CLI must be installed.
- Symphony Elixir service must be available (typically at `~/Code/symphony/`).

## Step 1: Gather Project Context

Read the project's existing configuration to populate the WORKFLOW.md template:

1. Read `CLAUDE.md`, `AGENTS.md`, `README.md`, or `CODEX.md` (whichever exist) to understand:
   - Project name and description
   - Tech stack
   - Build/test/lint commands
   - Coding conventions and rules
2. Run `git remote get-url origin` to confirm the repo URL.
3. Note the primary branch name (`main` or `master`).

## Step 2: Copy Codex Skills

Copy the Symphony skill templates into `.codex/skills/` in the target repository. The templates are bundled at `skills/maestro:symphony-setup/reference/codex-skills/`.

Create these directories and copy the files:

```
.codex/skills/
  commit/SKILL.md
  debug/SKILL.md
  land/SKILL.md
  land/land_watch.py
  linear/SKILL.md
  pull/SKILL.md
  push/SKILL.md
```

If `.codex/skills/` already exists, do not overwrite existing files -- only add missing ones. Warn the user about any conflicts.

## Step 3: Generate WORKFLOW.md

Use the template at `skills/maestro:symphony-setup/reference/WORKFLOW.md.template` as the base. Replace the placeholders:

| Placeholder | Source |
|---|---|
| `{{PROJECT_SLUG}}` | First argument |
| `{{REPO_CLONE_URL}}` | Second argument (or `git remote get-url origin`) |
| `{{PROJECT_NAME}}` | From project docs (Step 1) |
| `{{PROJECT_DESCRIPTION}}` | From project docs (Step 1) -- 2-3 sentence summary |
| `{{BUILD_AND_TEST_COMMANDS}}` | From project docs (Step 1) -- formatted as a markdown list under a `Build and validation commands:` header |
| `{{PROJECT_RULES}}` | From project docs (Step 1) -- formatted as a markdown list under a `Rules:` header |

Write the result to `WORKFLOW.md` in the repository root.

If `WORKFLOW.md` already exists, warn the user and ask before overwriting.

## Step 4: Verify Setup

Run these checks:

1. Confirm all 7 skill files exist in `.codex/skills/`:
   ```
   ls .codex/skills/*/SKILL.md .codex/skills/land/land_watch.py
   ```
2. Confirm `WORKFLOW.md` exists and has valid YAML front matter:
   ```
   head -3 WORKFLOW.md
   ```
3. Confirm `LINEAR_API_KEY` is set:
   ```
   [ -n "$LINEAR_API_KEY" ] && echo "ok" || echo "missing"
   ```
4. Confirm `codex` is installed:
   ```
   command -v codex
   ```

## Step 5: Report and Guide

Print a summary:

```
Symphony Setup Complete
=======================

Repository: {repo_name}
Linear project: {project_slug}
Skills installed: commit, debug, land, linear, pull, push
WORKFLOW.md: created

Remaining manual steps:
1. Add custom Linear statuses in Team Settings -> Workflow:
   - "Rework" (started state)
   - "Human Review" (started state)
   - "Merging" (started state)

2. Start Symphony:
   cd ~/Code/symphony
   ./bin/symphony /path/to/repo/WORKFLOW.md --port 4000

3. Commit the new files:
   git add .codex/skills/ WORKFLOW.md
   git commit -m "feat: add Symphony workflow and Codex skills"
```

## Notes

- The `linear` skill requires Symphony's `linear_graphql` app-server tool -- it only works during active Symphony sessions.
- The `land` skill uses `land_watch.py` to monitor PR status asynchronously -- requires Python 3.
- The `debug` skill is for troubleshooting Symphony/Codex log issues -- not needed for normal operation but useful for diagnosing stuck runs.
- Symphony depends on non-standard Linear issue statuses: `Rework`, `Human Review`, and `Merging`. These must be added manually in Linear Team Settings -> Workflow.
