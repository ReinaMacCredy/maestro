# Rust-era Maestro data

Byte-identical copies of the last Rust-era store on `origin/main` (`ef87d0d4`, 2026-07-08):

- `store.sqlite`: `cards` (936), `card_files` (1370), `receipt_artifacts` (32) — import with `maestro import rust --path legacy/rust/store.sqlite`, then read with `maestro search` and `maestro legacy show <card>`.
- `archive-cards.sqlite`: `archived_snapshots` (110) and `archive-INDEX.md` — not yet accepted by the importer.
- `RECOVERY.md`: the Rust-era recovery notes.

The TypeScript store never writes here. Promotion of cards into live work items is tracked as w116.
