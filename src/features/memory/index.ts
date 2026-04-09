/**
 * Public surface for the memory feature.
 *
 * Cross-feature consumers (`src/tui/state/snapshot.ts`,
 * `src/tui/state/types.ts`, the composition root, worker prompt
 * generator, and tests) import from `@/features/memory`. Deep paths
 * into the feature are not allowed from outside (enforced by
 * `bun run check:boundaries`).
 *
 * Phase 6 relies on `recallMemory` being available from this surface so
 * the worker can inject matching corrections into generated prompts.
 */
export type {
  CompiledLearnings,
  Correction,
  CorrectionQuery,
  CorrectionTrigger,
  CreateCorrectionInput,
  MemoryConfig,
  MemoryStats,
  RawLearningEntry,
} from "./domain/memory-types.js";
export type { CorrectionStorePort } from "./ports/correction-store.port.js";
export type { LearningStorePort } from "./ports/learning-store.port.js";
export { FsCorrectionStoreAdapter } from "./adapters/correction-store.adapter.js";
export { FsLearningStoreAdapter } from "./adapters/learning-store.adapter.js";
export {
  recallMemory,
  type RecallContext,
  type RecallResult,
} from "./usecases/memory-recall.usecase.js";
export {
  buildMemoryStats,
  getMemoryStats,
} from "./usecases/memory-stats.usecase.js";
export { registerMemoryCompileCommand } from "./commands/memory-compile.command.js";
export { registerMemoryCorrectCommand } from "./commands/memory-correct.command.js";
export { registerMemoryLearnCommand } from "./commands/memory-learn.command.js";
export { registerMemoryLintCommand } from "./commands/memory-lint.command.js";
export { registerMemoryRecallCommand } from "./commands/memory-recall.command.js";
export { registerMemorySearchCommand } from "./commands/memory-search.command.js";
export { registerMemoryStatsCommand } from "./commands/memory-stats.command.js";
