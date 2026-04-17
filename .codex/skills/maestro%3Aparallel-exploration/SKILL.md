---
name: maestro:parallel-exploration
description: Use when you need parallel, read-only exploration with task() (Scout fan-out)
---

# Parallel Exploration (Scout Fan-Out)

## Overview

When you need to answer "where/how does X work?" across multiple domains (codebase, tests, docs, OSS), investigating sequentially wastes time. Each investigation is independent and can happen in parallel.

**Core principle:** Decompose into independent sub-questions, spawn one scout per sub-question, collect results, synthesize into a coherent picture.

**Safe in Planning mode:** This is read-only exploration. It is OK to use during exploratory research even when there is no feature, no plan, and no approved tasks.

**This skill is for read-only research.** For parallel implementation work, use `maestro skill maestro:dispatching` with `maestro task-start`.

## When to Use

**Use when:**
- Investigation spans 2+ domains (code + tests, code + docs, code + config)
- Questions are independent (answer to A doesn't affect B)
- No edits needed (read-only exploration)
- The exploration likely spans multiple files or packages

**Skip when:**
- It's a single focused question answerable with one grep + one file read
- Questions are dependent (answer A materially changes what to ask for B)
- Work involves file edits (use Hive tasks instead)

Thinking "this is simple, I'll just do it myself"? If the investigation touches 3+ domains, fan out. Sequential exploration in a parallel-capable system is waste.

## The Pattern

### 1. Decompose Into Independent Questions

Split your investigation into 2-4 independent sub-questions.

| Domain | Question shape |
|--------|---------------|
| Codebase | "Where is X implemented? What files define it?" |
| Tests | "How is X tested? What patterns exist?" |
| Docs/OSS | "How do other projects implement X?" |
| Config | "How is X configured? What env vars affect it?" |

See `reference/scout-patterns.md` for concrete fan-out strategies: codebase survey, dependency analysis, pattern search, API surface mapping.

### 2. Write Scout Prompts

Each scout prompt must be specific, bounded, and evidence-oriented.

<Good>
```
Find all call sites for `loadSkill()` in the codebase.
For each call site, return:
- File path and line number
- The calling function name
- What arguments are passed
- Whether the result is awaited or fire-and-forget

Return as a table. Do not read files outside src/.
```
Specific function. Defined output format. Bounded scope. Evidence-oriented.
</Good>

<Bad>
```
How does the skill system work? Look at everything related to skills
and give me a summary.
```
Unbounded scope. No output format. Will return a vague narrative instead of evidence.
</Bad>

<Good>
```
Find all error handling patterns in src/commands/.
For each try/catch block, return:
- File:line
- What error types are caught
- What action is taken (rethrow, log, swallow, wrap)
Return as a table.
```
Specific artifact (try/catch). Defined output columns. Bounded directory.
</Good>

<Bad>
```
Look at error handling in the codebase.
```
No artifact specified. No output format. No scope boundary.
</Bad>

**Scout prompt checklist:**
- [ ] Specific target (function, pattern, file type -- not "everything about X")
- [ ] Bounded scope (directory, file glob, or explicit exclusions)
- [ ] Defined output format (table, list with file:line, or explicit columns)
- [ ] Evidence-oriented ("return file paths with line numbers", not "summarize")

### 3. Decide How Many Scouts

| Situation | Scouts | Rationale |
|-----------|--------|-----------|
| Focused question, 2 clear domains | 2 | Minimum useful parallelism |
| Typical investigation | 3 | Sweet spot: structure + behavior + tests/config |
| Complex unfamiliar subsystem | 4 | Maximum before synthesis overhead dominates |
| 5+ scouts | **Stop. Reframe.** | Decompose into 2 sequential rounds of 2-3 each |

**The overlap test:** Write out each scout's expected output before launching. If two scouts would return the same files, merge them or redefine boundaries.

### 4. Fan Out (Spawn All Before Waiting)

Launch all scouts in the same assistant message. Do not wait for results between launches.

```typescript
// GOOD: All scouts launched in one message
task({
  subagent_type: 'scout-researcher',
  description: 'Map skill registry structure',
  prompt: `Map the file structure of src/skills/.
    For each file: purpose, exports, and which modules import it.
    Return as a table with columns: file, purpose, exports, importers.`,
});

task({
  subagent_type: 'scout-researcher',
  description: 'Trace skill loading data flow',
  prompt: `Trace data flow for skill loading: from CLI invocation
    through registry lookup to file read. List each function in the
    chain with file:line. Return as an ordered list.`,
});

task({
  subagent_type: 'scout-researcher',
  description: 'Audit skill configuration points',
  prompt: `Find all configuration that affects skill loading: env vars,
    config file keys, CLI flags, defaults. Return as a table with
    columns: source, key, default_value, effect.`,
});
```

```typescript
// BAD: Sequential -- defeats the purpose
const result1 = await task({ ... });  // Waits here
const result2 = await task({ ... });  // Then waits here
```

### 5. Continue Working (Optional)

While scouts run, you can:
- Prepare synthesis structure
- Draft based on what you already know
- Work on unrelated aspects

You'll receive a `<system-reminder>` notification when each scout completes.

### 6. Synthesize Findings

This is the hardest step. Raw scout results are not the answer.

**Synthesis process:**
1. **Deduplicate** -- List every file mentioned. Files cited by 2+ scouts are integration points.
2. **Resolve contradictions** -- Scouts disagree? One is wrong. Re-read the code yourself. Never average.
3. **Identify gaps** -- What wasn't found? Missing answers matter as much as found ones.
4. **Build narrative** -- Write an integrated story with evidence, not a paste of each scout's output.

<Good>
```
Skill loading is a 3-step pipeline:
1. CLI entry (cli.ts:45) parses the skill name from argv
2. Registry lookup (registry.ts:23) resolves name to path
   - Uses SKILL_PATH env var (default: ./skills/)
   - Falls back to built-in directory if not found
3. File parsing (loader.ts:67) reads YAML frontmatter + markdown body

Integration point: registry.ts -- both CLI and loader depend on it.
Gap: No tests found for the fallback path.
```
Cross-referenced. Integration points identified. Gaps noted.
</Good>

<Bad>
```
Scout 1 found: registry.ts, loader.ts, types.ts
Scout 2 found: cli.ts calls loadSkill(), loadSkill() calls parseFile()
Scout 3 found: SKILL_PATH env var, default is ./skills/
```
Dumped raw results. No integration. No gaps identified.
</Bad>

See `reference/synthesis-guide.md` for detailed patterns: resolving contradictions, prioritizing discoveries, gap analysis.

### 7. Verify Completeness

After synthesis, check:
- [ ] All scouts spawned before collecting any results (true fan-out)
- [ ] Scout prompts were specific and bounded (not "tell me about X")
- [ ] Contradictions resolved (not glossed over)
- [ ] Gaps identified and either investigated or noted
- [ ] Synthesis is a narrative with evidence, not a scout dump

## Diminishing Returns -- When to Stop Exploring

Exploration is seductive. You can always learn more. The goal is not complete knowledge -- it's sufficient knowledge to make design decisions.

### Stop signals

| Signal | What it means | Action |
|--------|--------------|--------|
| Scouts return the same files you already know about | You've mapped the domain | Stop. Start designing. |
| Follow-up scouts find edge cases, not new subsystems | Core structure is understood | Note edge cases. Start designing. |
| You're on round 3+ of scouts | Diminishing returns | Stop. What you don't know, you'll learn during implementation. |
| You can explain the subsystem to someone else | Sufficient understanding | Stop. Write the context doc. |
| You're exploring "just in case" | Curiosity, not necessity | Stop. Explore when blocked, not preemptively. |

### The 2-round rule

- **Round 1:** Broad survey (2-4 scouts). Map structure, data flow, config.
- **Round 2:** Targeted follow-ups (1-2 scouts). Fill specific gaps identified in synthesis.
- **Round 3:** Almost never needed. If two rounds didn't answer your question, either reframe the question or read the code yourself.

### Exploration vs. design time

| Codebase familiarity | Exploration budget | Then |
|---------------------|-------------------|------|
| Greenfield / never seen | 1 broad round + 1 targeted round | Start designing |
| Familiar codebase, new subsystem | 1 targeted round | Start designing |
| Familiar codebase, known subsystem | Skip scouts. Read 1-2 files yourself. | Start designing |

**The test:** Can you write the `## Discovery` section of the plan? If yes, you have explored enough. If no, identify the specific gap and send one more targeted scout.

## Common Mistakes

| Mistake | Problem | Fix |
|---------|---------|-----|
| Sequential spawning | `await task()` blocks; no parallelism | Launch all scouts in the same message |
| Vague prompts | "Tell me about X" returns vague summaries | Use the scout prompt checklist above |
| Too many scouts (5+) | Synthesis overhead exceeds parallelism benefit | Max 4 per round; reframe if you need more |
| Overlapping scopes | Two scouts search the same files | Use the overlap test before launching |
| Dependent questions | Scout B needs Scout A's answer | Run sequentially or make independent |
| Scout dump synthesis | Paste each scout's output without integration | Write a narrative; see synthesis guide |
| Exploring forever | "Just one more scout" delays design | Apply the 2-round rule |
| Using scouts for edits | Scouts are read-only | Use `maestro:dispatching` for implementation |

## Prompt Templates

### Codebase Structure

```
Map the file structure of [DIRECTORY].
For each file, return:
- File path
- Purpose (1 sentence)
- Exports (function/class/type names)
- Which other modules import from it

Return as a table. Do not read files outside [DIRECTORY].
```

### Data Flow Trace

```
Trace the data flow for [OPERATION]:
- Where does the data enter? (entry point, function signature)
- What transforms are applied? (each step with file:line)
- Where does the data exit? (return value, side effect, output)

Return as an ordered list with file:line for each step.
```

### Test Coverage Audit

```
Find all tests related to [FEATURE] in [TEST_DIRECTORY]:
- What test files exist?
- What behaviors are tested? (list each test name)
- What edge cases are covered?
- What's obviously missing?

Return as a table with columns: file, test_name, behavior_tested.
```

### Dependency Impact

```
Find all files that depend on [MODULE/FUNCTION]:
- Direct imports (files that import from [MODULE])
- Call sites (files that call [FUNCTION] with file:line)
- Type references (files that use [TYPE] in signatures)

Return as a table with columns: file, relationship_type, specific_usage.
```

### Configuration Audit

```
Find all configuration points for [SUBSYSTEM]:
- Environment variables
- Config file keys
- CLI flags
- Hardcoded defaults

For each, return: source, key, default_value, effect.
Return as a table.
```

## Reference Documents

- `reference/scout-patterns.md` -- Concrete fan-out strategies for common investigation types
- `reference/synthesis-guide.md` -- Patterns for merging findings, resolving contradictions, gap analysis
