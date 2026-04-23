# Runtime Recovery

Mission-specific notes for runtime supervision, automatic recovery, and checkpoint-aware resume behavior.

---

## Key Principles

- Runtime ownership is distinct from feature status.
- Recovery is append-only from an audit perspective; do not erase prior evidence.
- Staleness and failure must be derived from explicit runtime metadata, not guessed from feature timestamps alone.
- Recovery paths must be safe to run more than once and must avoid duplicate live ownership records.

## Agent Expectations

- Backend agents should add narrow unit/usecase tests first for recovery invariants.
- CLI agents should prove recovery state is operator-visible through supported surfaces.
- Any new command/path for recovery should have both JSON and human-readable verification where applicable.

## Resume / Checkpoint Safety

- Checkpoint restore must not blindly reactivate old ownership.
- Expired or stale runtime ownership should restore as recoverable/non-live state.
- Retry history and audit links should survive checkpoint save/load cycles.
