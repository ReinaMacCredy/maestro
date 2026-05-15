# Big bang v2.0 release, no backward compatibility

Maestro ships the harness-OS rebuild as a single 2.0 major release. Old verbs are removed. The `.maestro/` data layout migrates via `maestro setup --migrate-v2`. There is no parallel-with-flag period, no aliasing of old verbs to new, no separate binary.

Tradeoff accepted: every consumer must update on the same release. The user owns the maestro project and is willing to take the breaking change to avoid maintaining dual paths.

Implementation implications:
- No deprecation warnings, no aliases. `maestro mission`, `maestro intake`, etc. disappear cleanly.
- Tests, skill bundles, hooks, and docs all flip to the new model in one PR (or one tightly coupled series).
- The `setup --migrate-v2` command is the single migration path; it must be idempotent and reversible enough to recover from a botched run.

Rejected: parallel-with-flag (doubles surface area for months); aliasing (old vocabulary leaks into agent context); fresh binary (bifurcates the brand).
