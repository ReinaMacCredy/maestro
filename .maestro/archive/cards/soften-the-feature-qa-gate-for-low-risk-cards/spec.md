# Soften the feature QA gate for low-risk cards

## Current state

QA is a FEATURE-level gate, not per-task. Tasks have NO QA gate at all. accept gate = baseline_present() (qa.md must exist); ship gate = ship_qa_gaps() = presence + freshness (E.1) + per-[bl-NNN] scenario coverage (E.2) (src/domain/feature/qa.rs).

A zero-[bl-NNN] baseline ALREADY ships with no slices: test no_behavioral_surface_ships_with_no_slices proves a baseline with no behavioral scenarios passes ship with zero slice coverage. So 'soft QA' partly exists -- the residual ceremony is (1) a qa.md file must EXIST at accept even for trivial features, and (2) any [bl-NNN] you do write demands a counting slice at ship.

The shipped light lane (dec-light-scope-relax-test-first-and-0720) relaxes TASK gates (test-first, simplify) but DELIBERATELY KEPT accept/QA. Softening accept/ship QA therefore SUPERSEDES that decision's 'keep accept' clause and is a heavier (binary-level) change than the skill-only light lane. light is behavior-keyed not size-keyed (dec-light-is-behavior-keyed-not-size-keyed-f867) and is a marker word not a grant (dec-light-marker-lane-light-convention-word-9e0d).

## Problem

## Review findings (live)

LIVE-VERIFIED 2026-06-17 (binary g2650f3a5, scratch repo): existing `maestro feature accept <id> --qa none --reason` ALREADY satisfies ac-1..ac-5. Proven: accept succeeds with NO qa.md (ac-1); feature show renders 'qa: none (<reason>)' (ac-3); ship --dry-run shows NO qa-baseline/qa-slice gap, only the unrelated acceptance-sweep gate (ac-2); honor-system, no acceptance/area inspection (ac-4); a non---qa-none feature still gates (ac-5). CORRECTS the current-state claim that 'a qa.md must EXIST at accept even for trivial features' -- it does NOT; --qa none --reason waives it. Only ac-6 is unmet: skill prose never documents this path (--qa none lives only in cli.md). The designed separate 'light marker' would DUPLICATE --qa none, which is additionally SAFER (carries a freshness re-block the flat marker lacked). ONE REAL BUG: qa_declared_none freshness uses amend_log_position == amends.len() (count-based, registry.rs:847) so a NON-behavioral amend (--add-non-goal) over-blocks ship with 'qa-baseline missing' (proven live), whereas qa.md freshness correctly filters on is_behavioral(); one-line alignment fix.
