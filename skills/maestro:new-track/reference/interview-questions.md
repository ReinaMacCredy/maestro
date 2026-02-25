# Specification Interview Questions

Generate a requirements specification through interactive questioning. Batch independent questions into a single interaction if your runtime supports it.

---

## For Features

Batch these into a single interaction if your runtime supports it:

**Q1:** Ask the user: "What should this feature do? Describe the core behavior and expected outcomes."

**Q2:** Ask the user: "How will users interact with this feature?"
Options:
- **UI component** -- Visual element users see and interact with
- **API endpoint** -- Programmatic interface
- **CLI command** -- Terminal command or flag
- **Background process** -- No direct user interaction

**Q3:** Ask the user: "Any constraints or non-functional requirements? (performance, security, compatibility)" (select all that apply)
Options:
- **No special constraints** -- Standard quality expectations
- **Performance-critical** -- Must meet specific latency/throughput targets
- **Security-sensitive** -- Handles auth, PII, or financial data
- **Let me specify** -- Type your constraints

**Q4:** Ask the user: "Any known edge cases or error scenarios to handle?"
Options:
- **I'll list them** -- Type known edge cases
- **Infer from requirements** -- Generate edge cases from the spec

---

## For Bugs

Batch these into a single interaction if your runtime supports it:

**Q1:** Ask the user: "What is happening? Provide steps to reproduce."

**Q2:** Ask the user: "What should happen instead?"

**Q3:** Ask the user: "How critical is this? Which users or flows are affected?"
Options:
- **Blocker** -- Core flow broken, no workaround
- **High** -- Significant degradation, workaround exists
- **Medium** -- Noticeable issue, limited impact
- **Low** -- Minor or cosmetic

---

## For Chores

Batch these into a single interaction if your runtime supports it:

**Q1:** Ask the user: "What needs to change and why?"

**Q2:** Ask the user: "Any backward compatibility requirements?"
Options:
- **Must be backward compatible** -- No breaking changes to public API
- **Breaking changes acceptable** -- Semver major bump is fine
- **Internal only** -- No public surface affected

---

## Spec Draft Template

```markdown
# Specification: {track description}

## Overview
{one-paragraph summary}

## Type
{feature | bug | chore}

## Requirements

### Functional Requirements
{from Q1}

### User Interaction
{from Q2}

### Non-Functional Requirements
{from Q3}

## Edge Cases & Error Handling
{from Q4 or inferred}

## Out of Scope
{explicitly list what this track does NOT cover}

## Acceptance Criteria
- [ ] {criterion 1}
- [ ] {criterion 2}
- [ ] {criterion 3}
```

## Spec Approval Loop

Present the full draft to the user by embedding the entire spec content in the question field:

Ask the user: "Here is the drafted specification -- does it look correct?\n\n---\n{full spec content}\n---"
Options:
- **Approved** -- Spec is ready, generate the plan
- **Needs revision** -- I'll tell you what to change

If revision needed: ask what to change, update, and re-present. Max 3 revision loops.

Write approved spec to `.maestro/tracks/{track_id}/spec.md`.

---

## Plan Generation

Read project context for informed planning:
- `.maestro/context/workflow.md`
- `.maestro/context/tech-stack.md`
- `.maestro/context/guidelines.md`

Use the plan template from `reference/plan-template.md`.

### Plan Rules

1. **Phases** group related tasks into logical milestones (1-4 phases typical)
2. **Tasks** are atomic work items completable in one session
3. **Sub-tasks** are granular steps within a task
4. **TDD injection**: If workflow.md specifies TDD, every implementation task gets:
   - `Write failing tests for {task}` (Red)
   - `Implement {task} to pass tests` (Green)
   - `Refactor {task}` (optional cleanup)
5. **Phase Completion Verification**: Every phase ends with a verification block containing:
   - Test coverage check
   - Test suite execution
   - Manual verification steps
   - `- [ ] Maestro - User Manual Verification '{Phase Name}' (Protocol in workflow.md)`
6. **All items** get `[ ]` checkboxes for progress tracking
7. Tasks MUST be ordered by dependency (no forward references)

Present the full plan for approval by embedding the entire plan content directly in the question field (same pattern as spec approval). Max 3 revision loops.

Write approved plan to `.maestro/tracks/{track_id}/plan.md`.
