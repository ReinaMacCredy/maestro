# Prototype

Use this recipe for a throwaway sketch that makes a design or behavioral
decision cheaply, or that settles an empirical fork by observing it instead of
asking the owner ("prototype", "mock it up", "try this layout", "sketch it to
decide"). Ported from pstack's prototype playbook (Lauren Tan, MIT; notice in
THIRD_PARTY.md).

You own the design decision, not the code. The prototype is a throwaway
instrument. The real build follows `maestro recipe show feature`.

An observable fork comes here instead of to the owner
(`~/maestro/WORKFLOW.md#decisions-and-readiness`); that includes whether an
eval separates. A read-only investigation whose
deliverable is a cited answer stays in `maestro recipe show investigation` and
answers from the evidence instead of building a sketch. Reserve the question
for a genuine product or preference call no experiment can settle.

This is the one recipe where the **Laziness Protocol** (PRINCIPLES.md)
"smallest change" and the verification bar invert. Speed beats polish, code
quality does not matter, and there is no planning. The rigor is in picking the
right design cheaply. Propose variations the owner did not ask for. Throw an
approach away and try another.

## Steps

1. Scope the decision the prototype exists to make. That is which layout,
   which interaction, which density, or for an empirical fork which behavior,
   timing, or approach. No decision means no prototype. Route to
   `maestro recipe show feature`.
2. Gather references when the design space is open. Search for prior art,
   summarize a moodboard of themes, palettes, and layouts, and let the owner
   pick directions before building. Skip this when the direction is set.
3. Build throwaway in the folder `maestro topic path <slug>` prints, separate
   from production source.
   - For a visual decision, use vanilla HTML/CSS/JS or the lightest stack that
     renders the idea, CDN deps, and a dev server with hot reload.
   - For a behavioral or timing decision, write the smallest script that
     exercises the question.
   - For a logic or state-model question, put the logic in one pure module
     (a reducer, a state machine, or pure functions over a plain type) that
     could lift into the real code later. Wrap it in one self-contained HTML
     file that opens by double-click, labelled in domain language for a
     non-developer. Show the question at the top, a current-state panel
     re-rendered after every click, one free-play button per action, and
     walkthrough tabs for the happy path, a tricky edge case, and an attempt
     that should be illegal.
   - For a UI question, prefer rendering the variants inside the real page
     they would live in, in a throwaway worktree
     (`maestro recipe show worktree`), switched by a `?variant=` URL
     parameter. An empty route hides problems a populated page shows.
     Default to three structurally different variants (layout, hierarchy,
     primary affordance, not colours), five at most.
   - No production framework, no tests, no abstractions.
   - Fixing the prototype's own bugs is still this step. Iterate directly.
4. When comparing alternatives, build them behind one switcher (buttons or a
   keypress), each variant labeled. This is **Exhaust the Design Space**
   (PRINCIPLES.md) made cheap.
5. Verify by running it. For a visual decision, screenshot each variant and
   drive the interaction, with a browser driver or the repository's
   verify-<repo> skill (`maestro recipe show verification-skill`) when the
   sketch runs inside the product. For a behavioral or timing decision, observe
   the thing you are deciding by logging the timing, printing the output, or
   watching the render. The observation is the test here, not an assertion. A
   timing or perf number decides nothing until each side ran at least five
   times alternating, with errors counted and the limiter named.
6. Present alternatives, tradeoffs, and a recommendation. The output is the
   decision plus the throwaway artifact, not shippable code. Hand the chosen
   direction to `maestro recipe show feature` for the real build.
   - A verdict that settles a durable choice is recorded with
     `maestro decision draft "<choice>" --rationale "<observed evidence and prototype path>" --work <id>`,
     then `maestro decision lock <id>`.
   - Only a decision-encoding fragment (a schema, a reducer, a type shape) may
     enter a locked decision or a bundle's SPEC.md.
   - Porting the prototype is production work, routed by tier
     (`~/maestro/WORKFLOW.md#tiers`). The prototype never merges as-is. "Ship it
     as-is" waives the rewrite, not the route.
   - A prototype that is deployed or relied on in real work is no longer
     throwaway. Say so, and route its next change by tier.

## Reply

The variants explored, the evidence (screenshots for a visual decision, the
observed output or timing for a behavioral one), tradeoffs, your
recommendation, and the prototype's path in the topic folder. Say plainly that
the prototype is throwaway.
