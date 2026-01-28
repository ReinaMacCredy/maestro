---
description: Atlas interview-driven planning
argument-hint: "<what you want to plan>"
allowed-tools: Read, Glob, Grep, Task, AskUserQuestion, EnterPlanMode, ExitPlanMode, Write, Edit
---

# Atlas Plan Command

## IMMEDIATE ACTION

**Enter plan mode with reason:**

```javascript
EnterPlanMode({ reason: "Planning: $ARGUMENTS" })
```

**User Request:** $ARGUMENTS

---

## Your Role

You conduct the interview yourself in the main context (not delegating to prometheus). After gathering requirements, you spawn prometheus with GENERATE phase to write the plan.

## Architecture

```
Main Context (YOU):
├── EnterPlanMode({ reason: "..." })
├── Interview (domain detection + questions + codex choice)
├── Update pipeline-state.json with codex_choice
├── Spawn prometheus GENERATE
├── Metis gap analysis
├── Momus review loop
├── Optional Codex enhancement (if user chose "enhanced")
├── File tasks
└── ExitPlanMode()
```

---

## Workflow

### Phase 1: Interview (YOU do this in main context)

#### Step 1: Domain Detection

Read domain patterns from `skills/atlas/references/domain-questions.md`.

Match user request keywords (case-insensitive) against domains:

| Domain | Keywords |
|--------|----------|
| **AUTH** | oauth, login, auth, authentication, session, token, password, signup, signin |
| **API** | api, endpoint, rest, graphql, route, backend, server |
| **UI/UX** | ui, ux, frontend, component, page, design, layout, style, css |
| **TESTING** | test, testing, spec, coverage, e2e, unit, integration, qa |
| **REFACTOR** | refactor, cleanup, restructure, reorganize, modernize, migrate |
| **ARCHITECTURE** | architecture, system, design, infrastructure, scale, deploy, cloud |

**If multiple domains match:**
```javascript
AskUserQuestion({
  questions: [{
    question: "This request spans multiple domains. Which should I prioritize?",
    header: "Domain",
    multiSelect: false,
    options: [
      { label: "AUTH", description: "Focus on authentication/session questions" },
      { label: "API", description: "Focus on endpoint/backend questions" },
      // ... matched domains only
    ]
  }]
})
```

**If no domain matches:** Skip structured questions, go directly to freeform.

#### Step 2: Structured Questions (Domain-Specific)

Use the question patterns from `skills/atlas/references/domain-questions.md` for the detected domain. Ask 5-6 domain-specific questions using `AskUserQuestion`.

#### Step 3: Freeform Follow-ups

Ask 2-4 additional questions based on:
- Gaps in domain answers
- Edge cases
- Integration concerns
- Scope clarification

#### Step 4: Gather Codebase Context

After user answers:
- Read key files mentioned
- Check existing patterns in the codebase
- Note any existing plans in `.claude/plans/`

#### Step 5: Codex Choice (Upfront)

**CRITICAL**: Ask about plan generation mode DURING interview, not after:

```javascript
AskUserQuestion({
  questions: [{
    question: "How would you like the plan generated?",
    header: "Generator",
    multiSelect: false,
    options: [
      { label: "Standard (Recommended)", description: "Fast, reliable Claude generation" },
      { label: "Enhanced (Codex)", description: "Deeper analysis with Codex 5.2 (~10 min)" }
    ]
  }]
})
```

#### Step 6: Update Pipeline State

Store the codex choice and plan mode status for hooks to read.

**CRITICAL**: Create plan-scoped state in notepad directory, not global state.

```javascript
// After determining plan name from user request, normalize it to plan_id
const planName = "${plan_name}";  // e.g., "auth-system-refactor"

// IMPORTANT: Call bash helper for consistent normalization
// This ensures JavaScript matches bash exactly (path traversal, hash fallback, etc.)
const normalizeResult = await Bash({
  command: `source scripts/lib/hook-common.sh && generate_unique_plan_id "${planName.replace(/"/g, '\\"')}"`
});

const finalPlanId = normalizeResult.trim();
if (!finalPlanId) {
  console.error("Failed to generate plan_id");
  return;
}

// Create notepad directory (already created by generate_unique_plan_id, but ensure it exists)
Bash({
  command: `mkdir -p .atlas/notepads/${finalPlanId}`
});

