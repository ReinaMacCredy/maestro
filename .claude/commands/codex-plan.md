---
description: Create a detailed implementation plan using Codex 5.2 with xhigh reasoning
argument-hint: "<what you want to plan>"
allowed-tools: Read, Write, Bash, AskUserQuestion, TaskOutput
---

# Codex Plan Command

You are being asked to create a detailed implementation plan using Codex. Your job is to:
1. Understand the user's planning request
2. Ask clarifying questions using AskUserQuestion to improve plan quality
3. Craft an excellent, detailed prompt for Codex using GPT-5.2 best practices
4. Execute Codex to generate and save the plan

**Always uses:** `gpt-5.2-codex` with `xhigh` reasoning

## User Request

```
$ARGUMENTS
```

## Step 1: Analyze the Request

Look at what the user wants to plan. Identify:
- What is the core goal?
- What technology/domain is involved?
- What aspects are ambiguous or underspecified?
- What decisions would significantly impact the plan?

## Step 2: Ask Clarifying Questions

**Use AskUserQuestion to ask 2-4 targeted clarifying questions** before generating the plan.

Good clarifying questions:
- Narrow down scope and requirements
- Clarify technology choices
- Understand constraints (time, budget, team size)
- Identify must-haves vs nice-to-haves
- Uncover integration requirements
- Determine security/compliance needs

### Example Question Patterns

**For "implement auth":**
- What authentication methods do you need? (email/password, OAuth providers like Google/GitHub, SSO, magic links)
- Do you need role-based access control (RBAC) or just authenticated/unauthenticated?
- What's your backend stack? (Node/Express, Python/Django, etc.)
- Where will you store user credentials/sessions? (Database, Redis, JWT stateless)
- Do you need features like: password reset, email verification, 2FA?
- Any compliance requirements? (SOC2, GDPR, HIPAA)

**For "build an API":**
- What resources/entities does this API need to manage?
- REST or GraphQL?
- What authentication will the API use?
- Expected scale/traffic?
- Do you need rate limiting, caching, versioning?

**For "migrate to microservices":**
- Which parts of the monolith are you migrating first?
- What's your deployment target? (K8s, ECS, etc.)
- How will services communicate? (REST, gRPC, message queues)
- What's your timeline and team capacity?

**For "add testing":**
- What testing levels do you need? (unit, integration, e2e)
- What's your current test coverage?
- What frameworks do you prefer or already use?
- What's the most critical functionality to test first?

## Step 3: Gather Context

After getting answers, also gather relevant context:
- Read key files in the codebase if applicable
- Check existing architecture/patterns
- Note any existing plans or documentation

## Step 4: Craft the Codex Prompt with GPT-5.2 Best Practices

Create a detailed prompt that includes GPT-5.2 specific constraint blocks. The prompt structure should be:

### Required XML Constraint Blocks for GPT-5.2

