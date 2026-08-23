# ADR-0005: One store per repository, anchored at the git common root

Date: 2026-08-23
Status: accepted

## Context

Stage 1 anchored `.maestro/` at the process cwd's repo checkout. Linked git
worktrees therefore get their own private store: sessions, leases, mailbox,
and work trees in a worktree are invisible to the main checkout and vice
versa — exactly where coordination matters most (the owner's daily pattern
is parallel sessions across worktrees of one repo). The Rust maestro papered
over this by scanning sibling worktree roots read-only (`active` union,
warm-file overlap); that meant N stores, cross-store cursors, and advisory
code that re-derived what a single store would simply know.

## Decision

The store anchors at the repository's git common root (resolve via
`git rev-parse --git-common-dir`, i.e. the main checkout), not the current
checkout. All worktrees of one repository share one `.maestro/` store:
sessions, leases, mailbox, work, decisions, log. Different repositories
remain fully separate. The new store file (`maestro.db`) never collides with
the legacy Rust `store.sqlite` even when both live in the same `.maestro/`.
When a linked worktree still carries a private stage-1 store, the shared
store wins and maestro prints a one-line advisory naming the orphan file.

## Rejected

- Per-checkout stores + cross-worktree scanning (Rust model): N stores,
  re-derived unions, advisory drift; worktree msg delivery would need a
  second transport.
- A machine-global store (`~/.maestro`): crosses repository boundaries,
  breaking the local-first per-repo model and the wipe/backup story.

## Consequences

- Coordination primitives (overlap advisories, cross-worktree msg, session
  union) become plain reads of one store — no scanning, no daemon.
- SQLite WAL over one shared file handles concurrent sessions; that is what
  it is for.
- The gitignore convention (`.maestro/`) already covers the anchor point.
