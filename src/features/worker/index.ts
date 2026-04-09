/**
 * Public surface for the worker feature.
 *
 * External consumers (mission feature's feature.command, install/update/
 * uninstall commands, TUI config inspector, TUI modal builders, tests)
 * must import from `@/features/worker` rather than reaching into subpaths.
 *
 * Phase 6 note: worker is the only feature that legitimately imports from
 * another feature's public surface -- it consumes `@/features/mission`
 * and `@/features/memory` to compose worker prompts. That exception is
 * documented in `scripts/check-feature-boundaries.ts` via FEATURE_EXCEPTIONS.
 */

// Domain types (worker config shapes read from maestro.yaml)
export type {
  WorkerConfig,
  WorkerOutputMode,
  SupervisionLevel,
  CliWorkerConfig,
  ExecutionConfig,
  SupervisionConfig,
  ParallelConfig,
} from "./domain/worker-types.js";

// Worker config validator
export { WorkerConfigSchema, validateWorkerConfig } from "./domain/worker-validators.js";

// Presentation helpers (labels and guidance text for TUI)
export {
  type WorkerGuidance,
  formatWorkerLabel,
  getWorkerGuidance,
} from "./domain/worker-presentation.js";

// Agent config specs (shared across install/update/uninstall flows)
export {
  type AgentConfigSpec,
  SUPPORTED_AGENTS,
  BLOCK_START_MARKER,
  BLOCK_END_MARKER,
  agentConfigPath,
  agentConfigDirPath,
  agentLegacyConfigPaths,
} from "./domain/agents.js";

// Error factories
export { workerSkillNotFound } from "./domain/errors.js";

// Usecases
export {
  generateWorkerPrompt,
  type GenerateWorkerPromptResult,
} from "./usecases/generate-worker-prompt.usecase.js";
export { recommendWorkerFit } from "./usecases/worker-fit-recommendation.usecase.js";
export {
  injectAgentBlocks,
  removeAgentBlocks,
  type InjectResult,
  type RemoveResult,
} from "./usecases/manage-agents.usecase.js";

// Lib (instruction-block editors and stream-json parser)
export {
  wrapBlock,
  hasBlock,
  extractBlock,
  injectBlock,
  replaceBlock,
  removeBlock,
  removeLegacyBlock,
} from "./lib/agent-block.js";
export {
  parseRawOutput,
  parseStreamJsonOutput,
  extractStreamJsonLineText,
} from "./lib/stream-json-parser.js";

// Services (composition root helper -- empty for symmetry; worker has no ports)
export { buildWorkerServices } from "./services.js";
export type { WorkerServices } from "./services.js";
