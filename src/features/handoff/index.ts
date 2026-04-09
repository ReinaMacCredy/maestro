/**
 * Public surface for the handoff feature.
 *
 * Cross-feature consumers (`src/tui/state/snapshot.ts`, the composition
 * root, and tests) import from `@/features/handoff`. Deep paths into the
 * feature are not allowed from outside (enforced by
 * `bun run check:boundaries`).
 */
export type {
  CreateUkiHandoffInput,
  ExecuteUkiHandoffContent,
  PlanUkiHandoffContent,
  UkiHandoff,
  UkiHandoffContent,
} from "./domain/uki-types.js";
export { NO_SESSION_ID } from "./domain/constants.js";
export type { HandoffStorePort } from "./ports/handoff-store.port.js";
export { FsHandoffStoreAdapter } from "./adapters/handoff-store.adapter.js";
export { compressUki, parseUki, validateUki } from "./lib/uki-format.js";
export { createUkiHandoff } from "./usecases/create-uki-handoff.usecase.js";
export { listUkiHandoffs } from "./usecases/list-uki-handoffs.usecase.js";
export { pickupUkiHandoff } from "./usecases/pickup-uki-handoff.usecase.js";
export { registerHandoffCommand } from "./commands/handoff.command.js";
