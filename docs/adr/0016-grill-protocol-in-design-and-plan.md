# Grill protocol baked into design + plan skills

Maestro v2 adopts the `grill-with-docs` interaction pattern as a named protocol the agent runs inside `maestro-design` and `maestro-plan`. The protocol is: walk the decision tree one branch at a time, ask one question per turn with a recommended answer, challenge user language against the existing CONTEXT.md glossary, cross-reference claims against code when verifiable, update CONTEXT.md inline as terms resolve, and offer ADRs sparingly (only when the decision is hard to reverse, surprising without context, and a real trade-off).

Grill is not a sixth skill (ADR-0012's 5-skill bundle is preserved) and adds no new CLI verbs. It is a protocol the existing skills run internally. There is no `maestro spec grill` or `maestro plan grill` verb; entering the design or plan skill is what runs the grill.

**Verb-category rule (clarifies the apparent inconsistency with ADR-0015's `principle promote`):** maestro accepts new verbs that *materialize a new artifact* from an existing one (`principle promote` writes a markdown file from an evidence row) but rejects new verbs that are *flow shortcuts* into a protocol the owning skill already runs (`spec grill` would just re-enter the design skill's interview). The distinction: material-producing verbs change the file tree; flow-shortcut verbs only re-trigger interaction. The 5-skill bundle owns the flows; verbs own the artifact production.

The grill protocol is the deliberate human-driven counterpart to the auto-promotion machinery ADR-0015 removed: together they make principles authoring intentional rather than emergent. Memory's `memory-compile` would have auto-promoted recurring corrections into principles silently; the grill interview surfaces those promotions as explicit decisions the agent commits.

Inside `maestro-design`, grill drives the spec-authoring Q&A: acceptance criteria, non-goals, risk class, mode, work-type, plus stress-tests of the spec body against committed knowledge in CONTEXT.md and ADRs. The skill SKILL.md documents the grill steps verbatim so the agent knows how to interview.

Inside `maestro-plan`, grill drives the decompose step: each proposed child task is challenged against the spec, CONTEXT.md, and the architecture lint set before the task batch is emitted. The skill SKILL.md documents the grill steps for the plan context.

The protocol produces committed artifacts on exit (refined spec frontmatter, updated CONTEXT.md, new ADRs where genuinely warranted). This matches the article principle "what Codex can't see doesn't exist": the grill output lands as committed markdown, not as a transient session log.

Phasing: Phase 1 ships the grill protocol inside `maestro-design` (spec authoring runs on grill from day 1). Phase 2 extends the protocol to `maestro-plan` (decomposition runs on grill).

Rejected: sixth skill `maestro-grill` (breaks ADR-0012's small-stable-bundle commitment); dedicated verbs `maestro spec grill` / `maestro plan grill` (adds CLI surface for a flow the skills already own; user constraint is to integrate the skill, not grow the verb surface); verb-only without skill integration (loses the canonical interaction pattern).
