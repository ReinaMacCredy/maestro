# General code style

Project instructions and established local style take precedence.

- Optimize for human readability; avoid clever constructs that hide control
  flow or ownership.
- Prefer the smallest complete solution and keep coupling low.
- Match existing naming, formatting, module boundaries, and error patterns.
- Make invalid states hard to represent and validate at trust boundaries.
- Comment the non-obvious reason, not a restatement of the code.
- Keep documentation synchronized with observable behavior.
- Test promised behavior through public or consumer-facing seams.
- Format, lint, type-check, and test the changed surface before handoff.
