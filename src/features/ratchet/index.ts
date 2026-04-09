/**
 * Public surface for the ratchet feature.
 *
 * Cross-feature consumers (`src/features/memory/usecases/memory-stats.usecase.ts`,
 * `src/tui/state/types.ts`, `src/tui/state/snapshot.ts`, the composition
 * root, and tests) import from `@/features/ratchet`. Deep paths into the
 * feature are not allowed from outside (enforced by
 * `bun run check:boundaries`).
 */
export type {
  RatchetAssertion,
  RatchetBaseline,
  RatchetSuite,
} from "./domain/types.js";
export type { RatchetStorePort } from "./ports/ratchet-store.port.js";
export { registerRatchetCheckCommand } from "./commands/ratchet-check.command.js";
export { registerRatchetPromoteCommand } from "./commands/ratchet-promote.command.js";