```markdown
# Plan: [Task Name]

## Planning Request

[Clear objective and all requirements learned from clarifying questions]

## Codebase Context

[Relevant codebase info, existing patterns, file paths]

<output_verbosity_spec>
- Default: Concise task descriptions (2-3 sentences per task).
- For complex multi-step tasks:
  - 1 short overview paragraph
  - then ≤5 bullets per task: What, Where, Dependencies, Test-First, Acceptance Criteria.
- Avoid long narrative paragraphs; prefer compact bullets and short sections.
- Do not rephrase the user's request unless it changes semantics.
- Each task must have: Location, Description, Dependencies, Complexity (1-10), Test-First, Acceptance Criteria.
</output_verbosity_spec>

<design_and_scope_constraints>
- Implement EXACTLY and ONLY what is requested in this planning prompt.
- No extra features, no added components, no scope creep.
- Do NOT invent additional requirements beyond what is specified.
- If any instruction is ambiguous, choose the simplest valid interpretation.
- Each task should be atomic and independently verifiable.
- Prefer small, focused tasks over large multi-concern tasks.
</design_and_scope_constraints>

<long_context_handling>
- For inputs longer than ~10k tokens:
  - First, produce a short internal outline of key sections relevant to the request.
  - Re-state the user's constraints explicitly before generating tasks.
  - Anchor tasks to specific files and sections rather than speaking generically.
- When the task depends on fine details (paths, configs, existing code), reference them explicitly.
</long_context_handling>

<uncertainty_and_ambiguity>
- Do NOT ask clarifying questions - you have all needed information.
- If a requirement is ambiguous, choose the simplest valid interpretation and document the assumption.
- Never fabricate exact file paths or line numbers when uncertain - use patterns like "src/**/auth*.ts".
- Prefer language like "Based on the provided context..." for assumptions.
</uncertainty_and_ambiguity>

<high_risk_self_check>
Before finalizing the plan:
- Briefly re-scan for unstated assumptions.
- Verify all file paths referenced actually exist in context.
- Check for overly strong claims ("always", "guaranteed") and soften if needed.
- Ensure each task has clear, verifiable acceptance criteria.
</high_risk_self_check>

<tool_usage_rules>
- For implementation tasks, specify which tools the executor should use.
- Parallelize independent tasks when possible - mark with "Parallelizable: YES".
- Require verification steps for high-impact operations (database changes, API modifications).
- After any destructive operation, include rollback instructions.
</tool_usage_rules>

<agentic_steerability>
- Each phase should have a clear goal statement.
- Tasks within a phase should be ordered by dependency, not arbitrary sequence.
- Mark blocking dependencies explicitly: "Blocked by: Task X.Y".
- Include "Next steps" at the end of each phase.
</agentic_steerability>

## Plan Structure

Use this exact template:

# Plan: [Task Name]

**Generated**: [Date]
**Estimated Complexity**: [Low/Medium/High]
**Reasoning Effort**: xhigh

## Overview
[1 paragraph: what this plan accomplishes and high-level approach]

## Prerequisites
- [Dependencies that must be in place before starting]

## Phase 1: [Phase Name]
**Goal**: [What this phase accomplishes - 1 sentence]

### Task 1.1: [Task Name]
- **Location**: [Specific file paths]
- **Description**: [What needs to be done - 2-3 sentences max]
- **Dependencies**: [Task IDs or "None"]
- **Parallelizable**: [YES/NO]
- **Complexity**: [1-10]
- **Test-First**: [Test to write before implementation]
- **Acceptance Criteria**: [Bullet list of verifiable conditions]

[Continue with all tasks...]

## Testing Strategy
- **Unit Tests**: [Specific test patterns]
- **Integration Tests**: [How to test component interactions]
- **E2E Tests**: [If applicable, user flow tests]

## Dependency Graph
[Show parallel vs sequential execution paths]
```
Phase 1: Task 1.1 ──┬── Task 1.2 (parallel)
                   └── Task 1.3 (parallel)
                          │
Phase 2: Task 2.1 ────────┘ (sequential, depends on 1.2, 1.3)
```

## Potential Risks
| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| [Risk] | [Low/Med/High] | [Low/Med/High] | [Strategy] |

## Rollback Plan
[How to undo changes if something goes wrong]

## TODOs
[Checklist format for tracking - used by beads integration]
- [ ] 1. [Task 1.1 title] **Complexity**: X **Location**: path **Dependencies**: None **Parallelizable**: YES
- [ ] 2. [Task 1.2 title] **Complexity**: X **Location**: path **Dependencies**: 1 **Parallelizable**: NO

## Instructions

- Write the complete plan to `.atlas/codex/results/{plan-name}.md`
- Do NOT ask any clarifying questions - you have all the information needed
- Be specific and actionable with file paths
- Each task should be atomic and independently verifiable
- Follow test-driven development principles
- Include complexity scores for prioritization
- Mark parallelizable tasks explicitly
- Just write the plan and save the file

Begin immediately.
```

### Complete Prompt Template

Here is the full template to use when crafting prompts for Codex:

