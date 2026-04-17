---
name: maestro:review
description: "Code review for a track against its spec and plan. Verifies implementation matches requirements, checks code quality and security."
argument-hint: "[<track-name>] [--current]"
---

# Review -- Track Code Review

Review the implementation of a track against its specification and plan. Verifies intent match, code quality, test coverage, and security.

## Arguments

`$ARGUMENTS`

- `<track-name>`: Match track by name or ID substring
- `--current`: Auto-select the in-progress (`[~]`) track
- No args: ask user which track to review, or fall back to uncommitted/staged changes if no tracks exist

---

## Step 1: Select Track

1. **If `--current`**: Find the `[~]` track in `.maestro/tracks.md`
2. **If track name given**: Match by ID or description substring
3. **If no args and tracks exist**: List completed and in-progress tracks, ask user
4. **If no args and no tracks**: Fall back to reviewing uncommitted changes via `git diff HEAD`

## Step 2: Load Track Context

Read all track files:
- `.maestro/tracks/{track_id}/spec.md` -- requirements to verify against
- `.maestro/tracks/{track_id}/plan.md` -- task SHAs and completion status
- `.maestro/tracks/{track_id}/metadata.json` -- track metadata
- `.maestro/context/code_styleguides/` -- code style references (if exist)
- `.maestro/context/product-guidelines.md` -- product/brand/UX guidelines (if exists)

## Step 3: Collect Commits

If `metadata.json` has `beads_epic_id`: use `br list --status closed --parent {epic_id} --all --json` and parse `close_reason` for SHAs (`sha:{7char}`). Otherwise: parse `plan.md` for all `[x] {sha}` markers.

If no SHAs found (and a track was selected): "Nothing to review." Stop.

If operating in arbitrary scope (no track), skip -- diff collected in Step 4.

## Step 4: Aggregate Diffs

```bash
# Track mode
git diff {first_sha}^..{last_sha}

# Arbitrary scope (no track)
git diff HEAD
```

If diff > 300 lines, offer Iterative Review Mode (per-file review).

## Step 5: Run Automated Checks

Run all applicable checks and interpret results before manual review. Automated checks catch mechanical issues so manual review can focus on logic and design.

### 5.1: Determine Check Commands

Detect from project config (`package.json`, `Makefile`, `pyproject.toml`, etc.):

```bash
CI=true {test_command}      # e.g., bun test, uv run pytest, ./gradlew test
{lint_command}               # e.g., eslint ., ruff check, clippy
{typecheck_command}          # e.g., tsc --noEmit, mypy, cargo check
{format_check_command}       # e.g., prettier --check ., ruff format --check
```

### 5.2: File-Type-Specific Checks

Run targeted checks based on which file types appear in the diff:

| File type | Check | What it catches |
|-----------|-------|-----------------|
| `.ts`, `.tsx` | `tsc --noEmit` | Type errors, missing imports, incorrect generics |
| `.ts`, `.tsx` | `eslint --no-warn-ignored {files}` | Lint violations, unused vars, style drift |
| `.py` | `ruff check {files}` | Lint, import order, complexity |
| `.py` | `mypy {files}` | Type errors, None handling |
| `.rs` | `cargo check` | Borrow checker, lifetime errors |
| `.rs` | `cargo clippy -- -D warnings` | Idiomatic Rust violations |
| `.go` | `go vet ./...` | Suspicious constructs |
| `.go` | `staticcheck ./...` | Bug-prone patterns |
| `.java` | `./gradlew check` or `./mvnw verify` | Compile errors, checkstyle |
| `.json` | Validate with `jq . < file > /dev/null` | Syntax errors |
| `.yaml`, `.yml` | Validate with language-appropriate parser | Syntax errors, indentation |
| `Dockerfile` | `hadolint` (if available) | Best practice violations |
| `.sql` | `sqlfluff lint` (if available) | SQL anti-patterns |

### 5.3: Interpret Results

For each check, report:

```
[ok] TypeScript: 0 errors
[!]  ESLint: 3 errors in 2 files (2 auto-fixable)
[ok] Tests: 47 passed, 0 failed
[--] Format: not configured
```

