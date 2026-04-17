# Plan Template

## Structure

```markdown
# Implementation Plan: {title}

> Track: {track_id}
> Type: {feature | bug | chore}
> Created: {YYYY-MM-DD}

## Phase 1: {phase title}

### 1. {task title}

{Description -- what this task does and why}

#### Design
{Approach, architecture decisions, key implementation details}

#### Acceptance Criteria
- [ ] {verifiable condition}

### 2. {task title}
[depends: 1]

{Description}

#### Design
{Approach}

#### Acceptance Criteria
- [ ] {verifiable condition}

## Phase 2: {phase title}

### 3. {task title}
[depends: 1, 2]

{Description}

#### Design
{Approach}

#### Acceptance Criteria
- [ ] {verifiable condition}
```

---

## How to Decide Phase Boundaries

A phase is a group of tasks that produce a **testable, demonstrable increment**. If you can't demo the phase result to the user, it's not a real phase boundary.

**Good phase boundaries:**
- After data model + CRUD operations: "We can now create and read {entity} in the database"
- After API layer: "We can now call the endpoint and get a response"
- After UI integration: "The user can now see and interact with {feature} in the browser"

**Bad phase boundaries:**
- "Set up files" -- not demonstrable, just scaffolding
- "Write all tests" -- tests without code to test are not an increment
- "Refactoring" as its own phase -- refactoring happens within each task, not as a separate phase

**Decision rule:** If a phase has only setup/config tasks and nothing testable, merge it into the next phase as the first tasks.

### Phase Ordering

1. **Foundation first:** Data models, schemas, types, configuration
2. **Logic second:** Business logic, algorithms, processing
3. **Interface third:** API endpoints, CLI commands, UI components
4. **Integration last:** Connecting pieces, end-to-end flows, polish

Exception: Bug fixes are often single-phase. Don't force multi-phase structure on a 2-task bug fix.

---

## How to Size Tasks

A task should be completable in one focused session (30-90 minutes for a human, 1 tool-use cycle for an agent). If it takes longer, split it.

### Task Too Big -- Split When:

| Signal | Split Strategy |
|--------|---------------|
| Task description has "and" | Split at the "and" into separate tasks |
| More than 3 files to modify | Split by file group (model vs. handler vs. test) |
| Multiple acceptance criteria in the spec | One task per criterion |
| "Implement {feature}" for a complex feature | Split into sub-features: parse, validate, execute, format |

### Task Too Small -- Merge When:

| Signal | Merge Strategy |
|--------|---------------|
| Task is "create file" with no logic | Merge into the first task that uses that file |
| Task is "add import" or "update config" | Merge into the task that needs the import/config |
| Task takes <5 minutes | Combine with its dependent task |

### Good vs. Bad Task Decomposition

<Good>
```markdown
### Task 1.1: Rate limiter core logic
- [ ] Write failing tests for sliding window counter (increment, check, reset)
- [ ] Implement SlidingWindowLimiter class with in-memory store
- [ ] Refactor: extract TimeWindow helper if counter logic is complex

### Task 1.2: Rate limiter middleware
- [ ] Write failing tests for HTTP middleware (under limit, over limit, missing auth)
- [ ] Implement rateLimitMiddleware that uses SlidingWindowLimiter
- [ ] Refactor: extract response formatting if needed
```
Each task produces testable code. Clear boundaries. No overlap.
</Good>

<Bad>
```markdown
### Task 1.1: Set up rate limiting
- [ ] Create rate-limiter.ts file
- [ ] Add imports
- [ ] Define types

### Task 1.2: Implement rate limiting
- [ ] Write all the rate limiting code
- [ ] Write all the tests
- [ ] Make everything work
```
Task 1.1 produces nothing testable. Task 1.2 is "do everything" -- no guidance.
</Bad>

---

## Auto-Inference from Codebase

Before generating the plan, scan the project to infer these automatically. Only ask the user when inference fails or is ambiguous.

### What to Infer (scan the codebase)

