# ref-template concept adoption

## Current state

Source bundle: ~/Downloads/Compressed/ref/ = Codex Human-in-the-Loop Template; 21 markdown files, no code. Top-level policy docs (AGENTS.md map, ARCHITECTURE.md codemap+invariants, PRODUCT_SENSE.md, QUALITY_GATES.md, SECURITY.md, PLANS.md exec-plan rules), 3 workflow docs, 5 templates (task brief / exec plan / ADR / PR review / retro), 3 copy-paste prompts. Everything is prose convention; nothing mechanized.

Maestro coverage verified: task briefs -> task create with locked acceptance (HARNESS.md 1.11.0 lines 39-49); exec plans -> feature cards with notes.md/decisions.yaml/acceptance_evidence (e.g. no-dead-end-errors card); ADR -> decisions store; quality gates -> qa-baseline/qa-slice + proof-backed claims; session start/review loops -> status + skills. Uncovered: risk field on tasks is stored but gates nothing (src/domain/task/mod.rs:49,157-158 - set, never checked by any operation); no protected-areas policy; no architecture-invariants surface shipped to user repos (embedded/ has only HARNESS.md, RECOVERY.md, hooks, skills, shell); no standing-principles home (decisions are born from per-feature forks); no retro prompt at feature ship (self-improvement loop is passive friction counting per self-improvement-loop-hygiene card).

Adjacent precedent: decision card-2234cf (design-session-spec-workflow, locked) - approval capture routes constraints by type, accept grows nothing; relevant to any approval-gate shape. self-improvement-loop-hygiene (proposed) - friction/idea lineage gaps; overlaps the ship-time retro concept.

## Problem

