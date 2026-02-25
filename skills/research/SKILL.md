---
name: research
description: "Supporting follow-up mode for multi-source validation after analyze. Use to extend existing findings with docs/web cross-checks."
argument-hint: "<topic> [--auto|--stages <N>]"
allowed-tools: Read, Write, Bash, Grep, Glob, Task, TeamCreate, TeamDelete, SendMessage, WebSearch, WebFetch, AskUserQuestion
disable-model-invocation: true
---

# Research — Follow-Up Multi-Source Mode

> Extend an existing `analyze` investigation with focused multi-source validation (docs/web/external references). This is a supporting mode, not the primary investigation workflow.

## Arguments

- `<topic>` — What to research (required)
- `--auto` — Run all stages automatically without user confirmation between stages
- `--stages <N>` — Override number of stages (default: 2-4, range: 2-5)

## Guardrails

```
max_stages: 5
max_concurrency: 5
```

These limits are hard — no override. They prevent runaway research sessions.

## Role Boundary

- Start with `analyze` for core codebase investigation and root-cause analysis.
- Use `research` only after `analyze` has produced baseline findings or open questions.
- Avoid duplicating `analyze` steps (full call-chain tracing, broad codebase mapping) unless needed to verify a specific contradiction.

## Tooling Compatibility

- Prefer team orchestration APIs when available (`TeamCreate`, `SendMessage`, `Task`, `TeamDelete`).
- If team APIs are unavailable, execute the same stages directly with available tools.
- Do not assume APIs such as `spawn_agent`, `send_input`, or `request_user_input` unless explicitly provided by the host runtime.

## Workflow

### Step 1: Ingest Baseline

Capture the relevant output from `analyze` before doing new work:

1. Baseline findings (with `file:line` evidence)
2. Open questions requiring external confirmation
3. Assumptions that need verification

If no prior `analyze` output exists, run `analyze` first.

### Step 2: Plan Follow-Up Stages

Break follow-up work into 2-5 narrowly scoped stages:

```markdown
## Research Plan: {topic}

1. **{Stage Name}** — {What to investigate}
   - Sources: [codebase|docs|web|all]
   - Linked analyze finding/open question: [reference]
   - Key validation question: [specific question to answer]
2. ...
```

**If NOT `--auto`**: Present plan to user for approval.

**If `--auto`**: Log the plan and proceed immediately.

### Step 3: Initialize Session

Create a lightweight session note only:

```json
// .maestro/research/{topic-slug}.json
{
  "topic": "{topic}",
  "status": "active",
  "mode": "auto|interactive",
  "based_on": "analyze:{topic-or-thread-reference}",
  "stages": ["{stage names}"],
  "started": "{ISO timestamp}"
}
```

### Step 4: Execute Validation Stages

Create a team:

```
TeamCreate(team_name: "research-{topic-slug}", description: "Researching {topic}")
```

For each stage, spawn up to `max_concurrency` agents in parallel:

| Source | Agent | Task |
|--------|-------|------|
| Codebase | `explore` | Verify/contradict specific baseline findings |
| Strategic | `oracle` | Evaluate implications of confirmations/contradictions |
| Web | Use WebSearch/WebFetch directly | External documentation, articles, best practices |

**Stage execution:**
1. Spawn agents for current stage
2. Collect findings
3. Update session state
4. Cross-check against baseline `analyze` findings
5. Move to next stage

**In AUTO mode:**
- No user prompts between stages
- Continue through planned stages

**In interactive mode:**
- After each stage, present findings and ask:
  ```
  AskUserQuestion(
    questions: [{
      question: "Stage {N} complete. How to proceed?",
      header: "Research",
      options: [
        { label: "Continue", description: "Proceed to next stage" },
        { label: "Deep dive", description: "Add one focused validation sub-stage" },
        { label: "Skip ahead", description: "Jump to synthesis" },
        { label: "Stop", description: "End research here" }
      ],
      multiSelect: false
    }]
  )
  ```

### Step 5: Verify

Cross-reference findings:
- Do `analyze` findings match docs/web evidence?
- Do multiple sources agree on disputed points?
- Which baseline assumptions are now confirmed vs contradicted?

Mark each finding as: `verified`, `unverified`, or `contradicted`.

### Step 6: Synthesize

Produce a final report:

```markdown
# Research Report: {topic}

## Executive Summary
[2-3 paragraphs covering key findings]

## Findings by Stage

### Stage 1: {name}
- **Finding**: [description]
  - Source: [file:line | URL | agent analysis]
  - Confidence: [high|medium|low]
  - Verified: [verified|unverified|contradicted]

### Stage 2: {name}
...

## Delta From Analyze
- Confirmed findings:
- Contradicted findings:
- Newly discovered findings:

## Cross-References
[Where multiple stages produced related findings]

## Open Questions
[What wasn't answered — potential follow-up research]

## Recommendations
1. [Actionable recommendation based on findings]
2. ...

## Methodology
- Stages: {N}
- Agents used: {N}
- Duration: {time}
- Mode: {auto|interactive}
- Based on: {analyze reference}
```

Save report to `.maestro/research/{topic-slug}-report.md`.

### Step 7: Cleanup

```
TeamDelete(reason: "Research session complete")
```

**TeamDelete cleanup**: If TeamDelete fails, fall back to: `rm -rf ~/.claude/teams/{team-name} ~/.claude/tasks/{team-name}`

Update session state:
```json
{
  "status": "completed",
  "completed": "{ISO timestamp}"
}
```

## Anti-Patterns

| Don't | Do Instead |
|-------|-----------|
| Start in research without baseline analysis | Run `analyze` first and carry findings forward |
| Re-run full codebase investigation | Only validate specific open questions from `analyze` |
| Research without a plan | Decompose into 2-5 narrow validation stages |
| Spawn more than 5 agents | Respect max_concurrency limit |
| Skip verification | Always cross-reference findings |
| Produce findings without sources | Every finding needs a source reference |
