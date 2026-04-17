# Advanced Elicitation Methods

These reasoning methods stress-test specifications through structured second passes. When a user selects [A] Advanced Elicitation at any step's A/P/C menu, suggest 3-5 methods most relevant to the current step's content. The user picks one; apply it; show improvements; user accepts or rejects.

## 1. Pre-mortem Analysis

Assume the track has already failed in production. Work backward: what went wrong? What was the root cause? What did we miss in the spec? This surfaces gaps that optimistic forward-planning misses.

**When to use:** Any step -- especially effective after vision (step 5) and functional requirements (step 8).

**How to apply:**
1. State "Imagine it's 6 months from now and this track was a failure."
2. Generate 3-5 specific failure scenarios with concrete details (not vague "it didn't scale").
3. For each scenario, trace back to what the spec should have addressed.
4. Present findings as spec improvements. User accepts or rejects each.

## 2. Inversion

Ask: "How would we guarantee this fails?" List all the ways to ensure failure. Then verify the current spec prevents each one. Reveals assumptions and blind spots that positive-framing questions miss.

**When to use:** After functional requirements (step 8) or non-functional requirements (step 9).

**How to apply:**
1. Generate 5-7 ways to guarantee failure (e.g., "ignore all error states," "assume infinite bandwidth," "skip authentication").
2. For each failure mode, check whether the current spec has a counter-measure.
3. Flag unaddressed failure modes as spec gaps with proposed fixes.

## 3. First Principles

Strip all assumptions. What is actually true versus assumed? Rebuild the requirement from ground truth. This prevents inherited bias from similar projects or conventional wisdom that does not apply here.

**When to use:** During classification (step 4) or domain requirements (step 7) -- especially when building something "like X but different."

**How to apply:**
1. List 3-5 assumptions embedded in the current content.
2. For each assumption, ask: "Is this actually true for THIS project?"
3. Remove unfounded assumptions. Rebuild from verified truths.
4. Present the before/after for user review.

## 4. Red Team

Attack your own spec. What would a hostile reviewer say? Where are the security holes? Where would competitors find weakness? This surfaces defensive gaps that collaborative thinking overlooks.

**When to use:** After non-functional requirements (step 9) or during spec approval (step 10).

**How to apply:**
1. Adopt an adversarial stance.
2. Generate 3-5 attacks on the spec: security gaps, scalability bottlenecks, UX failures, competitive weaknesses.
3. For each attack, propose a defensive requirement or spec change.
4. Present attacks and defenses. User decides which to incorporate.

## 5. Constraint Removal

Remove ALL constraints (budget, time, technology, team size). What would the ideal solution look like? Then add constraints back one at a time. This reveals which constraints are real versus self-imposed, and which constraints eliminate the most value.

**When to use:** During vision (step 5) or scope negotiation within vision.

**How to apply:**
1. Describe the unconstrained ideal solution.
2. Add back constraints one at a time, noting what changes with each.
3. Identify constraints that eliminate the most value -- these are candidates for challenging or working around.
4. Present findings. User decides whether to push back on any constraints.
