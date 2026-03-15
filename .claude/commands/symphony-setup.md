Set up Symphony orchestration for the current repository so that Codex agents can autonomously work Linear issues.

Arguments: $ARGUMENTS (format: `<linear-project-slug> <repo-clone-url>`)

Follow these steps exactly:

## 1. Gather project context

Read the project's `CLAUDE.md`, `AGENTS.md`, `README.md`, or `CODEX.md` to extract:
- Project name and one-line description
- Tech stack summary
- Build, test, and lint commands
- Key coding rules/conventions

Run `git remote get-url origin` to get the repo URL if not provided as an argument.

## 2. Copy Codex skills

The Symphony skill templates are at `skills/maestro:symphony-setup/reference/codex-skills/`. Copy them into `.codex/skills/` in this repo:

```
.codex/skills/commit/SKILL.md
.codex/skills/debug/SKILL.md
.codex/skills/land/SKILL.md
.codex/skills/land/land_watch.py
.codex/skills/linear/SKILL.md
.codex/skills/pull/SKILL.md
.codex/skills/push/SKILL.md
```

If `.codex/skills/` already has files, only add missing ones. Do not overwrite.

## 3. Generate WORKFLOW.md

Use the template at `skills/maestro:symphony-setup/reference/WORKFLOW.md.template`. Replace:

- `{{PROJECT_SLUG}}` -- Linear project slug (first argument)
- `{{REPO_CLONE_URL}}` -- git clone URL (second argument or `git remote get-url origin`)
- `{{PROJECT_NAME}}` -- from project docs
- `{{PROJECT_DESCRIPTION}}` -- 2-3 sentence summary from project docs
- `{{BUILD_AND_TEST_COMMANDS}}` -- build/test commands as markdown list with header `Build and validation commands:`
- `{{PROJECT_RULES}}` -- coding rules as markdown list with header `Rules:`

Write to `WORKFLOW.md` in the repo root. Warn before overwriting if it exists.

## 4. Verify

Check that:
- All 7 skill files exist under `.codex/skills/`
- `WORKFLOW.md` has valid YAML front matter
- `LINEAR_API_KEY` env var is set
- `codex` CLI is installed

## 5. Report

Print the setup summary including:
- What was created
- Remaining manual steps (Linear custom statuses: Rework, Human Review, Merging)
- How to start Symphony: `cd ~/Code/symphony && ./bin/symphony /path/to/repo/WORKFLOW.md --port 4000`
- Suggest committing the new files