| Signal | Where to look | What it tells you |
|--------|--------------|-------------------|
| Test framework | `package.json` scripts, test config files (`jest.config`, `vitest.config`, `pytest.ini`) | Which test runner commands to put in verification steps |
| Test file convention | Existing `*.test.*` or `*.spec.*` or `*_test.*` files | Where to create new test files, naming pattern |
| Source structure | `src/` vs. flat, `routes/` vs. `pages/` vs. `handlers/` | Where new files should go |
| Module pattern | ESM (`import/export`) vs. CJS (`require/module.exports`) | How to structure imports in new code |
| Existing patterns | How similar features were built (find the closest analog) | Task structure should mirror existing patterns |
| Build system | `tsconfig.json`, `Makefile`, `build.ts`, `webpack.config` | Whether a build step is needed before testing |
| CI config | `.github/workflows/`, `Jenkinsfile`, `.gitlab-ci.yml` | What CI checks the plan should anticipate |

### What Requires User Input (cannot infer)

| Decision | Why it can't be inferred |
|----------|------------------------|
| Phase ordering preferences | User may want to demo early, or build foundation first |
| Acceptable test coverage threshold | Project policy, not detectable from code |
| Whether to use TDD or ship-fast pattern | Workflow preference (check `workflow.md` first, ask if absent) |
| Priority of tasks within a phase | Business priority, not technical dependency |
| Manual verification steps | User knows what to check visually/manually |

### Presenting Inferred Defaults

When inference succeeds, present it as a default the user can override:

"I detected `vitest` as your test framework and `src/__tests__/` as your test directory. The plan will use `bun test` and place test files in `src/__tests__/{module}.test.ts`. Change this? [yes/no]"

Do NOT silently use inferred values without telling the user. Do NOT ask about values you can clearly detect.

---

## TDD Pattern Injection

For TDD methodology (from `workflow.md`), every implementation task gets three sub-tasks:

1. **Write failing tests** (Red)
   - Create test file if it doesn't exist
   - Write tests defining expected behavior from spec
   - Run tests -- MUST fail (confirms tests are meaningful)
   - Do NOT proceed until tests fail

2. **Implement to pass** (Green)
   - Write minimum code to make tests pass
   - Run tests -- MUST pass
   - Do NOT add code beyond what's needed to pass

3. **Refactor** (optional)
   - Improve code quality with passing tests as safety net
   - Run tests after refactoring -- MUST still pass

## Ship-fast Pattern (alternative)

For ship-fast methodology, implementation tasks get:

1. **Implement** -- Write the feature/fix code
2. **Add tests** -- Write tests covering the implementation
3. **Verify** -- Run tests, confirm passing

---

## Phase Completion Verification

Every phase ends with a verification meta-task:

1. **Automated test execution**
   - Announce exact command before running (e.g., `CI=true npm test`)
   - Run and report results
   - Max 2 fix attempts on failure; if still failing, halt and ask user

2. **Manual verification plan**
   - Generate step-by-step verification instructions
   - Include commands and expected outcomes
   - Frontend: start dev server, test UI interactions
   - Backend: verify API endpoints with curl/httpie
   - CLI: run commands with expected output

3. **User confirmation**
   - Wait for explicit user approval
   - Record checkpoint commit SHA

---

## Sizing Guidelines

| Scope | Phases | Tasks/Phase | Total Tasks |
|-------|--------|-------------|-------------|
| Small (1-2 files) | 1 | 1-3 | 1-3 |
| Medium (3-8 files) | 2-3 | 2-4 | 4-12 |
| Large (8+ files) | 3-4 | 3-5 | 9-20 |

**Over-planning signal:** More than 20 tasks means the scope is too large for a single track. Split into multiple tracks.

**Under-planning signal:** A "large" feature with only 2 tasks means each task is too big. Apply the "Task Too Big" splitting rules above.

---

## Dependency Rules

- Tasks within a phase are ordered by dependency
- No forward references (task N cannot depend on task N+1)
- Shared infrastructure tasks come first (models, schemas, config)
- UI tasks come after their backing API/logic tasks
- Integration tests come after unit tests
- Cross-phase dependencies are implicit: phase N+1 depends on phase N completion

**Circular dependency detected?** You over-coupled the tasks. Merge the circular tasks into one, or extract the shared piece into its own task that both depend on.
