# tdd skill adoption

## Current state

Evidence (2026-06-12):
- Source skill: github.com/mattpocock/skills skills/engineering/tdd, MIT license. 6 files: SKILL.md 4395B, tests.md 1640B, mocking.md 1481B, deep-modules.md 1239B, interface-design.md 653B, refactoring.md 387B.
- Content: red-green-refactor, vertical tracer-bullet slices (one test -> impl -> repeat, never all-tests-upfront), behavior-over-implementation testing, planning step that asks the user for interface + behavior priorities, per-cycle checklist.
- Already installed locally: ~/.claude/skills/tdd and ~/.agents/skills/tdd are REAL directories (Apr 26 / May 22), not maestro-managed symlinks. /tdd already available in every Claude Code session on this machine.
- Maestro bundle today: embedded/skills/{maestro-design,maestro-setup,maestro-audit,maestro-card} only, registered in src/domain/skills/catalog.rs:37-40, installed as symlinks via src/domain/skills/global.rs into ~/.claude/skills + ~/.agents/skills. All 4 are harness-driving skills, none are methodology.
- maestro-card uses a reference/ routing pattern (work.md, feature.md, verify.md, qa-baseline.md, qa-slice.md) - SKILL.md routes by job.
- work.md implement loop has proof gates (claim/proof e.g. 'cargo test: 40 passed') and an 'implement' lane but says nothing about HOW to implement; methodology-agnostic today.
- Overlap notes: tdd Planning step (confirm interface/behaviors with user, get approval) duplicates maestro-design/feature-accept; tdd cycle evidence (test fails -> passes) maps directly onto task claim/proof.
- Cost of bundling: catalog.rs registration, skill version bump + guard re-record on every edit (established constraint), and a name collision risk: maestro installs symlinks, but ~/.claude/skills/tdd already exists as a real dir.

## Problem