// Create plan-scoped pipeline state (NOT global .atlas/pipeline-state.json)
Write({
  file_path: `.atlas/notepads/${finalPlanId}/pipeline-state.json`,
  content: JSON.stringify({
    "phase": "PLANNING",
    "codex_choice": "${user_choice}",  // "standard" or "enhanced"
    "plan_mode_active": true,          // Track that EnterPlanMode succeeded
    "momus_iterations": 0,
    "plan_name": planName,              // Original human-readable name
    "plan_id": finalPlanId              // Normalized ID for directory/locking
  }, null, 2)
});
```

**Why plan_id matters**:
- `plan_name`: Human-readable (e.g., "Auth System Refactor")
- `plan_id`: Filesystem-safe (e.g., "auth-system-refactor")
- Hooks use `plan_id` for locking: `acquire_lock(plan_lock_name(plan_id))`
- Notepad directory path: `.atlas/notepads/${plan_id}/`

**Default**: If pipeline-state.json creation fails, continue with defaults:
- `codex_choice: "standard"`
- `plan_mode_active: false` (ExitPlanMode will be skipped if EnterPlanMode failed)

**Note**: The `plan_mode_active` flag tells hooks whether to include ExitPlanMode instructions. If EnterPlanMode failed, this flag remains false and hooks will skip ExitPlanMode calls.

---

### Phase 2: Generate Plan

Spawn prometheus with GENERATE phase (interview complete, no questions):

```javascript
Task({
  subagent_type: "atlas-prometheus",
  prompt: `PHASE: GENERATE

## Collected Requirements

**Goal**: ${original_request}

**Domain Detected**: ${detected_domain}

**Technical Decisions**:
${all_answers_from_interview}

**Scope**:
- MUST-HAVE: ${must_have_features}
- EXCLUDED: ${excluded_items}

**Test Strategy**: ${tdd_or_manual}

**Codebase Context**:
${relevant_patterns_and_files}

## Instructions

Skip interview. Requirements are above.
Generate plan directly to .claude/plans/{appropriate-name}.md
Do NOT ask any questions - all decisions already made.
Output PLAN_READY JSON when done.
`
})
```

---

### Phase 3: Mandatory Review Pipeline

After prometheus returns PLAN_READY:

#### Step 1: Metis Gap Analysis

```javascript
Task({
  subagent_type: "atlas-metis",
  prompt: `Analyze plan for hidden requirements and AI failure modes.
Plan file: ${plan_path}
Output JSON: {"status": "GAP_ANALYSIS_COMPLETE", "critical_questions": [...], "must_do": [...], "must_not_do": [...]}`
})
```

If Metis finds critical questions, present them to user via `AskUserQuestion` and update plan.

#### Step 2: Momus Review Loop (max 5 iterations)

```javascript
Task({
  subagent_type: "atlas-momus",
  prompt: `Review plan for gaps and ambiguities.
Plan file: ${plan_path}
Output JSON: {"verdict": "OKAY"} or {"verdict": "REJECT", "issues": [...]}`
})
```

**If REJECT**: Spawn prometheus to fix issues:

```javascript
Task({
  subagent_type: "atlas-prometheus",
  prompt: `PHASE: REVISE

Plan file: ${plan_path}

Momus identified these issues:
${momus_issues}

Address each issue and revise the plan.
Output JSON: {"status": "PLAN_REVISED"}`
})
```

Then re-run Momus (loop until OKAY or max 5 iterations).

**If OKAY**: Continue to Step 3.

#### Step 3: Codex Enhancement (Conditional)

**Check pipeline-state.json for codex_choice**:
- If `"standard"` or missing: Skip to Phase 4
- If `"enhanced"`: Run Codex enhancement

**Codex Enhancement Flow**:

1. Write handoff to `.atlas/codex/handoffs/{plan-name}.md`:
   - Include full plan content
   - Include interview context and Metis analysis
   - Request edge cases, security, error handling additions

2. Run Codex CLI in background:
   ```javascript
   Bash({
     command: "./scripts/codex-cli.sh .atlas/codex/handoffs/{plan-name}.md {plan-name}",
     run_in_background: true
   })
   ```

3. Poll with `TaskOutput({ task_id, block: true })` - repeat if timeout (Codex may take 10-15 min)

4. If Codex succeeds, read and merge:
   ```javascript
   Read({ file_path: ".atlas/codex/results/{plan-name}.md" })
   // Merge into .claude/plans/{plan-name}.md
   ```

5. Have prometheus review Codex enhancements:
   ```javascript
   Task({
     subagent_type: "atlas-prometheus",
     prompt: `PHASE: REVIEW_CODEX_ENHANCEMENT

   Verify enhancements are integrated (not appended), no scope creep, all original content preserved.
   Fix any issues directly.
   Output JSON: {"status": "CODEX_REVIEWED"}`
   })
   ```

---

### Phase 4: File Tasks

Parse the plan's `## TODOs` section and create tasks:

