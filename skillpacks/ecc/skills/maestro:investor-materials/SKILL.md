---
name: maestro:investor-materials
description: "Pitch decks, one-pagers, investor memos, financial models, and fundraising materials. Use in any maestro phase when producing investor-facing documents."
lifecycle: [cross-cutting]
domain: [content:fundraising]
---

# Investor Materials

Build investor-facing materials that are consistent, credible, and easy to defend.

## Maestro Integration

**Lifecycle**: cross-cutting
**Activates when**: maestro:new-track detects relevant tech in tech-stack.md, or maestro:implement encounters matching task types.

### Phase Guidance
**In maestro:cross-cutting**: Apply fundraising document patterns when creating investor materials. Ensure consistency across pitch deck, memo, and financial model.

### Related Skills
- maestro:investor-outreach
- maestro:market-research
- maestro:frontend-slides

## Golden Rule

All investor materials must agree with each other.

Create or confirm a single source of truth before writing:
- traction metrics
- pricing and revenue assumptions
- raise size and instrument
- use of funds
- team bios and titles
- milestones and timelines

If conflicting numbers appear, stop and resolve them before drafting.

## Core Workflow

1. inventory the canonical facts
2. identify missing assumptions
3. choose the asset type
4. draft the asset with explicit logic
5. cross-check every number against the source of truth

## Asset Guidance

### Pitch Deck
Recommended flow:
1. company + wedge
2. problem
3. solution
4. product / demo
5. market
6. business model
7. traction
8. team
9. competition / differentiation
10. ask
11. use of funds / milestones
12. appendix

If the user wants a web-native deck, pair this skill with `frontend-slides`.

### One-Pager / Memo
- state what the company does in one clean sentence
- show why now
- include traction and proof points early
- make the ask precise
- keep claims easy to verify

### Financial Model
Include:
- explicit assumptions
- bear / base / bull cases when useful
- clean layer-by-layer revenue logic
- milestone-linked spending
- sensitivity analysis where the decision hinges on assumptions

### Accelerator Applications
- answer the exact question asked
- prioritize traction, insight, and team advantage
- avoid puffery
- keep internal metrics consistent with the deck and model

## Red Flags to Avoid

- unverifiable claims
- fuzzy market sizing without assumptions
- inconsistent team roles or titles
- revenue math that does not sum cleanly
- inflated certainty where assumptions are fragile

## Quality Gate

Before delivering:
- every number matches the current source of truth
- use of funds and revenue layers sum correctly
- assumptions are visible, not buried
- the story is clear without hype language
- the final asset is defensible in a partner meeting

---
## Relationship to Maestro Workflow
- `/maestro:new-track` -- Detects this skill during Step 9.5 (skill matching)
- `/maestro:implement` -- Loads this skill's guidance during task execution
- `/maestro:review` -- Uses checklists as review criteria
