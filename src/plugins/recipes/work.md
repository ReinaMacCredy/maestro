# Work

Use this recipe for one accepted implementation unit. Keep the change inside
the work item's acceptance and authority. If the scope is unclear or must
expand, stop and return to the design recipe.

## Loop anatomy

### Perceive

Run `maestro work show <id>`, inspect `maestro ready`, read relevant decisions,
dispatches, handbacks, source, tests, and repository instructions. Name the
task-owned dirty paths before editing.

### Choose

Select the smallest behavior that can be falsified at the accepted seam. For a
behavior change, write the agreed failing test before production code. When
adding new or child work here, record `--acceptance "<observable result>"` so
Observe has a target to review against.

### Act

Start the item with `maestro work start <id>`. Make the minimum source and test edits
needed for that behavior, reusing what the repo already has before adding new
code. Minimum means the fewest concepts at the seam, not the fewest lines.
Preserve unrelated files and avoid speculative abstractions or dependencies.

### Observe

Run the focused test, then the relevant type, lint, and build checks. Review
the diff against acceptance and confirm the test could expose the defect.

### Learn

Record a `maestro work note <id> "..."` only for a reusable correction, decision, or
failed approach. Keep ordinary command output out of notes.

### Continue

Complete with `maestro work done <id>` and the evidence required by enabled policies.
`--evidence` records one opaque blob; `--claim`/`--proof` record paired
assertions and are preferred when a pair-checking gate is enabled. Use
tag-prefixed pairs such as `test:` or `qa:` when those gates are active;
the tag prefix goes on the claim. Otherwise return the next ready item or
a concrete blocker.
