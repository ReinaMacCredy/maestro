// Markdown bodies for the default principle pack. Lives in src/shared/ so the
// lint:arch passive-harness scan (scoped to src/{config,providers,repo,runtime,
// service,types,ui}/**) does not match the example/forbidden token names that
// appear inside this prose.

export const LAYER_ORDER_MD = `# layer-order

## Rule

Layers import forward-only along \`types -> config -> repo -> service -> runtime -> ui\`. Providers are cross-cutting and universally importable. No layer may import a sibling further down the stack.

## Rationale

Layer-order is the spine of the architecture (ADR-0017). Mechanical enforcement keeps boundaries real instead of aspirational.

## Scan Command

bun run lint:arch

## Fix Recipe

1. Identify the offending import in the lint output.
2. Move the dependency up the stack rather than reaching down.
3. Re-run \`bun run lint:arch\` until the violation set is empty.
`;

export const NO_YOLO_DATA_PROBING_MD = `# no-yolo-data-probing

## Rule

Do not run ad-hoc shell pipelines against \`.maestro/\` JSONL stores from source code. Read through the typed store port instead.

## Rationale

JSONL files are an append-only journal. Shell reads skip schema validation and the v1/v2 split, which has caused real corruption incidents.

## Scan Command

! rg -n "(cat|head|tail|awk|sed)\\s+[^\\"']*\\.maestro/(tasks|plans|evidence)" --glob 'src/**' --glob '!**/*.test.ts'

## Fix Recipe

1. Replace the shell read with the matching store call.
2. If the data isn't reachable via the port, add the missing method to the port.
3. Run \`bun test\` to confirm behavior is unchanged.
`;

export const PASSIVE_HARNESS_MD = `# passive-harness

## Rule

Maestro never schedules background work. No \`set\` + \`Interval\`, no detached daemons, no LLM calls, no auto-launching subprocesses.

## Rationale

Passive harness is the load-bearing invariant: a single repo-tracked state directory can be safely shared across agents and operators only when nothing is mutating the world while it is being read.

## Scan Command

! rg -n "setInterval|setTimeout|child_process\\.fork|spawn.*detached|new Worker\\(" --glob 'src/{config,providers,repo,runtime,service,types,ui}/**' --glob '!**/*.test.ts'

## Fix Recipe

1. Remove the background scheduler.
2. Turn the work into a CLI verb the agent or external cron invokes.
3. Record the resulting state change as an evidence row.
`;

export const PREFER_SHARED_UTILS_MD = `# prefer-shared-utils

## Rule

When two or more features need the same primitive, use the helper in \`src/shared/lib/\` instead of duplicating per feature.

## Rationale

Duplicating shared primitives drifts implementations apart and hides bugs. Centralizing helpers in \`src/shared/lib/\` is what lets boundary checks enforce the layer rule.

## Scan Command

! rg -n "^export (function|const) (generateId|appendJsonl|toIsoDate|kebabize)\\b" --glob 'src/features/**' --glob '!src/shared/**'

## Fix Recipe

1. Move the helper into \`src/shared/lib/<helper>.ts\`.
2. Replace each feature-local copy with \`import { ... } from "@/shared/lib"\`.
3. Delete the feature-local definition.
`;
