# Evidence emits on every state transition plus ad-hoc

Each lifecycle transition (task and exec-plan) writes one evidence row automatically with `kind=transition` and structured fields recording from-state, to-state, trigger (verb call), verdict (if applicable), and witness level. Agents and adapters can also write ad-hoc evidence rows outside transitions for things like test runs, runtime signals, plan-check results, or long-running checks.

This makes the lifecycle fully auditable from `.maestro/evidence/` while preserving the existing fine-grained evidence kinds. Witness levels (L0-L7) stay as defined in `docs/witness-levels.md`.

Rejected: evidence only on transitions (loses mid-state observations); evidence independent of transitions (harder to audit cause of transition); no evidence concept (loses witness levels and provenance).
