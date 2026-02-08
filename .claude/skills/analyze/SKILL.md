---
name: analyze
description: Deep investigation mode. Gather context, analyze, synthesize recommendations without making code changes.
argument-hint: "<problem or topic>"
allowed-tools: Read, Grep, Glob, Bash, Task, TeamCreate, TeamDelete, SendMessage, AskUserQuestion
disable-model-invocation: true
---

# Analyze — Deep Investigation Mode

> Investigate a problem or topic thoroughly. Gather context, analyze patterns, synthesize actionable recommendations — without making code changes.

## Arguments

`<problem or topic>` — A description of what to investigate.

## Hard Rules

- **Read-only**: You MUST NOT use Write, Edit, or NotebookEdit tools. Investigation only.
- **Evidence-based**: Every finding must reference specific files and line numbers.
- **Structured output**: Always produce the standard report format below.

## Workflow

### Step 1: Scope

Parse the user's description. Identify:
- Core question or problem
- Relevant domains (files, modules, systems)
- What "answered" looks like (exit criteria)

### Step 2: Gather Context

Create a team for parallel investigation:

```
TeamCreate(team_name: "analyze-{topic-slug}", description: "Investigating {topic}")
```

Spawn workers:
- `explore` — codebase search for relevant files, patterns, dependencies
- `oracle` — strategic analysis once explore provides findings (only for complex topics)

Assign targeted search tasks to explore:
1. Find files related to the topic
2. Trace data flow or call chains
3. Identify patterns and conventions

### Step 3: Analyze

Synthesize findings from workers. Look for:
- Root causes (not just symptoms)
- Patterns and anti-patterns
- Dependencies and coupling
- Risk areas
- Missing coverage (tests, docs, error handling)

### Step 4: Report

Output a structured report:

```markdown
## Summary
[2-3 sentence overview of findings]

## Key Findings
1. **[Finding]**: [Evidence with file:line references]
2. ...

## Analysis
[Detailed breakdown organized by theme/area]

## Recommendations
1. **[Action]** — [Why, effort estimate, risk level]
2. ...

## Files Examined
- `path/to/file.ts:42` — [what was found]
```

### Step 5: Cleanup

```
TeamDelete()
```

## When to Use

- Debugging a problem before fixing it
- Understanding unfamiliar code areas
- Evaluating architectural decisions
- Pre-refactoring analysis
- Incident investigation

## When NOT to Use

- If you already know what to fix → use `/work` instead
- If you need to make changes → this is read-only
- For simple "where is X?" questions → just use Grep/Glob directly