Classification:
- **[ok]**: Check passed -- no action needed
- **[!]**: Check failed -- findings become review items with severity
- **[--]**: Check not available -- note in report, not a blocker
- **[x]**: Check errored (tool crashed, config broken) -- investigate before proceeding

**Auto-fixable results**: If a linter reports auto-fixable issues (e.g., `eslint --fix`, `ruff format`), note the count separately. These feed into Step 8's auto-fix protocol.

## Step 6: Review Dimensions

Analyze the diff against 5 dimensions: intent match, code quality, test coverage, security, product guidelines.
See `reference/review-dimensions.md` for full criteria per dimension.

## Step 7: Generate Report

Format findings with severity ratings and checkbox verification.
See `reference/report-template.md` for the full report format and verdict options.

### Severity Classification

Every finding gets exactly one severity. Use these definitions consistently:

| Severity | Label | Meaning | Blocks approval? |
|----------|-------|---------|-------------------|
| **Blocker** | `[!!]` | Incorrect behavior, data loss risk, security vulnerability, spec violation | Yes -- must fix |
| **Major** | `[!]` | Significant quality issue, missing error handling, untested path, performance trap | Yes -- must fix or justify |
| **Minor** | `[?]` | Style inconsistency, naming, missing edge case test, documentation gap | No -- should fix |
| **Nit** | `[.]` | Formatting, subjective preference, trivial improvement | No -- optional |

**Rules for severity assignment:**
- Spec violations are always Blocker, regardless of how small
- Security issues are always Blocker unless clearly theoretical (e.g., timing attack on non-secret comparison)
- "I would have done it differently" is a Nit, not a Major
- If unsure between two severities, pick the lower one and explain your uncertainty
- Never mark something Blocker without a concrete failure scenario

## Step 8: Auto-fix Option

If verdict is not PASS, offer auto-fix options.
See `reference/report-template.md` for the auto-fix protocol and the boundary between auto-fixable and human-judgment issues.

## Step 9: Post-Review Cleanup

Offer archive, delete, keep, or skip for the track.
See `reference/report-template.md` for cleanup options.

---

## Reviewing Across Worktrees

When reviewing a task completed in a worktree (common in maestro workflows):

### Compare Worker Output to Spec

1. **Read the worker prompt**: `.maestro/features/{feature}/tasks/{task}/worker-prompt.md` contains the full spec the worker received
2. **Read the task report**: `.maestro/features/{feature}/tasks/{task}/report.md` for the worker's self-assessment
3. **Diff against main**: `git diff main...{task-branch}` to see only changes from this task
4. **Check for bleed**: Ensure the worker didn't modify files outside its task scope

### Cross-Task Consistency

When multiple tasks are complete:
- Check for conflicting patterns (e.g., Task A uses one error style, Task B another)
- Check for duplicated code across task branches
- Verify shared interfaces match between producer and consumer tasks

### Worktree-Specific Red Flags

- Worker committed `.maestro/` metadata changes (should not happen)
- Worker modified files belonging to another task's scope
- Worker left debugging artifacts (console.log, print statements, TODO markers)
- Worker diverged from the plan without documenting why in the task report

---

## Review Discipline

These principles govern every review, whether track-scoped or ad-hoc.

### Iron Laws

- Review against the task/plan first. Code quality comes second.
- Bias toward deletion and simplification. Every extra line is a liability.
- Prefer changes that leverage existing patterns and dependencies.
- Be specific: cite file paths and (when available) line numbers.
- Do not invent requirements. If the plan/task is ambiguous, mark it and request clarification.
- Only report findings you believe are >=80% likely to be correct. If unsure, explicitly label it as "Uncertain" and explain what evidence would confirm it.

### Review Layers (In Order)

**1) Identify Scope** -- List all files changed. For each file, state why it changed (what requirement it serves). Flag changes that do not map to the task/plan.

**2) Plan/Task Adherence (Non-Negotiable)** -- Create a checklist: what the task says must happen, evidence in code/tests that it happens. Flag missing requirements, partial implementations with no follow-up task, and behavior changes not in the plan.

