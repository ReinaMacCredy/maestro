---
name: trace
description: Show agent execution timeline and performance summary
argument-hint: "[--filter <tool|agent>] [--last N] [--summary]"
disable-model-invocation: true
---

# Agent Flow Trace

[TRACE MODE ACTIVATED]

## Objective

Display the flow trace showing how hooks, keywords, skills, agents, tools, and modes interacted during this session.

## User Command

`$ARGUMENTS`

## Instructions

1. Call `trace_timeline` MCP tool first.
   - Default: latest session timeline with no arguments
   - If `--filter <tool|agent>` is provided, pass `filter`
   - If `--last N` is provided, pass `last`
2. Call `trace_summary` MCP tool after timeline.
3. Present the **timeline first**, then the **summary**.
4. Highlight:
   - **Bottlenecks** (tools with average duration >5s)
   - **Mode transitions** (execution mode changes)
   - **Failure clusters** (repeated failures by tool/step)
   - **Flow patterns** (keyword → skill → agent/tool chains)

## Output Format

```markdown
## Trace Timeline
<chronological events from trace_timeline>

## Trace Summary
<aggregate metrics from trace_summary>

## Highlights
- Bottlenecks: ...
- Mode transitions: ...
- Failure clusters: ...
- Flow patterns: ...
```
