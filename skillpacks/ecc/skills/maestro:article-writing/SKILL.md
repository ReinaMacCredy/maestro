---
name: maestro:article-writing
description: "Long-form content writing for articles, guides, blog posts, and tutorials with distinctive voice. Use in any maestro phase when producing written content longer than a paragraph."
lifecycle: [cross-cutting]
domain: [content:writing]
---

# Article Writing

Write long-form content that sounds like a real person or brand, not generic AI output.

## Maestro Integration

**Lifecycle**: cross-cutting
**Activates when**: maestro:new-track detects relevant tech in tech-stack.md, or maestro:implement encounters matching task types.

### Phase Guidance
**In maestro:cross-cutting**: Apply writing patterns in any phase when producing documentation, articles, or content deliverables. Maintain voice consistency and structure.

### Related Skills
- maestro:content-engine
- maestro:investor-materials
- maestro:frontend-slides

## Core Rules

1. Lead with the concrete thing: example, output, anecdote, number, screenshot description, or code block.
2. Explain after the example, not before.
3. Prefer short, direct sentences over padded ones.
4. Use specific numbers when available and sourced.
5. Never invent biographical facts, company metrics, or customer evidence.

## Voice Capture Workflow

If the user wants a specific voice, collect one or more of:
- published articles
- newsletters
- X / LinkedIn posts
- docs or memos
- a short style guide

Then extract:
- sentence length and rhythm
- whether the voice is formal, conversational, or sharp
- favored rhetorical devices such as parentheses, lists, fragments, or questions
- tolerance for humor, opinion, and contrarian framing
- formatting habits such as headers, bullets, code blocks, and pull quotes

If no voice references are given, default to a direct, operator-style voice: concrete, practical, and low on hype.

## Banned Patterns

Delete and rewrite any of these:
- generic openings like "In today's rapidly evolving landscape"
- filler transitions such as "Moreover" and "Furthermore"
- hype phrases like "game-changer", "cutting-edge", or "revolutionary"
- vague claims without evidence
- biography or credibility claims not backed by provided context

## Writing Process

1. Clarify the audience and purpose.
2. Build a skeletal outline with one purpose per section.
3. Start each section with evidence, example, or scene.
4. Expand only where the next sentence earns its place.
5. Remove anything that sounds templated or self-congratulatory.

## Structure Guidance

### Technical Guides
- open with what the reader gets
- use code or terminal examples in every major section
- end with concrete takeaways, not a soft summary

### Essays / Opinion Pieces
- start with tension, contradiction, or a sharp observation
- keep one argument thread per section
- use examples that earn the opinion

### Newsletters
- keep the first screen strong
- mix insight with updates, not diary filler
- use clear section labels and easy skim structure

## Quality Gate

Before delivering:
- verify factual claims against provided sources
- remove filler and corporate language
- confirm the voice matches the supplied examples
- ensure every section adds new information
- check formatting for the intended platform

---
## Relationship to Maestro Workflow
- `/maestro:new-track` -- Detects this skill during Step 9.5 (skill matching)
- `/maestro:implement` -- Loads this skill's guidance during task execution
- `/maestro:review` -- Uses checklists as review criteria