**3) Correctness** -- Edge cases and error paths. Incorrect assumptions about inputs/types. Inconsistent behavior across platforms. Broken invariants. Prefer "fail fast, fail loud": invalid states should become clear errors, not silent fallbacks.

**4) Simplicity / YAGNI** -- Remove dead branches, unused flags/options, unreachable code. Remove speculative TODOs and "reserved for future" scaffolding. Inline one-off abstractions. Replace cleverness with obvious code. Reduce nesting with guard clauses / early returns. Avoid nested ternary operators; prefer `if/else` or `switch` when branches matter.

**5) De-Slop Pass (AI Artifacts / Style Drift)** -- Scan the diff for AI-generated slop:
- Extra comments that a human would not add, or that do not match the file's tone
- Defensive checks or try/catch blocks abnormal for that area of the codebase (especially swallowed errors, silent fallbacks, redundant validation in trusted internal codepaths)
- TypeScript escape hatches (`as any`, `as unknown as X`) without necessity
- Style drift: naming, error handling patterns, logging style inconsistent with nearby code
- Default stance: prefer deletion over justification. Validate at boundaries; keep internals trusting parsed inputs. When recommending simplifications, do not accidentally change behavior.

**6) Risk (Security / Performance / Maintainability)** -- No secrets in code/logs. No injection vectors introduced. Authz/authn checks preserved. Avoid N+1 queries, repeated parsing, large sync operations. Clear naming, consistent error handling, API boundaries not blurred.

**7) Primary Recommendation** -- One clear path to reach approval. Mention alternatives only when they have materially different trade-offs.

**8) Effort Estimate** -- Tag required follow-up: Quick (<1h), Short (1-4h), Medium (1-2d), Large (3d+).

### Common Review Smells (Fast Scan)

Task/plan adherence:
- Adds features not mentioned in the plan/task
- Leaves TODOs as the mechanism for correctness
- Introduces new configuration modes/flags "for future"

YAGNI / dead code:
- Options/config that are parsed but not used
- Branches that do the same thing on both sides
- Comments like "reserved for future" or "we might need this"

AI slop / inconsistency:
- Commentary that restates code, narrates obvious steps, or adds process noise
- try/catch that swallows errors or returns defaults without a requirement
- `as any` used to silence type errors instead of fixing types
- New helpers/abstractions with a single call site

Correctness:
- Silent fallbacks to defaults on error when the task expects a hard failure
- Unhandled error paths, missing cleanup, missing returns

Maintainability:
- Abstractions used once
- Unclear naming, "utility" grab-bags

### Output Format

---

**Files Reviewed:** [list]

**Plan/Task Reference:** [task name + link/path to plan section if known]

**Overall Assessment:** [APPROVE | REQUEST_CHANGES | NEEDS_DISCUSSION]

**Bottom Line:** 2-3 sentences describing whether it matches the task/plan and what must change.

#### Critical Issues
- None | [file:line] - [issue] (why it blocks approval) + (recommended fix)

#### Major Issues
- None | [file:line] - [issue] + (recommended fix)

#### Minor Issues
- None | [file:line] - [issue] + (suggested fix)

#### YAGNI / Dead Code
- None | [file:line] - [what to remove/simplify] + (why it is unnecessary)

#### Positive Observations
- [at least one concrete good thing]

#### Action Plan
1. [highest priority change]
2. [next]
3. [next]

#### Effort Estimate
[Quick | Short | Medium | Large]

---

### When to Escalate

Use NEEDS_DISCUSSION (instead of REQUEST_CHANGES) when:
- The plan/task is ambiguous and multiple implementations could be correct
- The change implies a product/architecture decision not documented
- Fixing issues requires changing scope, dependencies, or public API

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- **You are here.** Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

Review works best after commits are made, as it analyzes git history to understand what was implemented. It compares the implementation against the spec from `/maestro:new-track` and the plan from `/maestro:implement`. If issues are found, use `/maestro:revert` to undo and re-implement, or apply fixes directly.

Remember: Good validation catches issues before they reach production. Be constructive but thorough in identifying gaps or improvements.
