# prefer-shared-utils

## Rule

When two or more features need the same primitive (id generation, JSONL append, time formatting, slug validation, etc.), use the helper in `src/shared/lib/` instead of duplicating the logic per feature.

## Rationale

Duplicating shared primitives across features drifts implementations apart and quietly hides bugs (e.g. one feature trims slugs, another doesn't). `src/shared/lib/` is the single composition point that every feature can import; keeping the helpers there is what lets `check:boundaries` enforce the layer rule.

## Scan Command

! rg -n "^export (function|const) (generateId|appendJsonl|toIsoDate|kebabize)\b" --glob 'src/features/**' --glob '!src/shared/**'

## Fix Recipe

1. Move the helper into `src/shared/lib/<helper>.ts` and re-export from `src/shared/lib/index.ts`.
2. Replace the feature-local copy with `import { ... } from "@/shared/lib"`.
3. Delete the feature-local definition.
4. Run `bun run check:boundaries && bun test`.
