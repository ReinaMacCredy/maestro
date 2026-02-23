---
name: learner
description: "Extracts reusable principles from the current session and stores them as learned skills. Use when you want to capture durable patterns after completing work."
argument-hint: "[--from-session | --from-diff | <topic>]"
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, AskUserQuestion
disable-model-invocation: true
---

# Learner — Extract Hard-Won Principles

> Capture non-obvious, context-specific insights from your current work and save them as reusable skill files that Maestro can inject into future sessions.

## Arguments

- `--from-session` — Analyze the current session for learnings (default)
- `--from-diff` — Analyze recent git diff for patterns
- `<topic>` — Extract learnings about a specific topic

## Hard Rules

- **Quality over quantity**: One excellent principle beats ten mediocre ones.
- **No snippets**: Capture the *why* and *when*, not code blocks. If code is needed, it's a pattern, not a learning.
- **Verify uniqueness**: Check existing `.claude/skills/learned/` before saving duplicates.

## Quality Gates

Every extracted principle MUST pass ALL four gates:

| Gate | Question | Fail Example |
|------|----------|--------------|
| **Non-Googleable** | Would a developer find this in docs/Stack Overflow? | "Use `async/await` for promises" |
| **Context-Specific** | Is this specific to THIS project or domain? | "Always validate input" (too generic) |
| **Actionable** | Can someone act on this immediately? | "The system is complex" (observation, not action) |
| **Hard-Won** | Did this cost time/debugging to discover? | "TypeScript has interfaces" (trivial) |

## Workflow

### Step 1: Gather Evidence

Based on the argument:

**`--from-session`** (default):
1. Review conversation history for: corrections, debugging sessions, unexpected behavior, workarounds
2. Look for moments where assumptions were wrong

**`--from-diff`**:
1. Run `git diff HEAD~5..HEAD` (or appropriate range)
2. Identify non-obvious patterns in the changes
3. Look for: error handling patterns, API usage that wasn't obvious, config gotchas

**`<topic>`**:
1. Search codebase for topic-related files
2. Identify conventions and patterns
3. Note gotchas and edge cases

### Step 2: Extract Principles

For each candidate learning:
1. Apply all four quality gates
2. If any gate fails, discard or refine
3. Format as a principle with trigger conditions

### Step 3: Present for Approval

Show extracted principles to user:

```
AskUserQuestion(
  questions: [{
    question: "Save these learnings?",
    header: "Learnings",
    options: [
      { label: "Save all", description: "Save all extracted principles" },
      { label: "Select individually", description: "Choose which to save" },
      { label: "Discard", description: "Don't save any" }
    ],
    multiSelect: false
  }]
)
```

### Step 4: Save

For each approved principle, create a skill file:

**File**: `.claude/skills/learned/{slug}.md`

```markdown
---
name: {slug}

triggers:
  - {keyword1}
  - {keyword2}
priority: 200
---

# {Title}

## When This Applies
{Trigger conditions — when should this knowledge be surfaced?}

## The Principle
{The actual learning — what to do and why}

## Why This Matters
{What goes wrong without this knowledge}

## Evidence
{Where this was discovered — file paths, error messages, debugging session}
```

**Slug derivation**: Lowercase, hyphenated, max 5 words from the principle title.

### Step 5: Confirm

Report what was saved:
- Number of principles extracted vs. saved
- File paths created
- Trigger keywords that will surface these in future sessions

## Storage

- **Location**: `.claude/skills/learned/*.md`
- **Discovery**: Picked up automatically by Maestro's skill registry (session-start.sh parses `.claude/skills/*/SKILL.md`)
- **Priority**: 200 (lower than project skills at 100, so they supplement rather than override)

## Anti-Patterns

| Don't | Do Instead |
|-------|-----------|
| Save code snippets | Save the principle behind the pattern |
| Save generic advice | Save project-specific insights |
| Save obvious things | Save things that took debugging to learn |
| Save without triggers | Always include trigger keywords for discoverability |