```markdown
# Codex Planning Request: {Plan Title}

{Clear description of what needs to be planned}

## Requirements

{All requirements gathered from clarifying questions, formatted as bullet points}

## Constraints

{Technology choices, timeline, team size, compliance requirements}

## Codebase Context

{Relevant patterns, existing architecture, key file paths discovered during context gathering}

<output_verbosity_spec>
- Default: Concise task descriptions (2-3 sentences per task).
- For complex multi-step tasks:
  - 1 short overview paragraph
  - then ≤5 bullets per task: What, Where, Dependencies, Test-First, Acceptance Criteria.
- Avoid long narrative paragraphs; prefer compact bullets and short sections.
- Do not rephrase the user's request unless it changes semantics.
- Each task must have: Location, Description, Dependencies, Complexity (1-10), Test-First, Acceptance Criteria.
</output_verbosity_spec>

<design_and_scope_constraints>
- Implement EXACTLY and ONLY what is requested in this planning prompt.
- No extra features, no added components, no scope creep.
- Do NOT invent additional requirements beyond what is specified.
- If any instruction is ambiguous, choose the simplest valid interpretation.
- Each task should be atomic and independently verifiable.
- Prefer small, focused tasks over large multi-concern tasks.
</design_and_scope_constraints>

<long_context_handling>
- For inputs longer than ~10k tokens:
  - First, produce a short internal outline of key sections relevant to the request.
  - Re-state the user's constraints explicitly before generating tasks.
  - Anchor tasks to specific files and sections rather than speaking generically.
- When the task depends on fine details (paths, configs, existing code), reference them explicitly.
</long_context_handling>

<uncertainty_and_ambiguity>
- Do NOT ask clarifying questions - you have all needed information.
- If a requirement is ambiguous, choose the simplest valid interpretation and document the assumption.
- Never fabricate exact file paths or line numbers when uncertain - use patterns like "src/**/auth*.ts".
- Prefer language like "Based on the provided context..." for assumptions.
</uncertainty_and_ambiguity>

<high_risk_self_check>
Before finalizing the plan:
- Briefly re-scan for unstated assumptions.
- Verify all file paths referenced actually exist in context.
- Check for overly strong claims ("always", "guaranteed") and soften if needed.
- Ensure each task has clear, verifiable acceptance criteria.
</high_risk_self_check>

<tool_usage_rules>
- For implementation tasks, specify which tools the executor should use.
- Parallelize independent tasks when possible - mark with "Parallelizable: YES".
- Require verification steps for high-impact operations (database changes, API modifications).
- After any destructive operation, include rollback instructions.
</tool_usage_rules>

<agentic_steerability>
- Each phase should have a clear goal statement.
- Tasks within a phase should be ordered by dependency, not arbitrary sequence.
- Mark blocking dependencies explicitly: "Blocked by: Task X.Y".
- Include "Next steps" at the end of each phase.
</agentic_steerability>

## Plan Structure Required

[Include the full plan template from above]

## Instructions

- Write the complete plan to `.atlas/codex/results/{plan-name}.md`
- Do NOT ask any clarifying questions - you have all the information needed
- Be specific and actionable with file paths
- Each task should be atomic and independently verifiable
- Follow test-driven development principles
- Include complexity scores for prioritization
- Mark parallelizable tasks explicitly
- Just write the plan and save the file

Begin immediately.
```

## Step 5: Execute Codex (Background + Polling, No Hard Timeout)

1. Write your crafted prompt to `.atlas/codex/handoffs/{plan-name}.md`

2. **Launch Codex in background** (returns immediately with task ID):

```
Bash({
  command: "./scripts/codex-cli.sh .atlas/codex/handoffs/{plan-name}.md {plan-name}",
  run_in_background: true,
  description: "Run Codex CLI in background"
})
```

This returns a task ID like `b3f1274` and an output file path.

3. **Poll for completion using TaskOutput**:

```
TaskOutput({
  task_id: "<task-id-from-step-2>",
  block: true
})
```

