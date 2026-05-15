# layer-order

## Rule

V2 layers import forward-only along `types -> config -> repo -> service -> runtime -> ui`. Providers are cross-cutting and universally importable. No layer may import a sibling further down the stack; cross-feature reads in v1 must go through `@/features/<name>` public surfaces.

## Rationale

Layer-order is the spine of the v2 architecture (ADR-0017). Mechanical enforcement of the rule is the difference between "we have boundaries" and "the codebase quietly turned into a tangle of cross-imports". The lint runner already enforces this for v2 (`bun run lint:arch`); a same-rule principle keeps the rule legible to humans and agents.

## Scan Command

bun run lint:arch

## Fix Recipe

1. Identify the offending import in the lint output.
2. If the importer needs the value, move the dependency *up* the stack rather than reaching down (e.g. introduce a port in `repo/` and pass it in via the composition root).
3. If two siblings legitimately share logic, extract the shared piece into the layer below both of them or into `providers/`.
4. Re-run `bun run lint:arch` until the violation set is empty.
