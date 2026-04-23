---
name: maestro-brainstorm
description: Collaboratively turn a rough idea into an approved design before planning or implementation. Use when the user wants to create a feature, change behavior, explore options, refine a fuzzy request, or decide what should be built before making a concrete plan. Inspect the current project context first, ask clarifying questions one at a time, propose 2 to 3 viable approaches with tradeoffs and a recommendation, then present a design and get user approval before handing off to maestro-plan.
---

# Maestro Brainstorm

Use this skill before planning or implementation when the design is not settled yet.

## Hard rules

1. Do not jump into implementation.
2. Do not jump into detailed planning.
3. Do not answer with a long decision memo before understanding the ask.
4. Ask one clarifying question per message.
5. Prefer multiple-choice questions when they make the choice easier.
6. If the request is too large for a single design, stop and decompose it before refining details.
7. End by getting explicit design approval, then hand off to `maestro-plan`.

## Workflow

### 1. Explore current context

- Inspect relevant files, docs, specs, tickets, or recent changes when working in an existing project.
- Learn enough to understand the current shape of the system before suggesting changes.
- Keep any context summary brief and grounded in what you actually verified.
- Follow existing patterns unless the design should deliberately change them.

### 2. Offer the visual companion when it will help

- If upcoming questions are likely to be easier visually, offer the visual companion once.
- This offer must be its own message, not combined with a clarifying question or context summary.
- Use the visual companion only for questions where seeing options is better than reading about them.
- If the user accepts, read [visual-companion.md](./visual-companion.md) before continuing.
- Use the local scripts in [scripts/](./scripts) to run the companion server.
- Decide per question whether to stay in the terminal or use the browser.

### 3. Clarify the ask through dialogue

- Start by identifying what the user is actually asking for.
- If the user is still trying to understand the problem, answer that first instead of forcing a design frame.
- Ask one focused question at a time.
- Prioritize questions that uncover:
  - purpose
  - constraints
  - success criteria
  - users or operators affected
  - non-goals
- Prefer multiple-choice when possible, but use open-ended questions when the space is still unclear.
- If the user uses vague references like `this`, `it`, or `that`, stabilize the subject before going further.

### 4. Check scope before going deeper

- If the request spans several independent subsystems, say so early.
- Break oversized asks into smaller designable slices.
- Help the user choose the first slice instead of pretending one spec can cover everything.

### 5. Build the real picture behind the scenes

Before proposing approaches, synthesize the decision-relevant context:

- current behavior
- surrounding system constraints
- likely dependencies
- risks and failure modes
- short-term versus long-term cost

Use this analysis to guide the conversation, not to dump a giant structured memo unless the user explicitly wants that format.

### 6. Propose approaches

- Once you understand the request well enough, present 2 to 3 materially different approaches.
- Lead with your recommended option.
- Explain concrete tradeoffs, not generic pros and cons.
- If one option is clearly stronger, say so.
- If there is not a real alternative, do not invent fake variety.

### 7. Present the design

- After an approach is chosen, present the design in sections sized to the complexity of the task.
- Cover the parts that matter for the request, such as:
  - architecture or flow
  - major components or surfaces
  - state or data flow
  - failure handling
  - validation or testing strategy
- For small work, a short design is enough.
- For larger work, present the design incrementally and confirm it as you go.
- Revise when the user pushes back or clarifies something new.

### 8. Get approval before planning

- Do not hand off to planning until the design is explicitly approved.
- Once approved, summarize the accepted design as the planning input.
- Carry forward:
  - the approved approach
  - constraints
  - assumptions
  - unresolved decisions
  - validation expectations

### 9. Write and review a design doc when needed

- If the design is substantial, long-lived, or the user wants a durable artifact, write it to a project-appropriate spec or design doc path before planning.
- Keep the document aligned with the approved design. Do not add new scope while writing it down.
- After writing the design doc, run a quick completeness and consistency review.
- Use [spec-document-reviewer-prompt.md](./spec-document-reviewer-prompt.md) as the template for that review.
- When delegation is allowed and useful, use that prompt template to dispatch a reviewer. Otherwise apply the same checks yourself.
- If review finds blocking issues, fix them before handing off to planning.

## Response patterns

### While clarifying

- Give a short grounded interpretation if useful.
- Ask exactly one question.
- Do not present full design sections yet.
- Do not combine the visual companion offer with other content.

### While comparing approaches

- Present 2 to 3 options.
- State your recommendation and why.
- Ask for the user's choice or correction.

### When the design is ready

- Present the design clearly and proportionally to the work.
- Ask for approval before moving on.

## Existing codebase guidance

- Explore the real structure before proposing changes.
- Keep improvements tied to the current goal.
- Do not expand into unrelated refactors.
- If the design needs boundary cleanup or simplification in touched areas, include that as part of the design.

## Visual companion assets

- Use [visual-companion.md](./visual-companion.md) for the operating guide.
- Use [start-server.sh](./scripts/start-server.sh), [stop-server.sh](./scripts/stop-server.sh), [server.cjs](./scripts/server.cjs), [helper.js](./scripts/helper.js), and [frame-template.html](./scripts/frame-template.html) as the local implementation.

## Output states

Use these states internally and when useful in the response:

- `needs-clarification`
- `design-in-progress`
- `ready-for-planning`

## Hand off cleanly

- The next phase after this skill is `maestro-plan`.
- Pass an approved design, not a vague direction.
- Do not invoke implementation from this skill.
