# Worktree Isolation — Work Phase

### Step 1.7: Worktree Isolation (Optional)

Ask the user whether to execute in an isolated worktree or in the current working tree:

```
DECIDE(
  question: "Where should this plan execute?",
  options: [
    { label: "Execute in worktree (isolated)", description: "Creates a git worktree on a new branch. Safe for parallel execution." },
    { label: "Execute in main tree (current behavior)", description: "Run directly in the current working directory." }
  ],
  blocking: true,
  default: "Execute in main tree (current behavior)"
)
```

**If main tree chosen**: Proceed to Step 2. No worktree fields are added to the handoff JSON.

**If worktree chosen**, follow the git-worktrees skill workflow:

#### 1. Resolve Worktree Directory

Determine the worktree root using this priority chain:

| Priority | Location | Notes |
|----------|----------|-------|
| 1 | `.worktrees/` | Default — at project root |
| 2 | `worktrees/` | Alternate — same level |
| 3 | Project instructions preference | If project instruction files (for example `AGENTS.md` / `CLAUDE.md`) specify a custom path |
| 4 | Ask user | Prompt for directory if none of the above exist |

#### 2. Safety Check — Gitignore

```bash
git check-ignore -q .worktrees
```

- If exit 0: `.worktrees/` is already ignored — proceed.
- If exit 1: auto-add to `.gitignore`:

```bash
echo "" >> .gitignore
echo "# Maestro worktrees (auto-added)" >> .gitignore
echo ".worktrees/" >> .gitignore
```

#### 3. Create Worktree

Derive `<plan-slug>` from the plan filename (without `.md`).

```bash
git worktree add "<worktree-dir>/<plan-slug>" -b "maestro/<plan-slug>"
```

#### 4. Copy Plan and Create Runtime Directories

```bash
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/plans"
cp ".maestro/plans/<plan-slug>.md" "<worktree-dir>/<plan-slug>/.maestro/plans/"

mkdir -p "<worktree-dir>/<plan-slug>/.maestro/handoff"
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/drafts"
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/wisdom"
mkdir -p "<worktree-dir>/<plan-slug>/.maestro/archive"
```

#### 5. Project Setup

Detect and run the project's setup command inside the worktree directory:

| File | Setup Command |
|------|--------------|
| `package.json` | `bun install` |
| `Cargo.toml` | `cargo build` |
| `pyproject.toml` | `uv sync` |
| `go.mod` | `go mod download` |
| `build.gradle` / `gradlew` | `./gradlew build` |
| `pom.xml` / `mvnw` | `./mvnw install` |

#### 6. Test Baseline Verification

Run the project's test command inside the worktree to confirm a clean baseline. If tests fail, warn the user that failures are pre-existing and proceed.

#### 7. Update Handoff

Update the handoff JSON with worktree metadata:

```json
{
  "worktree": true,
  "worktree_path": "<absolute path to worktree>",
  "worktree_branch": "maestro/<plan-slug>"
}
```

**All subsequent steps operate inside the worktree directory.**

#### Error Handling

If worktree creation fails (e.g., branch name collision, dirty state, disk issues), fall back to main tree execution with a warning:

> Worktree creation failed: {error}. Falling back to main tree execution.

Proceed to Step 2 without worktree fields in the handoff.

### Worktree Cleanup (Step 8.7)

**Skip this step if the handoff does not have `"worktree": true`.**

If execution ran in a worktree, perform cleanup from the **main tree** (not from inside the worktree):

#### 1. Report Branch

Tell the user which branch contains the changes:

> Plan complete. Changes are on branch: `maestro/<plan-slug>`
> Worktree path: `<worktree-path>`
> You can merge with: `git merge maestro/<plan-slug>`
> Or create a PR from this branch.

#### 2. Ask User About Worktree Removal

```
DECIDE(
  question: "Remove the worktree now?",
  options: [
    { label: "Remove worktree", description: "Delete the worktree directory. The branch is preserved for merge/PR." },
    { label: "Keep worktree", description: "Leave it in place for manual inspection. Remove later with: git worktree remove <path>" }
  ],
  blocking: true,
  default: "Keep worktree"
)
```

#### 3. If Remove

```bash
git worktree remove "<worktree-path>"
```

Then check if the branch is fully merged:

```bash
git branch -d "maestro/<plan-slug>"
```

- If `git branch -d` succeeds: branch was fully merged, cleanup complete.
- If `git branch -d` fails (not merged): warn the user. Do NOT force-delete with `-D` unless the user explicitly confirms.

#### 4. If Keep

Leave the worktree in place. Log:

> Worktree preserved at `<worktree-path>`. Remove later with: `git worktree remove "<worktree-path>"`
