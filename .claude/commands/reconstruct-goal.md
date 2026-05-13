---
description: Reconstruct project intent from session + repo evidence and emit a directly executable /goal prompt; persist the spec under .maestro/goals/ with baseline + verdict rule for iterative runs.
argument-hint: [optional slug or objective]
---

Read the full session history and repository, then reconstruct the project's true intent, priorities, and end-state from evidence — not surface descriptions. Produce a directly executable /goal prompt, and persist the underlying spec so future runs can iterate against it.

Objective (if provided): $ARGUMENTS

# Sources (primary, in order)
- Conversation history — what we discussed, decided, corrected, retracted
- Repository state — code, configs, scripts, generated artifacts
- Repo history — commits, PR threads, issue threads, TODOs, inline comments
- Documentation — READMEs, AGENTS/CLAUDE files, docs/, skills/

# Reading the conversation
- User corrections and repeated preferences are authoritative
- Distinguish exploratory ideas from finalized decisions
- Detect shifts in direction; preserve the latest position
- Capture why a decision was made, not just the decision

# Reconstruct
- The problem actually being solved (not the README framing)
- Intended product behavior and UX
- Architectural and engineering philosophy
- Outcomes we were aligning toward
- Invariants, constraints, assumptions
- Short-term vs long-term priorities
- Current gaps, contradictions, unfinished objectives
- What "success" concretely looks like

# Rigor
- Cite evidence (conversation quote, file path, commit SHA, doc line) on every specific claim
- Mark assumptions explicitly; flag unresolved ambiguity
- Prefer high-confidence interpretation over broad guessing
- If a few targeted questions would materially improve accuracy, ask before finalizing

# Persist the spec
Write the full analysis to `.maestro/goals/<slug>.md` (create the directory if missing). Frontmatter must include:
- `slug`, `created_at`, `branch`
- `success_criterion`: one verifiable sentence
- `baseline`: the exact command(s) that capture current state as numbers, plus their stdout verbatim
- `verdict_rule`: which numbers must drop, which must not regress, by how much
- `stop_conditions`: when to halt the loop

Body holds the 8-section analysis. On later runs, append a `# Run <ISO date>` log entry with new measurements and verdict (better / same / worse, with deltas). Do not rewrite history — append.

# Measurement contract
Every generated /goal that targets *improving* something must embed:
- A **baseline command** the agent runs before touching code, with stdout recorded.
- A **verdict rule** stating which numbers must drop and which must not regress.
- A **re-measure command** (usually identical to baseline) run after each change-set.

A run claiming "improved" without numbers is invalid and must be rejected.

# The generated /goal prompt — structure
Must be **directly executable**, not orientation. ≤4000 chars. Sections in this order:
1. Identity — one sentence (where, stack, branch)
2. Mission — one sentence (the verifiable thing this session changes)
3. Baseline + verdict rule (the measurement contract)
4. Execute loop — numbered steps the agent runs without further prompting
5. Hard invariants — project-specific, re-read before destructive moves
6. Definition of done — per change-set, with numbers in the commit body
7. Stop conditions — when to halt, including "no improvement"
8. Pointers — load on demand only

Test the generated /goal against this failure mode: if pasting it causes the agent to report posture and ask "what next," the /goal is broken — rewrite it with a concrete first action and a queue-claim step.

# Your response — output sections
1. Reconstructed Intent & Goals
2. Key Decisions
3. Architectural & Product Philosophy
4. Evidence (cite specifics)
5. Constraints / Invariants
6. Gaps / Misalignments
7. Open Questions
8. Final /goal Prompt — concise, authoritative, paste-ready, ≤4000 chars
9. Confirm: `.maestro/goals/<slug>.md` was written with the baseline captured. Print the absolute path.
