---
name: work
description: Execute a plan or direct task with worker delegation and verification.
---

# Work Skill

## When to use

- Execute a plan from `.maestro/plans/`
- Run direct work from a description: `/work fix the login bug`
- Resume interrupted work: `/work --resume`

## Your role

You are the orchestrator.
- **Delegate** all implementation to worker subagents.
- **Never** edit product code directly.
- Assign tasks, verify results, commit verified work, manage quality gates.
- If no subagent tool is available, execute tasks inline (you become the worker).

## Arguments

| Argument | Meaning |
|----------|---------|
| `<plan-name>` | Load a plan by name or title substring |
| `--resume` | Skip completed tasks (`- [x]`) |
| _(none)_ | Auto-load if one plan; prompt if multiple |
| _(description)_ | Planless mode — see `reference/planless-flow.md` |

---

## Workflow

### Step 1: Load plan

1. Check `.maestro/handoff/*.json` for `"status": "designing"`. If found, warn the user and wait for confirmation.

2. Scan `.maestro/plans/*.md` for available plans.

3. **If `<plan-name>` argument given:**
   - Match by filename or title substring (case-insensitive)
   - Not found? Check if it looks like a work description (contains spaces, >40 chars, or action verbs: `add`, `fix`, `create`, `update`, `implement`, `refactor`, `remove`, `change`, `move`, `build`). If so → `reference/planless-flow.md`
   - Otherwise → error: `Plan not found. Available: {list}`

4. **If no argument:**
   - 0 plans → error: `No plans found. Run /design first, or /work <description>.`
   - 1 plan → auto-load
   - Multiple → ask user which plan to execute

### Step 2: Confirm

1. Validate the plan has:
   - `## Objective` section (required)
   - At least one `- [ ]` checkbox (required)
   - `## Verification` section (required)
   - `## Scope` section (warn if missing, proceed)

2. Present summary: title, objective excerpt, task count, scope.

3. Ask: **"Execute this plan?"** — wait for explicit confirmation. If cancelled, stop.

### Step 3: Initialize

1. **Write handoff file:**
   ```json
   // .maestro/handoff/{plan-slug}.json
   {
     "topic": "{slug}",
     "status": "executing",
     "started": "{ISO timestamp}",
     "plan": ".maestro/plans/{slug}.md"
   }
   ```

2. **Worktree isolation** (optional) — ask user, then follow `reference/worktree-isolation.md` if chosen.

3. **Load priority context** from `.maestro/notepad.md` under `## Working Memory`. Items tagged P0-P2 become hard constraints appended to every task prompt.

4. **Discover skills** → see `reference/skill-injection.md`. Build a registry for task prompt injection.

### Step 4: Execute tasks

Work through each `- [ ]` checkbox in the plan, in dependency order.

For each task:

#### 4a. Build task prompt

Use this template for every worker assignment:

```markdown
## TASK
[Task title from the checkbox]

## EXPECTED OUTCOME
- [ ] Files created/modified: [paths from plan]
- [ ] Tests pass: [test command]
- [ ] No new errors

## CONTEXT
[Background, constraints, related files from plan]

## SKILL GUIDANCE
[Only if matching skills found — see reference/skill-injection.md]

## MUST DO
- [Explicit requirements from plan]

## MUST NOT DO
- [Explicit exclusions]

## PRIORITY CONTEXT
- [P0-P2 items from notepad, if any]

## KNOWLEDGE CAPTURE
- If you discover a non-obvious constraint, append to .maestro/notepad.md under ## Working Memory.
- Emit <remember category="learning">description</remember> for non-trivial patterns.
```

#### 4b. Delegate

**Spawn a worker subagent** with the task prompt. Use whatever delegation tool your runtime provides (Task, handoff, subagent, etc.) — the point is isolated execution so you can verify independently.

Worker role selection:
| Role | Use for |
|------|---------|
| `kraken` | TDD, new features, multi-file changes |
| `spark` | Quick fixes, single-file changes, config |
| `build-fixer` | Build errors, lint failures, type errors |

If no subagent tool is available, do the work yourself inline.

#### 4c. Verify

After the worker finishes → see `reference/verification.md` for the full protocol.

Summary:
1. Read the modified files — confirm they exist and look correct
2. Run tests and build commands
3. Check against acceptance criteria from the plan

**If passes** → commit and mark done (4d).

**If fails** → retry up to 3 times:
1. Send failure details back to original worker (or fix inline)
2. Spawn a `build-fixer` for persistent build/lint errors
3. Spawn an `oracle` for diagnostic help on attempt 3
4. After 3 failures → mark task failed, ask user: Retry / Skip / Stop

#### 4d. Commit and mark done

1. Stage only this task's files: `git add <file1> <file2>`
2. Commit with conventional prefix:
   ```
   feat(<plan-scope>): <short task description>

   Plan: <plan-name>
   Task: <task subject>
   ```
3. Capture the commit SHA and annotate the plan checkbox:
   ```
   - [x] Task title <!-- commit: abc1234 -->
   ```

**Never end a session with 0 commits.** Each verified task gets its own commit.

#### 4e. Next task

Move to the next `- [ ]` checkbox. Repeat 4a-4d until all tasks are done.

**`--resume` mode**: Skip all `- [x]` checkboxes.

### Step 5: Quality gates

After all tasks are done, pass these gates before wrapping up:

1. **Run all verification commands** from the plan's `## Verification` section.
2. **Fix failures** — spawn a worker to fix, then re-run all verification from scratch.
3. **Security review** — if the plan has a `## Security` section → see `reference/security.md`
4. **Critic review** — for plans with >5 tasks or >5 files changed, spawn a `critic` agent for final review. If verdict is REVISE, send issues to workers and wait for fixes.

All gates must pass before proceeding.

### Step 6: Wrap up

1. **Extract wisdom** → see `reference/wisdom.md`
   Write learnings to `.maestro/wisdom/{slug}.md`.

2. **Archive plan:**
   ```bash
   mkdir -p .maestro/archive/
   mv .maestro/plans/{slug}.md .maestro/archive/{slug}.md
   ```

3. **Update handoff:**
   ```json
   { "status": "archived", "completed": "{ISO timestamp}" }
   ```

4. **Worktree cleanup** (if used) → see `reference/worktree-isolation.md`

5. **Append summary to notepad** (`.maestro/notepad.md` under `## Working Memory`):
   ```
   - [{date}] [work:{slug}] Completed: {N}/{total} tasks. Files: {count}. Security: {pass|skip}.
   ```

### Step 7: Report

Tell the user:
- Tasks completed: N/total
- Files created/modified
- Test results (command + pass/fail)
- Plan archived to `.maestro/archive/{slug}.md`
- Issues or follow-ups (if any)
- Worktree info (if used): branch name, merge command

Suggest: `Run /review to verify results against acceptance criteria.`

---

## Anti-patterns

| Don't | Do instead |
|-------|-----------|
| Edit files yourself | Delegate to workers |
| Skip verification | Verify every task before committing |
| Use one-line task prompts | Use the full delegation template |
| Skip wisdom extraction | Always write `.maestro/wisdom/` file |
| Forget to commit | Commit after each verified task |
| Start without handoff file | Always write handoff in Step 3 |

---

## Reference

| Topic | File |
|-------|------|
| Verification protocol | `reference/verification.md` |
| Security review | `reference/security.md` |
| Wisdom extraction | `reference/wisdom.md` |
| Worktree isolation | `reference/worktree-isolation.md` |
| Skill injection | `reference/skill-injection.md` |
| Planless flow | `reference/planless-flow.md` |
