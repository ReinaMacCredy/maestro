# PRD Synthesis

Use this branch when the user asks to turn the current conversation into a PRD.
Synthesize what is already known; do not start a discovery interview.

## Process

1. Map current codebase state if it has not already been mapped. Use domain
   glossary vocabulary and respect relevant ADRs.
2. Sketch the test seams for the feature. Prefer existing seams and the highest
   useful seam; the ideal is one seam. If publishing externally, pause only to
   confirm the seam sketch before publication.
3. Write the PRD. In Maestro, preserve it on the feature with `feature spec`
   sections and use `feature finalize` for the clean handoff. If an external
   issue tracker is configured and the user asked to publish there, publish the
   PRD and apply the `ready-for-agent` label.

Completion criterion: the PRD has the sections below, the test seam is named,
and either the Maestro feature handoff is refreshed or the external issue is
published with `ready-for-agent`.

## Template

```md
## Problem Statement

The problem from the user's perspective.

## Solution

The solution from the user's perspective.

## User Stories

1. As an <actor>, I want a <feature>, so that <benefit>

## Implementation Decisions

- Modules or interfaces to build or modify
- Technical clarifications
- Architectural decisions
- Schema changes
- API contracts
- Specific interactions

Do not include file paths or code snippets. Exception: a prototype snippet may
be included when it encodes a decision more precisely than prose, trimmed to the
decision-rich part and labeled as prototype-derived.

## Testing Decisions

- Good tests assert external behavior, not implementation details
- Modules or seams to test
- Similar prior tests in the codebase

## Out of Scope

Explicit non-goals.

## Further Notes

Remaining context.
```
