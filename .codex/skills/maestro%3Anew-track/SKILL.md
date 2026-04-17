---
name: maestro:new-track
description: "Create a new feature/bug track with spec and implementation plan. Interactive interview generates requirements spec, then phased TDD plan. Use when starting work on a new feature, bug fix, or chore."
argument-hint: "<track description>"
---

# New Track -- Specification & Planning

Create a new development track with a requirements specification and phased implementation plan. Every feature, bug fix, or chore gets its own track.

## Arguments

`$ARGUMENTS`

The track description. Examples: `"Add dark mode support"`, `"Fix login timeout"`, `"Refactor connection pooling"`

---

## Step 1: Validate Prerequisites

**Inputs:** Filesystem state.

**Actions:**
1. Check `.maestro/context/product.md` exists. If not: "Run `/maestro:setup` first." Stop.
2. Check `.maestro/tracks.md` exists. If missing, create it with this header:

```markdown
# Tracks Registry

Active and completed development tracks.
```

**Outputs:** Confirmed `.maestro/` directory is initialized.

**Transition:** Proceed to Step 2 when both files exist.

**Failure:** If `.maestro/` does not exist at all, stop and instruct the user to run `/maestro:setup`. Do not create `.maestro/` manually -- setup does more than just create the directory.

---

## Step 2: Parse Input

**Inputs:** `$ARGUMENTS` string.

**Actions:**
1. Extract track description from `$ARGUMENTS`.
2. If empty, ask user for type (feature/bug/chore) and description.
3. If the description is too vague, ask for clarification before proceeding.

**Outputs:** A track description string (1-3 sentences).

**Transition:** Proceed to Step 3 when you have a description with enough detail to classify.

### Recognizing Vague Descriptions

| Input | Problem | Follow-up |
|-------|---------|-----------|
| "fix bug" | No indication of what bug | "Which bug? What's broken, and where?" |
| "improvements" | No specifics | "Which part of the system? What specific improvement?" |
| "update the thing" | Ambiguous target | "Which module or feature? What should change about it?" |
| "dark mode" | Acceptable | Clear enough to proceed -- can refine in interview |
| "Add rate limiting to REST API" | Good | Proceed directly |

**Rule:** If you can't classify the description as feature/bug/chore from the words alone, it's too vague. Ask once. If still vague after one follow-up, accept what you have and let the interview fill in gaps.

---

## Step 3: Generate Track ID

**Inputs:** Track description.

**Actions:** Generate ID in format `{shortname}_{YYYYMMDD}` (2-4 words, snake_case + date).

**Outputs:** Track ID string.

**Examples:**
- "Add dark mode support" --> `dark_mode_20260225`
- "Fix login timeout on slow connections" --> `login_timeout_fix_20260225`
- "Refactor connection pooling" --> `refactor_conn_pool_20260225`

**Rules:**
- Use the most distinctive words from the description (skip articles, prepositions)
- Bug fixes: include "fix" in the ID
- Max 4 words before the date
- Use today's date

**Transition:** Proceed to Step 4.

---

## Step 4: Duplicate Check

**Inputs:** Generated track ID, existing `.maestro/tracks/` directories.

**Actions:** Scan `.maestro/tracks/*` directories. Warn if any starts with the same short name prefix (the part before the date).

**Outputs:** Warning message if duplicate found, otherwise silent.

