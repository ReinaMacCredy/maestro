---
name: maestro-diagnose
description: Diagnosis-only investigation of a failure with unknown cause - reproduce, localize, reduce, deliver root cause with evidence. Read-only; an explicit fix request routes by target and tier instead, never here.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-diagnose

Find the cause; change nothing. Any tier may use it.

1. **Reproduce.** Capture the exact failing command or input and its output.
   No reproduction: say so and stop; do not guess from symptoms.
2. **Localize.** Bisect the surface: which layer, module, commit, or input
   feature flips the behavior. Prefer evidence (logs, minimal harnesses,
   `git bisect`) over reading and speculating.
3. **Reduce.** Shrink to the smallest input or state that still fails.
4. **Deliver.** Root cause plus its evidence chain, the reduced reproduction,
   and the smallest fix direction. With a work item: `maestro work note <id>`
   carries the one-line cause and the reproduction command; in a bundle, also
   the NOTES.md Current State. Otherwise the conversation.

## Discipline

- Every probe follows a written hypothesis, "I believe X causes this because
  Y", then the minimal test that can falsify it. No hypothesis, no probe.
- Recall first: `maestro search "<symptom>"` and `maestro attention`. A
  prior `failed:` note on the same mechanism is evidence about which
  hypotheses are already refuted.
- Three refuted hypotheses in a row: stop treating this as a local bug.
  Question the architecture; a structural cause or an unsettled design
  decision is `maestro-design` work, not a fourth probe.
- Red flags that mean return to step 1: proposing a fix before the cause is
  known, changing several things at once, "probably X" without a check.

Diagnosis is read-only toward production code: instrumentation and harnesses
live outside production paths or are reverted before delivery.

An explicit fix request routes by target and tier (`maestro-bundle` tier
rule): a quickfix or Light fix proceeds directly with the diagnosis as its
first step; Full-tier scope is `maestro-work` inside the bundle, with the
diagnosis as its first phase; a prototype fix belongs to `maestro-explore`.
Never downgrade a requested fix to analysis.
