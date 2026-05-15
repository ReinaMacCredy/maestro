# Verb naming: git-style noun-verb with hot-path aliases

Maestro v2 uses `<noun> <verb>` as the primary CLI form for entity-shaped actions (`maestro task claim`, `maestro task verify`, `maestro task ship`, `maestro plan decompose`, `maestro spec validate`). Harness-shaped actions stay single-verb (`maestro setup`, `maestro recover`, `maestro bundle`).

Short single-token aliases are first-class for the hot path: `maestro claim <id>` → `task claim`, `maestro verify` → `task verify`, `maestro ship <id>` → `task ship`. Aliases route to the primary form internally; help text shows both. Aliases only exist for unambiguous hot-path task verbs; plan/spec verbs use the noun-verb form exclusively to avoid collisions.

Rationale: git's `git checkout` / `git co` pattern is the proven shape. Entity grouping aids discoverability and tab-completion, while aliases preserve typing ergonomics on the inner loop verbs an agent calls dozens of times per session. The locked alias set is small and curated, not user-configurable, so the agent context stays predictable.

Rejected: verb-first everywhere (`claim task <id>`, which loses entity grouping); flat single-token verbs only (`claim`, `verify`, ambiguous once plan/spec actions grow); strict noun-verb with no aliases (typing tax on the hot path).

Locked aliases (Phase 1):

- `claim` → `task claim`
- `verify` → `task verify`
- `ship` → `task ship`
- `block` → `task block`
- `abandon` → `task abandon`

No aliases for plan, spec, or harness-shaped verbs.
