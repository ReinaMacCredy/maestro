# Learning

Use this recipe when a verified correction, repeated failure, or durable
decision should guide future work.

## Better loop

Move through this chain without skipping a link:

1. Capture the episode and its literal evidence.
2. Identify the common mechanism across episodes.
3. Locate the smallest owning layer.
4. Make one rollback-able correction there.
5. Run a positive canary and a negative canary.
6. Promote only after both canaries and one real use pass.
7. Put a review/delete date on every promoted rule.

Useful metrics expose outcomes and correction quality: recurrence after the
change, rollback rate, time to locate the owning layer, and canary-to-real-use
agreement. Gameable metrics reward activity instead: note count, rule count,
raw test count, agent turns, and claims without layer-qualified proof. Use a
metric only when gaming it would still improve the intended outcome.

## Loop anatomy

### Perceive

Read the source event, evidence, decision, work note, or repeated failed
approach. A plausible idea without a durable source is not a lesson yet.

### Choose

Choose the narrowest durable home: a work note for local continuity, a locked
decision for an accepted fork, repository memory for stable project knowledge,
or a proposed workflow change for a repeated procedure.

### Act

Write a concise lesson that names its scope and source. Do not paste logs or
store secrets. Do not change global workflow rules without accepted scope.

### Observe

Read the artifact back, search for duplication, and ensure later agents can
tell when the lesson applies.

### Learn

Treat the saved artifact, not this conversation, as the durable result.

### Continue

Return to the active work or design item. If no reusable lesson exists, say so
and leave durable memory unchanged.
