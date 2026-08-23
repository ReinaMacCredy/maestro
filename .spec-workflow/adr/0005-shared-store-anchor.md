# ADR-0005: One store per repository, anchored at the git common root

Date: 2026-08-23
Status: accepted
Amended: 2026-08-23 (non-standard Git isolation and concurrent writers)

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
For a `.git` common directory, the store remains in its unique parent. For
common directories with any other name (submodules, bare-backed worktrees,
or `--separate-git-dir` layouts), the store lives inside that repository's
unique common directory rather than deriving a potentially shared parent.

The shared SQLite store uses a bounded busy timeout. Work IDs are allocated
inside an immediate transaction, and work leases are acquired with a
conditional compare-and-set update, so simultaneous CLI processes serialize
without duplicate IDs or multiple successful lease claimants.

## Rejected

- Per-checkout stores + cross-worktree scanning (Rust model): N stores,
  re-derived unions, advisory drift; worktree msg delivery would need a
  second transport.
- A machine-global store (`~/.maestro`): crosses repository boundaries,
  breaking the local-first per-repo model and the wipe/backup story.
- `dirname(git-common-dir)` for every Git layout: separate repositories can
  share a parent even though their common directories are distinct.
- Process-global mutexes or a daemon: CLI processes do not share memory, and
  a background coordinator contradicts the local on-demand mechanism.

## Consequences

- Coordination primitives (overlap advisories, cross-worktree msg, session
  union) become plain reads of one store — no scanning, no daemon.
- SQLite WAL plus bounded contention handling and atomic write boundaries
  supports concurrent CLI sessions without turning advisories into locks.
- The gitignore convention (`.maestro/`) already covers the anchor point.
