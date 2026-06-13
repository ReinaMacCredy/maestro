# Build flow runs prepare without a stop

## Current state

The 'implement now / stop at the handoff' confirmation is agent-improvised: no skill prescribes it. rg over .maestro/skills/ for implement-now/stop-at-handoff/checkpoint wording returns only generated cli.md signatures -- the question was invented at the boundary, not by maestro.

On accept the CLI already prints a single clear hint (src/interfaces/cli/feature.rs:75-77): when status becomes Ready it emits 'next: maestro feature prepare <id> --draft'. The improvised question overwrote this clear hint with vaguer words.

The maestro-card pipeline one-liner omits prepare (SKILL.md:82): 'maestro-design -> [maestro-card: qa-baseline -> feature accept -> work -> verify -> qa-slice -> feature ship]'. feature.md:16-26 lists prepare between accept and ship. The canonical line you anchor on does not name the step the user hit.

prepare has two halves. 'prepare --draft' (feature.rs:189-203) writes a reviewable plan template prepare-draft.md, feature stays ready, prints 'review and run: ... --from'. 'prepare --from <plan>' (feature.rs:204-218) creates task cards and, when >=1 task is accepted+unblocked, starts the feature (ready -> in_progress). Decomposition and feature-start both live in prepare.

The auto-generated draft is dumb: it lumps all acceptance criteria into one task. In the cross-session-run-event-awareness session the agent had to rewrite the single-task template into 7 dependency-ordered tasks. The decomposition value lives in that rewrite between --draft and --from, so auto-run cannot be blind.

## Problem

