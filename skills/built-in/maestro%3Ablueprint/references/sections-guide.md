# Blueprint Sections Guide

Detailed guidance for each of the 7 blueprint sections. Use this to understand what content belongs in each section and how to present it.

## 1. Executive Summary

The first thing anyone sees. It should answer: "what is this, how big is it, how risky is it, and how long will it take?" in under 5 seconds.

**Components:**
- **KPI cards** (4 cards in a row): Files changed, Phases, Estimated LOC, Risk level
- **Problem statement**: 2-3 sentences. What exists today, what's broken or missing, what the world looks like after this change. Written for someone who just walked into the room.

**Content guidelines:**
- KPI values should be concrete numbers, not ranges. "12 files" not "10-15 files". You can be approximate but commit to a number.
- Risk level is one of: Low (routine change, well-understood), Medium (some unknowns, moderate blast radius), High (architectural change, data migration, or touching critical paths).
- The problem statement should explain WHY, not WHAT. "Users can't collaborate in real-time because the editor uses HTTP polling, causing 2-5 second delays" not "Add WebSocket support."

## 2. Architecture Diagrams

Visual representation of the system changes. The choice of diagram type depends on what matters most about this change.

**When to use which diagram type:**

| What matters | Diagram type | Mermaid syntax |
|---|---|---|
| How components connect | Flowchart | `graph TD` or `graph LR` |
| Request/response sequence | Sequence diagram | `sequenceDiagram` |
| Data model changes | ER diagram | `erDiagram` |
| State transitions | State diagram | `stateDiagram-v2` |
| Multiple concerns | Multiple diagrams | Use 2-3, each focused |

**Content guidelines:**
- Show the CHANGE, not the entire system. Highlight what's new or modified. Use Mermaid `classDef` with semi-transparent fills to distinguish new components from existing ones.
- Label edges with data or actions, not just arrows. "POST /api/events" not just an arrow.
- If the diagram has more than 15 nodes, split into 2 focused diagrams rather than one sprawling one.
- Always include a brief text description above the diagram explaining what it shows.

**Showing before/after:**
For architectural changes, consider two diagrams side by side: "Current" and "Proposed". Use the diff-panels CSS pattern for this.

## 3. Phased Implementation

A timeline view that breaks the work into ordered phases. Each phase should be independently shippable or at least testable.

**How to define phases:**
- Each phase produces something that can be verified (tests pass, feature works in isolation)
- Dependencies flow forward -- Phase 2 builds on Phase 1, never the reverse
- Name phases after what they deliver, not what they do: "Auth Layer" not "Phase 1"

**Timeline content per phase:**
- Phase number and name
- Duration estimate (relative: "~2 days", not absolute dates)
- 1-line summary of what this phase delivers
- The phase marker shows status: done (filled), active (ring), future (faded ring)

**Content guidelines:**
- 2-5 phases is the sweet spot. More than 7 phases means you're over-decomposing.
- If a phase has more than 8 tasks, it should probably be split.
- Duration estimates should be honest. If you're uncertain, say so: "~3 days (depends on API stability)".

## 4. Per-Phase Details

The drill-down layer. Each phase expands to show exactly what changes, what tasks to do, and what to test. These are collapsible by default to avoid overwhelming the reader.

**Content per phase:**

### File Changes
List every file that gets created, modified, or deleted in this phase. Use the file change indicator pattern:
- `+` (green): New file
- `~` (amber): Modified file
- `-` (red): Deleted file

Each file gets a one-line reason explaining WHY it changes.

### Tasks
Ordered list of concrete tasks within the phase. Each task has:
- A number (for dependency references)
- A name (action + noun: "Create WebSocket handler", not "WebSocket")
- A description (1-2 lines: what to do and any gotchas)
- Tags (optional): `test`, `refactor`, `breaking`, `migration`

### API Contracts (if applicable)
Type definitions, interface shapes, or schema changes introduced in this phase. Show the actual TypeScript/code signature, not prose descriptions.

### Test Plan
What tests to write for this phase. Be specific: "Unit test for `parseMessage()` with malformed input" not "Add unit tests".

## 5. Dependency Graph

A Mermaid DAG (directed acyclic graph) showing how tasks depend on each other. The critical path -- the longest chain of sequential dependencies -- should be highlighted.

**Content guidelines:**
- Nodes are tasks (use the task numbers from section 4)
- Edges show "must complete before" relationships
- Use `classDef` to highlight the critical path in a contrasting color
- Parallel tasks (no dependency between them) should be at the same vertical level
- Include a brief note about which tasks can run in parallel

**Example structure:**
```
graph TD
  T1[1. Schema migration] --> T3[3. Repository layer]
  T2[2. Config types] --> T3
  T3 --> T4[4. Service layer]
  T3 --> T5[5. API routes]
  T4 --> T6[6. Integration tests]
  T5 --> T6
  T6 --> T7[7. E2E tests]

  class T1,T3,T4,T6,T7 critical
```

## 6. Risk Matrix

A table identifying what could go wrong, how bad it would be, how likely it is, and what to do about it.

**Columns:**
- **Risk**: What could happen (specific, not vague)
- **Impact**: How bad if it happens (Low / Medium / High)
- **Likelihood**: How likely (Low / Medium / High)
- **Mitigation**: What to do to prevent or recover

**Content guidelines:**
- 3-6 risks is the sweet spot. Fewer means you haven't thought hard enough. More means you're listing noise.
- Risks should be specific to THIS change, not generic ("server could crash"). Bad: "Performance issues." Good: "The new index on `events.timestamp` could slow writes during migration of 2M existing rows."
- Every High-impact risk needs a concrete mitigation, not "be careful."
- Consider: data loss, breaking changes, performance regression, dependency failures, rollback difficulty.

## 7. Verification Checklist

Concrete steps to verify the implementation works. Someone should be able to follow this list mechanically and know if the feature is done.

**Content per item:**
- What to verify (plain language)
- Command to run (code block)
- Expected outcome (what success looks like)

**Content guidelines:**
- Start with build/lint/test basics (does it compile? do tests pass?)
- Then feature-specific verification (does the actual feature work?)
- Then edge cases and error handling
- End with integration/E2E if applicable
- Commands should be copy-pasteable. No "run the appropriate test command" -- specify the exact command.
- Expected outcomes should be observable. "No errors" is OK. "The system feels faster" is not.
