---
name: maestro:research
description: "Structured research workflow for maestro features. Guides tool selection across three tiers (codebase exploration, Context7 for library docs, NotebookLM for deep analysis), defines research patterns, finding organization via memory_write, and completion criteria. Use during the research pipeline stage after feature_create and before plan_write. Also use when investigating a problem space, comparing technical approaches, gathering context on unfamiliar code, or needing to understand external library APIs before making architectural decisions."
---

# Research Phase

Research sits between discovery and planning. Gather enough understanding to write a confident plan. Question assumptions, start with the smallest investigation that could answer the question, and scale up only when complexity warrants it.

## When to Research

Enter research when:
- A feature exists (`feature_create` done)
- The problem involves unfamiliar code, external libraries, or multiple valid approaches
- You can't yet write a confident `## Discovery` section for the plan

Skip research when the task is mechanical (rename, config change, straightforward addition) or you already have deep context from prior work.

## Research Tiers

Your approach adapts to what tools are available.

### Tier 1: Always Available

**Codebase exploration** -- Agent subagents

Use for understanding existing code, finding patterns, tracing data flow. Spawn focused subagents with one clear objective each.

```
memory_write({ feature: "my-feature", name: "research-existing-auth-patterns", content: "..." })
```

**Web search** -- WebSearch + WebFetch

Use for error messages, ecosystem comparisons, best practices. Save fetched content to a local file or memory immediately -- interrupted sessions lose unsaved fetches.

### Tier 2: Context7

**Available when**: Context7 MCP server is configured.

Current library documentation, not stale training data. Use when:
- Checking API signatures for a specific library version
- Finding migration guides between versions
- Understanding framework patterns that change across releases
- Verifying that an approach is still the recommended one

**Fallback**: WebSearch for library docs. Note in findings that docs may be outdated relative to your target version.

### Tier 3: NotebookLM

**Available when**: NotebookLM MCP server is configured.

Multi-source synthesis -- feed several documents and ask analytical questions. Use for:
- Comparing architectural approaches with trade-off analysis
- Synthesizing information from multiple docs/specs/codebases
- Generating structured analysis from unstructured sources
- Deep-diving into an unfamiliar domain

## Research Patterns

| Pattern | When | Approach |
|---|---|---|
| Breadth-first | Unknown unknowns | 2-3 parallel subagents exploring different angles, synthesize, targeted follow-up on gaps |
| Depth-first | Known area, need depth | Start at entry point, trace full path (data flow, call chain, state), document edge cases |
| Comparative | Multiple valid approaches | Define criteria, research each approach in parallel, build comparison matrix, recommend with reasoning |
| Constraint discovery | Migrations, refactors | Check code for invariants, deps for version limits, project config; separate hard constraints from soft preferences |

Save all findings with `memory_write` -- see Organizing Findings below.

## Organizing Findings

Every significant finding gets saved with `memory_write` scoped to the current feature. Structure each finding:

```
## Motivation
What question were we trying to answer? (1-2 lines, precise, no vague language)

## Findings
What did we learn? (methods, evidence, relevant files/sources)

## Implications
How does this affect the plan? (design decisions, constraints, recommendations)

## Limitations
What caveats apply? (confounders, incomplete coverage, assumptions made)

## Next Steps
What follow-up research would strengthen this finding? (optional -- only if gaps remain)
```

Name findings `research-<discriminator>` using: `<area>`, `<lib>-api`, `<topic>-comparison`, `constraints`, or `<source>-notes`.

## Completion Criteria

Research is done when all five hold:

1. **Problem understood** -- explainable in one paragraph without hand-waving
2. **Solution space mapped** -- viable approaches and trade-offs known
3. **Constraints identified** -- technical, time, and dependency limits known
4. **Key decisions ready** -- major architectural choices can be made
5. **No blocking unknowns** -- remaining unknowns are explicitly accepted as limitations

If any criterion fails, target the specific gap. If a question can't be answered without building the thing, note it as a limitation and move on.

## Transitioning to Planning

The plan's `## Discovery` section is where research pays off. Reference your findings by name and write it as a confident summary -- motivation, key findings, implications, and acknowledged limitations. Not a log of what you did.

## Tool Selection Quick Reference

| Question | Subagents | Context7 | NotebookLM |
|---|---|---|---|
| How does our code handle X? | [-->] primary | -- | -- |
| What's the API for library Y? | WebSearch fallback | [-->] primary | -- |
| Should we use approach A or B? | Research each | Docs for each | [-->] synthesize |
| What are the constraints? | Code + config | Dep docs | -- |
| What's the ecosystem best practice? | WebSearch | [-->] current docs | Deep analysis |
| Synthesize 5+ sources into analysis | Manual | -- | [-->] primary |
| Is this still the recommended pattern? | -- | [-->] primary | -- |

`[-->]` = best tool for the job. Use the highest available tier. Fall back gracefully when tools aren't installed.
