---
name: git-worktrees
description: "Creates isolated working directories using git worktrees for parallel plan execution. Enables multiple plans to run simultaneously without file conflicts. Use when running concurrent development tasks."
triggers: [worktree, isolation, parallel]
---

# Git Worktree Isolation

> Run multiple plans in parallel without stepping on each other's toes.

## When to Use

Use worktree isolation when:
- Executing a plan that modifies files other plans also touch
- Running multiple `/work` sessions simultaneously
- You want a clean baseline without stashing or committing in-progress work

## Directory Priority Chain

The worktree root is resolved in this order:

| Priority | Location | Notes |
|----------|----------|-------|
| 1 | `.worktrees/` | Default — at project root (sibling of `.maestro/`, `.claude/`) |
| 2 | `worktrees/` | Alternate — same level |
| 3 | CLAUDE.md preference | If project CLAUDE.md specifies a custom path |
| 4 | Ask user | Prompt for directory if none of the above exist |

All paths are relative to the **project root** (the directory containing `.maestro/` and `.claude/`).

## Safety Verification

Before creating a worktree, verify the environment is safe:

### 1. Gitignore Check

```bash
git check-ignore -q .worktrees
```

- If the command **succeeds** (exit 0): `.worktrees/` is already ignored — proceed.
- If the command **fails** (exit 1): `.worktrees/` is NOT ignored — auto-add it:

```bash
echo "" >> .gitignore
echo "# Maestro worktrees (auto-added)" >> .gitignore
echo ".worktrees/" >> .gitignore
```

### 2. Repository State Check

Verify the repo is clean enough to create a branch:

```bash
git status --porcelain
```

- If there are uncommitted changes, warn the user but proceed (worktrees branch from HEAD regardless).
- If there are merge conflicts, **stop** — the user must resolve them first.

## Worktree Creation

### 1. Create the Worktree

```bash
git worktree add "<worktree-dir>/<plan-slug>" -b "maestro/<plan-slug>"
```

- `<worktree-dir>` is the resolved directory from the priority chain (e.g., `.worktrees/`)
- `<plan-slug>` is the plan filename without `.md` (e.g., `add-auth` from `add-auth.md`)
- The `-b` flag creates branch `maestro/<plan-slug>` tracking the current HEAD

### 2. Copy Plan File

```bash
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/plans"
cp ".maestro/plans/<plan-slug>.md" "<worktree-dir>/<plan-slug>/.maestro/plans/"
```

### 3. Create Runtime Directories

```bash
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/handoff"
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/drafts"
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/wisdom"
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/archive"
```

## Project Setup Auto-Detection

After creating the worktree, detect and run the project's setup commands **inside the worktree directory**:

| File | Setup Command | Notes |
|------|--------------|-------|
| `package.json` | `bun install` | Always use `bun`, never npm/yarn/pnpm |
| `Cargo.toml` | `cargo build` | Rust projects |
| `pyproject.toml` | `uv sync` | Always use `uv`, never pip/poetry/pipenv |
| `go.mod` | `go mod download` | Go modules |
| `build.gradle` or `gradlew` | `./gradlew build` | Use project wrapper when available |
| `pom.xml` or `mvnw` | `./mvnw install` | Use project wrapper when available |

Run the setup command from the worktree root:

```bash
cd "<worktree-dir>/<plan-slug>"
# detect and run appropriate setup command
```

If multiple project files exist (e.g., monorepo), run each applicable setup command.

## Test Baseline Verification

Before starting any work in the worktree, verify the existing test suite passes:

```bash
cd "<worktree-dir>/<plan-slug>"
# Run the project's test command (detected from project conventions)
```

- If tests **pass**: clean baseline confirmed — proceed with plan execution.
- If tests **fail**: warn the user. The failures are pre-existing and not caused by the plan. Log the failures and proceed, but note them in the final report.

## Completion Workflow

When plan execution finishes (all tasks completed or user stops):

### 1. Merge Wisdom Back

Copy any wisdom files generated during execution back to the main tree:

```bash
cp "<worktree-dir>/<plan-slug>/.maestro/wisdom/"* ".maestro/wisdom/" 2>/dev/null
```

### 2. Report Branch

Report the branch name to the user for merge or PR:

```
Plan complete. Changes are on branch: maestro/<plan-slug>
You can merge with: git merge maestro/<plan-slug>
Or create a PR from this branch.
```

### 3. Worktree Cleanup

Ask the user whether to remove the worktree now or keep it for inspection:

- **If remove:**

```bash
git worktree remove "<worktree-dir>/<plan-slug>"
```

- Optionally delete the branch if fully merged:

```bash
git branch -d "maestro/<plan-slug>"
```

- If not merged, warn before force-deleting — let the user decide:

```bash
# Only if user confirms:
git branch -D "maestro/<plan-slug>"
```

- **If keep:** Leave the worktree in place. The user can remove it later with `git worktree remove`.

## Common Mistakes

- **Using npm/yarn/pnpm instead of bun** — This project uses `bun` for all JavaScript/TypeScript operations. Never fall back to npm.
- **Using pip/poetry instead of uv** — This project uses `uv` for Python. Never fall back to pip.
- **Creating worktrees inside the project tree** — Worktrees must be in `.worktrees/` (or resolved equivalent) at the project root, not nested inside `src/` or other directories.
- **Forgetting to copy the plan file** — The worktree needs its own copy of the plan in `.maestro/plans/` so workers can find it.
- **Not running setup in the worktree** — Dependencies must be installed in the worktree separately. Symlinked `node_modules` or virtualenvs from the main tree will not work.
- **Force-deleting unmerged branches** — Always use `git branch -d` first. Only use `-D` with explicit user confirmation.
- **Skipping gitignore verification** — If `.worktrees/` is not in `.gitignore`, worktree contents could be accidentally committed.

## Red Flags

- **Worktree directory already exists** — A previous run may not have cleaned up. Check `git worktree list` before creating.
- **Branch name collision** — `maestro/<plan-slug>` already exists. Either the plan was run before (offer to reuse or pick a new name) or there is a naming conflict.
- **Dirty worktree at completion** — Uncommitted changes in the worktree at completion time. Warn the user — these changes will be lost if the worktree is removed.
- **Main tree moved ahead** — If the main branch advanced while the worktree was active, the merge may have conflicts. Suggest rebasing the worktree branch first.
- **Disk space** — Worktrees duplicate the working tree. For large repos with heavy build artifacts, this can consume significant disk space.

## Integration Points

- **Called by**: `/work` command — offers worktree isolation before spawning workers
- **Reads from**: `.maestro/plans/` — the plan file to execute
- **Writes to**: `.maestro/wisdom/` — merged back from worktree on completion
- **Depends on**: `git` with worktree support (Git 2.5+)
- **Complements**: `project-conventions` skill — used for setup auto-detection