**If duplicate found:** Ask the user: "A track with a similar name already exists: `{existing_id}`. Continue creating a new track, or work on the existing one?"
- **Continue** -- Create the new track (user confirms it's distinct work)
- **Use existing** -- Stop and point user to the existing track

**Transition:** Proceed to Step 4.5 when duplicate check passes or user confirms continuation.

---

## Step 4.5: BR Bootstrap Check

**Inputs:** Filesystem state, `br` CLI availability.

**Actions:** If `.beads/` does not exist and `br` is available: `br init --prefix maestro --json`. Skip silently if `br` is not installed.

**Outputs:** `.beads/` directory created, or nothing.

**Transition:** Always proceed to Step 5 (this step never blocks).

---

## Step 5: Create Track Directory

**Inputs:** Track ID from Step 3.

**Actions:**
```bash
mkdir -p .maestro/tracks/{track_id}
```

**Outputs:** Empty track directory created.

**Transition:** Proceed to Step 6.

---

## Step 6: Auto-Infer Track Type

**Inputs:** Track description from Step 2.

**Actions:** Analyze description keywords to classify as `feature`, `bug`, or `chore`.

**Outputs:** Classified type, presented to user for confirmation only if ambiguous.

### Inference Decision Tree

```
Description contains bug keywords?
  --> YES: classify as "bug"
  --> NO:
    Description contains chore keywords?
      --> YES: classify as "chore"
      --> NO:
        Description contains feature keywords?
          --> YES: classify as "feature"
          --> NO: AMBIGUOUS -- ask user
```

### Keyword Lists

| Type | Keywords (match any) |
|------|---------------------|
| **bug** | fix, broken, error, crash, incorrect, regression, timeout, fail, wrong, missing, undefined, null, exception, 404, 500 |
| **chore** | refactor, cleanup, clean up, migrate, upgrade, rename, reorganize, extract, move, restructure, deprecate, remove, delete, update dependency |
| **feature** | add, build, create, implement, support, introduce, enable, new, allow, provide, expose, integrate |

### Confidence Levels

- **High confidence** (auto-classify, tell user): Description matches 2+ keywords from one type and 0 from others. Example: "Fix crash on login timeout" has "fix," "crash," "timeout" -- all bug keywords. Classify as bug, inform user: "Classifying as bug based on description."
- **Medium confidence** (auto-classify, ask to confirm): Description matches 1 keyword from one type. Example: "Add cleanup for stale sessions" -- "add" is feature, "cleanup" is chore. Ask: "This could be a feature (adding new cleanup behavior) or a chore (cleaning up existing code). Which is it?"
- **Low confidence** (must ask): No keywords match, or keywords from multiple types are present equally. Ask: "Is this a feature, bug, or chore?"

**Transition:** Proceed to Step 7 when type is determined.

---

## Step 7: Specification Interview

**Inputs:** Track type from Step 6, track description from Step 2.

**Actions:** Run the type-specific interview to gather requirements. See `reference/interview-questions.md` for all questions per type (feature/bug/chore).

**Key behaviors:**
1. **Batch questions** -- present all questions for the type in a single interaction
2. **Auto-infer what you can** -- scan the codebase before asking Q2 (interaction type) and Q5 (affected modules). Pre-fill obvious answers.
3. **Probe vague answers once** -- if an answer is one sentence or less, ask one follow-up. Accept after that.
4. **Don't ask what you know** -- if the project is clearly a CLI (no UI framework, no web server), don't offer "UI component" as an interaction type option.

**Outputs:** Complete set of interview answers covering: behavior, interaction type, constraints, edge cases, scope, and out-of-scope items.

**Transition:** Proceed to Step 8 when all questions are answered.

**Failure:** If user abandons the interview mid-way, save whatever answers you have and ask: "Want to continue later? I can save progress." Do NOT delete the track directory.

---

## Step 8: Draft Specification

**Inputs:** Interview answers from Step 7, spec template from `reference/spec-template.md`.

**Actions:**
1. Compose the spec from interview answers using the template structure.
2. Use the type-specific variation (bug specs have Reproduction sections, chore specs have Scope of Change sections -- see `reference/spec-template.md`).
3. Run the quality checklist from the template before presenting.
4. Present full draft for approval.

**Outputs:** Complete spec document, presented to user for approval.

### Draft Quality Gates

Before presenting the spec to the user, verify these internally (do not show the checklist to the user):

- [ ] Overview states the "why" -- not just the "what"
- [ ] Every functional requirement is independently testable
- [ ] Edge cases section has at least 3 items
- [ ] Out of Scope section has at least 2 items
- [ ] Acceptance criteria are binary pass/fail
- [ ] No implementation details leaked into requirements

If any gate fails, fix the draft before presenting. Do not present a draft you know is incomplete.

### Approval Loop

Present the full spec to the user. Max 3 revision rounds.

**Round 1-2:** Apply requested changes, re-present. Normal iteration.

**Round 3 (final):** If still not approved, ask: "We've been through 3 rounds. Should I apply your latest feedback and finalize, or do we need to step back and reconsider the scope?"

**When to push back (politely, once):**
- User adds scope that contradicts Out of Scope: "This was listed as out of scope -- should I move it in scope?"
- User removes all edge cases: "I'd recommend keeping at least the error handling cases."
- User makes acceptance criteria untestable: "How would we verify that? Can we make it a specific check?"

After pushing back once, accept the user's decision.

**Outputs:** Approved spec written to `.maestro/tracks/{track_id}/spec.md`.

**Transition:** Proceed to Step 9 when spec is approved and written.

---

## Step 9: Generate Implementation Plan

**Inputs:** Approved spec from Step 8, project context files.

**Actions:**
1. Read context: `.maestro/context/workflow.md`, `tech-stack.md`, `guidelines.md`.
2. Scan the codebase for auto-inferable values (see `reference/plan-template.md` "Auto-Inference from Codebase" section): test framework, test file convention, source structure, module pattern, existing analogous features.
3. Present inferred defaults to the user: "I detected {framework} as your test framework and {dir} as your test directory. The plan will use these. Change? [yes/no]"
4. Generate the plan using `reference/plan-template.md` for structure and rules.
5. Apply TDD or ship-fast pattern based on `workflow.md` (default: TDD if not specified).
6. Present full plan for approval.

**Outputs:** Complete implementation plan with phases, tasks, and verification steps.

### Plan Quality Gates

Before presenting the plan, verify:

- [ ] Every spec requirement maps to at least one task
- [ ] Every phase produces a testable, demonstrable increment
- [ ] No phase is just "setup" with nothing testable
- [ ] Task count is within sizing guidelines (see plan template)
- [ ] Dependencies flow forward (no circular references)
- [ ] Phase verification steps have concrete commands, not just "run tests"

### Approval Loop

Same protocol as spec approval (Step 8): max 3 rounds, push back on scope creep, accept user decision after one objection.

**Outputs:** Approved plan written to `.maestro/tracks/{track_id}/plan.md`.

**Transition:** Proceed to Step 9.5 when plan is approved and written.

---

## Step 9.5: Detect Relevant Skills

**Inputs:** Track description, spec content, runtime's installed skill list.

**Actions:** Scan the runtime's installed skill list. Record skills whose description matches this track's domain/tech. Store names + relevance in metadata.json `skills` array.

**Outputs:** List of matched skill names (may be empty).

**When to populate:** Only include skills whose description has a clear keyword match with the track's tech stack or domain. "maestro:tdd" matches if the plan uses TDD pattern. "maestro:review" always matches. Don't include skills based on vague associations.

**When to leave empty:** If no skills match, set `"skills": []`. Do not force matches.

**Transition:** Proceed to Step 9.7.

---

## Step 9.7: Plan-to-BR Sync

**Inputs:** Approved plan, `.beads/` directory state, `br` CLI availability.

**Actions:** If `.beads/` directory exists AND `command -v br` succeeds: run plan-to-BR sync per `reference/plan-to-br-sync.md` (in the `maestro:implement` skill). Otherwise skip entirely.

**Outputs:** BR epic and issues created (or nothing).

**Transition:** Always proceed to Step 10-12 (this step never blocks).

---

## Step 10-12: Write Metadata, Index, and Registry

**Inputs:** All data from prior steps.

**Actions:** Write `metadata.json`, `index.md`, update `tracks.md`. See `reference/metadata-and-registry.md` for all schemas, templates, commit message, and summary format.

**Outputs:** Three files written/updated:
- `.maestro/tracks/{track_id}/metadata.json`
- `.maestro/tracks/{track_id}/index.md`
- `.maestro/tracks.md` (appended)

**Transition:** Proceed to Step 13.

---

## Step 13: Commit

**Inputs:** All files created in Steps 5-12.

**Actions:**
```bash
git add .maestro/tracks/{track_id} .maestro/tracks.md
# Include beads state if BR sync was performed
[ -d ".beads" ] && git add .beads/
git commit -m "chore(maestro:new-track): add track {track_id}"
```

**Outputs:** Git commit with all track files.

**Transition:** Proceed to Step 14.

**Failure:** If git commit fails (dirty working tree, hook failure), report the error and ask the user how to proceed. Do NOT force-commit or skip hooks.

---

## Step 14: Summary

**Inputs:** All metadata from prior steps.

**Actions:** Display track creation summary.

**Output format:**
```
## Track Created

**{track description}**
- ID: `{track_id}`
- Type: {type}
- Phases: {count}
- Tasks: {count}

**Files**:
- `.maestro/tracks/{track_id}/spec.md`
- `.maestro/tracks/{track_id}/plan.md`
- `.maestro/tracks/{track_id}/metadata.json`
- `.maestro/tracks/{track_id}/index.md`

**Next**: `/maestro:implement {track_id}`
```

---

## Red Flags -- STOP and Fix

These indicate the spec or plan has problems. Fix before proceeding.

| Red Flag | Problem | Fix |
|----------|---------|-----|
| Spec has no edge cases | Requirements not thought through | Generate at least 3 from the requirements |
| Acceptance criteria say "works correctly" | Not testable | Rewrite as specific, verifiable checks |
| Plan has a "setup" phase with nothing testable | Over-scaffolding, no increment | Merge setup tasks into first real phase |
| Single task covers 5+ files | Task too big | Split by file group or concern |
| Plan has 20+ tasks | Scope too large for one track | Split into multiple tracks |
| Spec mentions specific technology in requirements | Implementation detail leaked | Rewrite as a behavior requirement |
| All tasks are "implement X" with no test tasks | TDD not applied | Inject TDD sub-tasks per plan template |
| Phase has no completion verification | No checkpoint | Add verification meta-task |
| Description matches existing track | Duplicate work | Check with user: extend existing or start new |

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- **You are here.** Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

A track created here produces `spec.md` and `plan.md` that `/maestro:implement` consumes. The spec also serves as the baseline for `/maestro:review` to validate against. Good specs lead to good implementations -- be thorough in the interview.
