---
name: research
description: "Conducts multi-stage research with parallel agents, verification, and synthesis. Use when you need high-confidence findings for complex questions."
argument-hint: "<topic> [--auto|--stages <N>|--resume]"
allowed-tools: Read, Write, Edit, Bash, Grep, Glob, Task, TeamCreate, TeamDelete, SendMessage, WebSearch, WebFetch, AskUserQuestion
disable-model-invocation: true
---

# Research — Multi-Stage Deep Investigation

> Structured research across codebase, documentation, and web sources. Breaks complex topics into stages, runs parallel agents, verifies findings, and synthesizes a final report.

## Arguments

- `<topic>` — What to research (required)
- `--auto` — Run all stages automatically without user confirmation between stages
- `--stages <N>` — Override number of stages (default: auto-determined, range: 3-7)
- `--resume` — Resume an interrupted research session from `.maestro/research/`

## Guardrails

```
max_stages: 7
max_iterations: 10
max_concurrency: 5
```

These limits are hard — no override. They prevent runaway research sessions.

## Workflow

### Step 1: Decompose

Break the research topic into 3-7 stages:

```markdown
## Research Plan: {topic}

1. **{Stage Name}** — {What to investigate}
   - Sources: [codebase|docs|web|all]
   - Key questions: [specific questions to answer]
2. ...
```

**If NOT `--auto`**: Present plan to user for approval.

**If `--auto`**: Log the plan and proceed immediately.

### Step 2: Initialize Session

Create session state:

```json
// .maestro/research/{topic-slug}.json
{
  "topic": "{topic}",
  "status": "active",
  "mode": "auto|interactive",
  "stages": [
    {
      "name": "{stage name}",
      "status": "pending",
      "sources": ["codebase"],
      "questions": ["..."],
      "findings": [],
      "agents_used": 0
    }
  ],
  "iteration": 1,
  "max_iterations": 10,
  "started": "{ISO timestamp}",
  "report_path": null
}
```

### Step 3: Execute Stages

Create a team:

```
TeamCreate(team_name: "research-{topic-slug}", description: "Researching {topic}")
```

For each stage, spawn up to `max_concurrency` agents in parallel:

| Source | Agent | Task |
|--------|-------|------|
| Codebase | `explore` | Search files, patterns, dependencies |
| Strategic | `oracle` | Analyze findings, identify implications |
| Web | Use WebSearch/WebFetch directly | External documentation, articles, best practices |

**Stage execution:**
1. Spawn agents for current stage
2. Collect findings
3. Update session state
4. Verify findings against sources (cross-reference)
5. Move to next stage

**In AUTO mode:**
- No user prompts between stages
- Log progress to session state
- Track iteration count — stop at `max_iterations`

**In interactive mode:**
- After each stage, present findings and ask:
  ```
  AskUserQuestion(
    questions: [{
      question: "Stage {N} complete. How to proceed?",
      header: "Research",
      options: [
        { label: "Continue", description: "Proceed to next stage" },
        { label: "Deep dive", description: "Add a sub-stage to explore a finding further" },
        { label: "Skip ahead", description: "Jump to synthesis" },
        { label: "Stop", description: "End research here" }
      ],
      multiSelect: false
    }]
  )
  ```

### Step 4: Verify

Cross-reference findings:
- Do codebase findings match documentation?
- Do multiple sources agree?
- Are there contradictions to flag?

Mark each finding as: `verified`, `unverified`, or `contradicted`.

### Step 5: Synthesize

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
  - Verified: [yes|no|contradicted]

### Stage 2: {name}
...

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
- Iterations: {N}
- Duration: {time}
- Mode: {auto|interactive}
```

Save report to `.maestro/research/{topic-slug}-report.md`.

### Step 6: Cleanup

```
TeamDelete(reason: "Research session complete")
```

**TeamDelete cleanup**: If TeamDelete fails, fall back to: `rm -rf ~/.claude/teams/{team-name} ~/.claude/tasks/{team-name}`

Update session state:
```json
{
  "status": "completed",
  "report_path": ".maestro/research/{topic-slug}-report.md",
  "completed": "{ISO timestamp}"
}
```

## Resume Support

When `--resume` is used:
1. Find the most recent `.maestro/research/*.json` with `status: "active"`
2. Load the session state
3. Skip completed stages
4. Resume from the first `pending` stage

## Session State Schema

```json
{
  "topic": "string",
  "status": "active | completed | failed",
  "mode": "auto | interactive",
  "stages": [{
    "name": "string",
    "status": "pending | in_progress | completed | skipped",
    "sources": ["codebase", "web", "docs"],
    "questions": ["string"],
    "findings": [{
      "description": "string",
      "source": "string",
      "confidence": "high | medium | low",
      "verified": "boolean | null"
    }],
    "agents_used": "number"
  }],
  "iteration": "number",
  "max_iterations": 10,
  "started": "ISO timestamp",
  "completed": "ISO timestamp | null",
  "report_path": "string | null"
}
```

## Anti-Patterns

| Don't | Do Instead |
|-------|-----------|
| Research without a plan | Always decompose into stages first |
| Spawn more than 5 agents | Respect max_concurrency limit |
| Run more than 10 iterations | Stop and synthesize what you have |
| Skip verification | Always cross-reference findings |
| Produce findings without sources | Every finding needs a source reference |
