---
title: Import Rust data
description: Reference or promote the preserved Rust-era stores under legacy/rust.
---

The preserved Rust stores live under `legacy/rust/`. Importing is a one-shot
read of the source database; the source file is not modified.

## Import for search and readback

```sh
maestro import rust --path legacy/rust/store.sqlite
```

The imported reference tables make legacy cards and files available through
native search and `maestro legacy show <id>`.

## Promote into native records

```sh
maestro import rust --path legacy/rust/store.sqlite --promote
```

Promotion creates native work, decisions, and provenance notes. It preserves
card kinds, terminal outcomes, decision links, supersession chronology, and
receipt provenance. Receipts whose source card is missing are skipped and
counted. `legacy_map` records legacy-to-native identity, so repeating promotion
creates no duplicate native records.

## Current source limitation

Only one legacy source can be referenced at a time. Import replaces the
`legacy_cards`, `legacy_files`, and `legacy_decisions` reference tables before
loading the selected source. Importing `archive-cards.sqlite` after
`store.sqlite` therefore replaces the searchable card references with archive
snapshot references; it does not merge both sources. Native records already
created by promotion remain native.
