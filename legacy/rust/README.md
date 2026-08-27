# Rust-era Maestro data

Byte-identical copies of the last Rust-era store on `origin/main` (`ef87d0d4`, 2026-07-08):

- `store.sqlite`: `cards` (936), `card_files` (1370), `receipt_artifacts` (32) — import with `maestro import rust --path legacy/rust/store.sqlite`, then read with `maestro search` and `maestro legacy show <card>`. Add `--promote` to create native work and decisions; the second promoted run creates nothing. Of the 32 receipts, 22 attach to promoted work and 10 whose card no longer exists are skipped and counted.
- `archive-cards.sqlite`: `archived_snapshots` (110) and `archive-INDEX.md` — import with `maestro import rust --path legacy/rust/archive-cards.sqlite`. Snapshots become searchable legacy files; Bun decodes their zstd payloads when possible and the summary counts any payloads that fell back to stored search text.
- `RECOVERY.md`: the Rust-era recovery notes.

The TypeScript store opens these files read-only and never writes them. Native
promotion uses `legacy_map` (`legacy id -> native id`) for idempotence. Mapping:
feature/task/idea/bug retain their kind, progress becomes `chore`; shipped,
closed, and verified become `done`; cancelled, abandoned, rejected, and
dismissed become `cancelled`; all other work remains open without a lease.
Decision cards become draft, locked, or superseded native decisions and retain
their parent-work and supersession links when those targets exist. Every
promoted card records `imported from legacy card <id>` provenance.
