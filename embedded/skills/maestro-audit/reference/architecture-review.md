# Architecture Review

Use this branch when the user asks to improve codebase architecture, find
deepening opportunities, or scan for shallow modules.

## Vocabulary

Use these terms exactly: module, interface, implementation, depth, deep,
shallow, seam, adapter, leverage, locality. Avoid component, service, API,
boundary, layer, wrapper, and cleaner code when those terms would blur the
architecture claim.

## Explore

1. Use the Maestro-native search engine first: `maestro grep "<topic>"`. Narrow
   with `maestro grep "<topic> corpus:memory"` for artifact precedent or
   `maestro grep "<topic> corpus:source"` for source evidence. Read the
   matching feature spec, handoff, notes, locked decisions, memory hits, and
   source evidence when present.
2. Explore organically and note where understanding a concept requires bouncing
   through many small modules, where interfaces are as complex as
   implementations, where testability extractions hurt locality, where modules
   leak across seams, and where tests struggle to cross the current interface.
3. Apply the deletion test: deleting a shallow module merely moves complexity;
   deleting a deep module spreads complexity across callers.
4. Do not propose interfaces yet. This branch finds candidates only.

## Report

Write a self-contained HTML report outside the repo:

- resolve temp dir from `$TMPDIR`, falling back to `/tmp`
- write `<tmpdir>/architecture-review-<timestamp>.html`
- use Tailwind and Mermaid from CDNs
- mix Mermaid for graph-shaped relationships with hand-built CSS/SVG for mass
  diagrams, cross-sections, call-graph collapse, and editorial before/after
  visuals
- open it with `open <path>` on macOS, `xdg-open <path>` on Linux, or
  `start <path>` on Windows
- tell the user the absolute path

Each candidate card includes:

- Files
- Problem
- Solution
- Benefits in terms of locality, leverage, and test improvement
- Before / after diagram
- Recommendation strength: `Strong`, `Worth exploring`, or `Speculative`

Mark real locked-decision conflicts only when the friction is strong enough to
justify superseding the decision. End with one **Top recommendation**.

Completion criterion: the report exists in the temp directory, has at least one
candidate with before/after visuals and recommendation strength, names the top
recommendation, and asks the user which candidate to explore.

## Hand-Off

When the user picks a candidate, route to `maestro-design` using its
`deepening-candidate`, `grilling`, and `domain-model` branches. If a candidate
is rejected for a durable reason future reviews need to remember, offer to lock
a Maestro decision or record a reusable memory; skip ephemeral reasons like "not
worth it right now".
