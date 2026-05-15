# Greenfield long-lived branch for v2 implementation

v2 rebuild work lives on a long-lived `harness-os` branch off main. v1 stays on main and may receive bug fixes during the rebuild window. v2 develops in isolation; periodic merges from main into the v2 branch absorb any critical fixes.

When v2 is feature-complete, the `harness-os` branch merges to main as the 2.0 release. The merge IS the big-bang flip (ADR-0007). After merge, the `harness-os` branch is deleted.

Planning artifacts (CONTEXT.md, this ADR set, the master plan) live on main, because they document decisions that apply regardless of which branch implements them.

Rationale: keeps main releasable during the rebuild; allows v2 to break radically without affecting current users; clearer mental model than parallel-`src/v2/`-with-flip because there is no in-codebase doubling.

Rejected: parallel `src/v2/` with flip PR (doubles in-codebase paths; risks accidental cross-pollination); in-place transformation (long unstable window); bottom-up rebuild on main (intermediate states awkward).
