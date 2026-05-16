# v1 sunset scope = v2-displaces

Phase 4 (docs polish + skill-surface cleanup, PRs 36-40) leaves the v1 source tree under `src/features/` untouched. After Phase 4 lands, `src/features/` still contains 31 feature directories, most of which v2 has replaced without v1 being deleted. This ADR fixes the deletion rule for the new Phase 5.

**Rule: v2-displaces.** For each `src/features/<x>/` directory, ask "does v2 own this verb today?" If yes, delete the v1 implementation. If no, keep it.

**v2 owns (delete in Phase 5):** `task`, `spec`, `verify`, `setup`, `mission`, `intake`. The exec-plan workflow portion of `plan` (split below).

**ADR-0015 absorbed/dropped (delete):** `memory`, `memory-ratchet`, `agent`, `graph`, `session`, `ralph`, `notes`, `inspect`, `state`.

**Non-goal kept (do not delete):** `mcp`, `ci`, `gc`, `recover`, `bundle`, `verdict`, `policy`, `risk`, `evidence`, `skills`, `deploy`, `review`, `merge`, `runtime`. The plan-check + cost-budget portion of `plan`. Mission Control TUI (`src/tui/`). Hooks (`hooks/`). GitNexus integration.

**Judgment-call dirs audited at Phase 5 kickoff:** `handoff`, `worktree`, and the `plan` split. v2 has emission-side adapters (`src/v2/repo/fs-handoff-emitter.adapter.ts`, auto-worktree at claim) but may not own the full read-side machinery (pickup, list, show). The Phase 5 PR plan lists which subroutines port into `src/v2/` versus stay. Principle: if v2 owns the agent-facing verb, the supporting machinery ports into `src/v2/` during sunset.

**Stability gate:** v2 e2e green (all `tests/e2e/v2-*.test.ts` pass), kept-feature e2e green, no dead imports, plus one dogfooded spec → task → ship cycle on maestro itself using only v2 verbs. Scenario tests (ADR-0019) are a separate gate in Phase 6.

Rejected: mapped-only deletion (only the ADR-0015 list — leaves v1 implementations of v2-owned verbs as rotting fallback); v2-displaces + parity-gate (a written parity test per dir — duplicates the Phase 6 scenario suite's job).
