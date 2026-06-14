# Code playbook / styleguides surfaced to agents

## Current state

Agent-guidance surfaces today: (1) embedded/AGENTS.md + embedded/CLAUDE.md written into user repos at init (project-level instructions); (2) .maestro/harness/HARNESS.md v1.13.0, the versioned 'how to work cards' protocol; (3) embedded/skills/ (maestro-card/design/audit/setup) workflow guidance; (4) cards + decisions + notes.md for per-feature contract & rationale. None of these is a dedicated house coding-STYLE surface (naming, error handling, language idioms).

init-deep ALREADY manages a hierarchical AGENTS.md tree: per-directory files (src/interfaces/cli/AGENTS.md, src/domain/proof/AGENTS.md, src/operations/AGENTS.md, ...) each with an AGENTS-HIERARCHY block carrying Parent/Children links and area-specific notes. So per-area scoped agent guidance is an EXISTING maestro mechanism, not something a playbook would have to invent.

Conductor analogy is not 1:1: Conductor ships separate templates/code_styleguides (per-language python.md/typescript.md/go.md/general.md...) because it ports style INTO a host CLI. Maestro's native equivalent surface is AGENTS.md/CLAUDE.md, which are first-class here. Design from maestro's surfaces, not by porting Conductor's file tree, else we build a parallel system next to AGENTS.md.

Precedent (HARNESS rule 9): harness backlog item cli-2026-06-08 already proposed surfacing one task's gotchas/conventions to downstream tasks, and carries a LEAN CAVEAT against building a notepad subsystem (lightest viable form = a freeform note, not a new subsystem). This feature extends that thread; the same lean caveat applies.

## Problem

## Setup integration

maestro init delivers agent guidance via a marker-delimited MANAGED BLOCK it writes into AGENTS.md and CLAUDE.md (src/domain/install/mirrors.rs:867-873). CLAUDE.md block = '# Maestro Harness Protocol\n@.maestro/harness/HARNESS.md' (the @ auto-loads the file in Claude Code); AGENTS.md block = '... Read .maestro/harness/HARNESS.md first ...'. init also installs .maestro/skills/ (symlinked into .claude/.codex), .maestro/harness/HARNESS.md (versioned), and hook configs. init keeps existing files, creates missing ones; sync resyncs the harness + hook recorder to the running binary.

Consequence: a playbook rides EXISTING machinery, no new subsystem needed. Three concrete delivery slots: (W1) a '## Code style' section inside the versioned HARNESS.md - zero new wiring, agents already load it; (W2) a dedicated .maestro/playbook.md file + one extra pointer line in the AGENTS.md/CLAUDE.md managed block - mirrors how HARNESS.md is wired, separately versioned/synced; (W3) seed per-area '## Code style' blocks into the init-deep AGENTS.md hierarchy nodes - reuses init-deep, nearest-file scoping for free.

Ownership consequence of W1: maestro sync is version-gated + edit-preserving (src/operations/sync/mod.rs:1-9) - matching-version folders are left untouched, but a version BUMP backs up the drifted folder then overwrites it. So a Code-style section inside HARNESS.md is maestro-OWNED: user edits survive until the next HARNESS.md version bump, then get backed up + replaced. Implication: the playbook is maestro's opinionated UNIVERSAL principles; per-repo house-style customization belongs in the user-owned AGENTS.md (whose managed block is marker-delimited, so edits OUTSIDE the block are never clobbered).

Dogfooding edit (this repo, 2026-06-14): root CLAUDE.md previously imported only @AGENTS.md and carried no harness reference; added a '# Maestro Harness Protocol / @.maestro/harness/HARNESS.md' block so Claude sessions in the maestro repo auto-load the harness (and the code-style section once added). This mirrors the install template CLAUDE.md block that user repos already get. AGENTS.md consumers (Codex) still use the plain 'Read ... first' instruction since @-import is Claude-only.

## Sync-managed mirror blocks

GAP (verified): maestro init/install writes maestro-managed blocks (<!-- maestro:start --> .. <!-- maestro:end -->, src/foundation/core/managed_blocks.rs:21) into BOTH CLAUDE.md and AGENTS.md via MirrorKind::MarkdownManagedBlock (src/domain/install/mirrors.rs). But maestro sync runs extract_all in Update mode, which covers ONLY the hook recorder script + the harness protocol HARNESS.md (src/domain/extraction/mod.rs:28-52); it does NOT resync the CLAUDE.md/AGENTS.md markdown mirror blocks. So those blocks are install-time-only: today only a re-init/install refreshes them, sync leaves them stale. User requirement: the CLAUDE.md maestro-managed block (like AGENTS.md's) must be sync-updatable so shipped changes propagate via sync, not only init.

Impl nuance: HARNESS.md/skills/hook-script are version-gated by markers (frontmatter version: / # maestro:hook-version:). The CLAUDE.md/AGENTS.md mirror blocks carry NO version marker, so sync must either re-upsert idempotently (compare block body to shipped; replace only on drift, backing up) or gain a version mechanism. Idempotent re-upsert is the lean option and reuses upsert_managed_block (preserves user content outside the markers).
