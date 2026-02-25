---
name: analyze
description: "Primary deep read-only investigation workflow. Use this first to understand a problem before any implementation work."
argument-hint: "<problem or topic>"
allowed-tools: Read, Grep, Glob, Bash, Task, TeamCreate, TeamDelete, SendMessage, AskUserQuestion
disable-model-invocation: true
---

# Analyze — Primary Investigation Mode

> Investigate a problem or topic thoroughly. Gather context, analyze patterns, synthesize actionable recommendations — without making code changes.

## Arguments

`<problem or topic>` — A description of what to investigate.

## Hard Rules

- **Read-only**: You MUST NOT use Write, Edit, or NotebookEdit tools. Investigation only.
- **Evidence-based**: Every finding must reference specific files and line numbers.
- **Structured output**: Always produce the standard report format below.
- **Primary flow**: Use this skill as the default investigation path. Use `research` only when multi-source follow-up is explicitly needed.

## Tooling Compatibility

- Prefer parallel workers when the runtime supports team orchestration APIs (`TeamCreate`, `SendMessage`, `Task`, `TeamDelete`).
- If team APIs are unavailable (for example in Amp-only execution), run the same workflow with available parallel mechanisms (such as thread handoffs, parallel searches, and direct tool calls).
- Do not assume APIs like `spawn_agent`, `send_input`, or `request_user_input` unless the host explicitly provides them.

## Workflow

### Step 1: Scope

Parse the user's description. Identify:
- Core question or problem
- Relevant domains (files, modules, systems)
- What "answered" looks like (exit criteria)

### Step 2: Gather Context

Run targeted discovery in parallel where possible.

If team APIs are available, create a team for parallel investigation:

```
TeamCreate(team_name: "analyze-{topic-slug}", description: "Investigating {topic}")
```

Assign workers/tasks:
- `explore` — codebase search for relevant files, patterns, dependencies
- `oracle` — strategic analysis once explore provides findings (only for complex topics)

Targeted discovery checklist:
1. Find files related to the topic
2. Trace data flow or call chains
3. Identify patterns and conventions
4. Collect concrete evidence snippets (`file:line`)

### Step 3: Analyze

Synthesize findings from workers. Look for:
- Root causes (not just symptoms)
- Patterns and anti-patterns
- Dependencies and coupling
- Risk areas
- Missing coverage (tests, docs, error handling)

### Step 4: Validate Confidence

- Verify that each key finding is backed by at least one direct source.
- Flag assumptions separately from verified facts.
- If evidence is thin or conflicting, record this explicitly in open questions.

### Step 5: Report

Output a structured report:

```markdown
## Summary
[2-3 sentence overview of findings]

## Key Findings
1. **[Finding]**: [Evidence with file:line references]
2. ...

## Analysis
[Detailed breakdown organized by theme/area]

## Open Questions
[Unknowns, conflicting evidence, or assumptions that need follow-up]

## Recommendations
1. **[Action]** — [Why, effort estimate, risk level]
2. ...

## Files Examined
- `path/to/file.ts:42` — [what was found]
```

### Step 6: Cleanup

If you created a team:

```
TeamDelete(reason: "Analysis complete")
```

**TeamDelete cleanup**: If TeamDelete fails, fall back to: `rm -rf ~/.claude/teams/{team-name} ~/.claude/tasks/{team-name}`

## When to Use

- Debugging a problem before fixing it
- Understanding unfamiliar code areas
- Evaluating architectural decisions
- Pre-refactoring analysis
- Incident investigation
- Producing the baseline investigation that `research` may extend

## When NOT to Use

- If you already know what to fix → use `/work` instead
- If you need to make changes → this is read-only
- For simple "where is X?" questions → just use Grep/Glob directly