**Polling Pattern** - TaskOutput has ~10 min system limit. If status is `timeout` or `running`, call TaskOutput again. Background task keeps running.

4. **Check for success** in the TaskOutput result:
   - Look for `status: completed` and `exit_code: 0`
   - Look for `[SUCCESS] Plan:` in the output

5. **Read the generated plan**:
```
Read({ file_path: ".atlas/codex/results/{plan-name}.md" })
```

**Why polling?**
- TaskOutput has ~10 min system limit (not configurable)
- Background Codex keeps running regardless of timeout
- If timeout occurs, just call TaskOutput again
- Codex with xhigh reasoning may take 10-15 min

The script handles model selection (gpt-5.2-codex) and reasoning effort (xhigh) automatically.

## Example Full Flow

**User:** `/codex-plan help me implement auth into my webapp`

**You ask (using AskUserQuestion):**

Questions about auth methods, RBAC needs, backend stack, session storage, feature requirements, compliance.

**User answers:** "Email/password and Google OAuth. Using Next.js with Prisma and Postgres. Need basic roles (admin/user). Yes to password reset and email verification. No 2FA for now. No special compliance."

**You craft prompt for Codex:**

```markdown
# Codex Planning Request: Next.js Authentication System

Create a detailed implementation plan for adding authentication to a Next.js web application.

## Requirements

- Authentication methods: Email/password + Google OAuth
- Framework: Next.js (App Router)
- Database: PostgreSQL with Prisma ORM
- Role-based access: Admin and User roles
- Features needed:
  - User registration and login
  - Password reset flow
  - Email verification
  - Google OAuth integration
  - Session management
- NOT needed: 2FA, SSO, special compliance

## Codebase Context

[Include relevant file paths and patterns discovered during context gathering]

<output_verbosity_spec>
- Default: Concise task descriptions (2-3 sentences per task).
- For complex multi-step tasks:
  - 1 short overview paragraph
  - then ≤5 bullets per task: What, Where, Dependencies, Test-First, Acceptance Criteria.
- Avoid long narrative paragraphs; prefer compact bullets and short sections.
- Do not rephrase the user's request unless it changes semantics.
</output_verbosity_spec>

<design_and_scope_constraints>
- Implement EXACTLY and ONLY what is requested: email/password + Google OAuth.
- No extra features, no added components, no scope creep.
- Do NOT add 2FA, SSO, or other auth methods not specified.
- If any instruction is ambiguous, choose the simplest valid interpretation.
</design_and_scope_constraints>

<uncertainty_and_ambiguity>
- Do NOT ask clarifying questions - you have all needed information.
- If a requirement is ambiguous, choose the simplest valid interpretation and document the assumption.
- Never fabricate exact file paths when uncertain.
</uncertainty_and_ambiguity>

<high_risk_self_check>
Before finalizing the plan:
- Verify all Prisma schema changes are reversible.
- Ensure password hashing uses bcrypt or argon2.
- Check OAuth callback URLs are configurable per environment.
</high_risk_self_check>

## Plan Structure Required

[Full template...]

## Instructions

- Write the complete plan to `.atlas/codex/results/nextjs-auth.md`
- Do NOT ask any clarifying questions
- Be specific and actionable
- Follow test-driven development
- Just write the plan and save the file

Begin immediately.
```

**Execute and return results.**

## After Codex Returns

**YOU MUST DO THIS - DO NOT SKIP:**

1. Read the generated plan from `.atlas/codex/results/{plan-name}.md`
2. Show a brief summary (3-5 bullet points of what the plan covers)
3. **IMMEDIATELY use AskUserQuestion** to offer next steps:

```
AskUserQuestion({
  questions: [{
    question: "Codex has generated your plan. How would you like to proceed?",
    header: "Next step",
    options: [
      { label: "Start Work", description: "Copy to .claude/plans/ and execute immediately" },
      { label: "High Accuracy Review", description: "Run Metis gap analysis then Momus review loop" }
    ],
    multiSelect: false
  }]
})
```

