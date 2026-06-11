# Schema evolution without migration churn

## Current state

- Schema versions are centralized in `src/foundation/core/schema.rs`, and current compatibility is exact-match only.
- Feature/task typed records are still large durable schemas, even when read through card envelopes.
- `maestro migrate` and `card_migrate` already carry substantial custom rewrite logic.
- Root design record for this brainstorm: `./SPEC-schema-evolution.md`.

## Problem

Frequent v1-to-v2 style artifact changes create too much code and file churn when every change requires strict schema rejection plus a bespoke migration. The design question is where Maestro should put compatibility: stricter migrations, reader-side normalization, or a more stable card-envelope contract.

\nUser follow-up: consider whether durable schemas should be rewired to work more like embedded resources, where future v2/v3 changes are mostly edits to shipped templates or schema packs. Boundary: templates can define new desired shapes, but existing user-owned artifacts still need reader compatibility or explicit migration.