```javascript
// For each task in the plan:
TaskCreate({
  subject: "1. Task title from plan",
  description: "Full task details with acceptance criteria",
  activeForm: "Implementing task 1"
})
```

Link dependencies with `TaskUpdate({ taskId, addBlockedBy: [...] })`.

---

### Phase 5: Exit Plan Mode

```javascript
ExitPlanMode()
```

User can now run `/atlas-work ${plan_name}` to begin execution.

---

## Error Handling

### EnterPlanMode Failure

If `EnterPlanMode()` fails or returns an error:
1. Log the error
2. Continue with interview anyway (plan mode is optional enhancement)
3. Note in plan that plan mode was unavailable

### Domain Detection Edge Cases

| Scenario | Action |
|----------|--------|
| Multiple domains detected | Ask user to prioritize |
| No domain matches | Skip structured questions, freeform only |
| User overrides with `@plan:auth` | Force specified domain pattern |

### Pipeline State Missing

If `.atlas/pipeline-state.json` doesn't exist or is corrupted:
1. Create new state file with defaults
2. Set `codex_choice: "standard"` as fallback
3. Continue with pipeline

---

## Quick Reference

| Phase | Step | Action | Tool |
|-------|------|--------|------|
| 1 | Start | Enter plan mode | `EnterPlanMode({ reason })` |
| 1 | Detect | Match domain keywords | Keyword matching |
| 1 | Interview | Ask domain + freeform questions | `AskUserQuestion` |
| 1 | Choice | Ask about Codex vs Standard | `AskUserQuestion` |
| 1 | State | Store codex_choice | `Read`, `Edit` |
| 1 | Context | Read codebase | `Read`, `Glob`, `Grep` |
| 2 | Generate | Prometheus GENERATE | `Task(atlas-prometheus)` |
| 3.1 | Review | Metis gap analysis | `Task(atlas-metis)` |
| 3.2 | Review | Momus approval loop | `Task(atlas-momus)` |
| 3.2 | Fix | Prometheus REVISE (if REJECT) | `Task(atlas-prometheus)` |
| 3.3 | Enhance | Codex enhancement (if "enhanced") | `Bash(codex-cli.sh)` |
| 3.3 | Review | Prometheus reviews Codex | `Task(atlas-prometheus)` |
| 4 | Tasks | File from plan | `TaskCreate`, `TaskUpdate` |
| 5 | End | Exit plan mode | `ExitPlanMode()` |

---

## Flow Diagram

```
EnterPlanMode({ reason: "Planning: <request>" })
       │
       ▼
Domain Detection (match keywords)
       │
       ▼
Structured Questions (domain-specific)
       │
       ▼
Freeform Follow-ups
       │
       ▼
Codex Choice → store in pipeline-state.json
       │
       ▼
Codebase Context Gathering
       │
       ▼
Prometheus GENERATE → plan file
       │
       ▼
Metis gap analysis
       │
       ▼
Momus review ◄────────────────┐
       │                       │
   OKAY?  ──NO──► Prometheus REVISE
       │
       ▼
codex_choice == "enhanced"?
       │           │
      YES          NO
       │           │
       ▼           │
Codex enhancement  │
       │           │
       ▼           │
Prometheus reviews ◄
       │
       ▼
File tasks (TaskCreate)
       │
       ▼
ExitPlanMode()
```

---

## References

For detailed workflow documentation, see:
- [Domain Questions](../skills/atlas/references/domain-questions.md)
- [Atlas SKILL.md](../skills/atlas/SKILL.md)
- [Prometheus Workflow](../skills/atlas/references/workflows/prometheus.md)
- [Execution Workflow](../skills/atlas/references/workflows/execution.md)