### User Choice Handling

**If "Start Work":**
1. Copy the plan to `.claude/plans/{plan-name}.md`
2. Tell user: "Run `/start-work` to begin execution"

**If "High Accuracy Review":**

This is a combined Metis → Momus pipeline:

**Step 1: Copy plan to .claude/plans/**
```bash
cp .atlas/codex/results/{plan-name}.md .claude/plans/{plan-name}.md
```

**Step 2: Metis Gap Analysis**
```
Task({
  subagent_type: "metis",
  prompt: `Analyze this plan for hidden requirements, ambiguities, and potential AI failure modes.

Plan file: .claude/plans/${plan_name}.md

End with JSON:
\`\`\`json
{"status": "GAP_ANALYSIS_COMPLETE", "critical_questions": [...], "must_do": [...], "must_not_do": [...]}
\`\`\`
`
})
```

- If Metis finds critical questions, present them to user
- If user provides answers, update the plan accordingly
- Then proceed to Momus

**Step 3: Momus Review Loop**
```
Task({
  subagent_type: "momus",
  prompt: `Review this plan for gaps, ambiguities, and missing context.

Plan file: .claude/plans/${plan_name}.md

End with JSON verdict:
\`\`\`json
{"verdict": "OKAY"} or {"verdict": "REJECT", "issues": [...]}
\`\`\`
`
})
```

- **If Momus returns OKAY**: Plan approved, suggest `/start-work`
- **If Momus returns REJECT**: Re-spawn Prometheus with fixes, then Momus again
- Loop until Momus returns OKAY (max 5 iterations)

## GPT-5.2 Prompting Reference

### Key Behavioral Differences (vs GPT-5/5.1)

- **More deliberate scaffolding**: Builds clearer plans by default
- **Lower verbosity**: More concise, task-focused output
- **Stronger instruction adherence**: Less drift from user intent
- **Conservative grounding bias**: Favors correctness and explicit reasoning

### Required Constraint Blocks

| Block | Purpose |
|-------|---------|
| `<output_verbosity_spec>` | Control output length and format |
| `<design_and_scope_constraints>` | Prevent scope creep |
| `<long_context_handling>` | Handle large inputs |
| `<uncertainty_and_ambiguity>` | Manage unclear requirements |
| `<high_risk_self_check>` | Self-verification before output |
| `<tool_usage_rules>` | Guide tool selection and parallelism |
| `<agentic_steerability>` | Control execution flow |

### Anti-Patterns to Avoid

- Vague verbosity expectations (always specify length)
- Implicit scope boundaries (explicitly forbid extras)
- Generic context handling (force section anchoring)
- Overconfident responses (add uncertainty handling)
- Serial tool calls when parallel possible
- Changing prompts and models simultaneously

## Important Notes

- **Always ask clarifying questions first** - Don't skip this step
- **Use AskUserQuestion tool** - This is interactive planning
- **Always use gpt-5.2-codex with xhigh reasoning** - No exceptions
- **Include all XML constraint blocks** - GPT-5.2 requires explicit boundaries
- **Tell Codex not to ask questions** - It should just execute
- **Output file:** `.atlas/codex/results/{plan-name}.md`
- **Use --full-auto** - No human approval needed

### Clarifying Questions: Operator vs Codex

**Important distinction:**

| Actor | Should Ask Questions? | Reason |
|-------|----------------------|--------|
| **You (Operator)** | YES - Step 2 | Gather requirements before crafting prompt |
| **Codex (Generator)** | NO - In handoff prompt | All info provided in handoff package |

The `<uncertainty_and_ambiguity>` block tells Codex "Do NOT ask clarifying questions" because:
1. You already gathered all requirements in Step 2
2. The handoff package contains complete context
3. Codex should execute immediately, not interview

This is not a contradiction - it's a separation of concerns between interview phase (you) and generation phase (Codex).

Now analyze the user's planning request above, ask your clarifying questions, and then generate and execute the Codex plan.
