export type {
  WorkerConfig,
  WorkerOutputMode,
  SupervisionLevel,
  CliWorkerConfig,
  ExecutionConfig,
  SupervisionConfig,
  ParallelConfig,
} from "./domain/worker-types.js";

export { WorkerConfigSchema, validateWorkerConfig } from "./domain/worker-validators.js";

export {
  type WorkerGuidance,
  formatWorkerLabel,
  getWorkerGuidance,
} from "./domain/worker-presentation.js";

export {
  type AgentConfigSpec,
  SUPPORTED_AGENTS,
  BLOCK_START_MARKER,
  BLOCK_END_MARKER,
  agentConfigPath,
  agentConfigDirPath,
  agentLegacyConfigPaths,
} from "./domain/agents.js";

export { workerSkillNotFound } from "./domain/errors.js";

export {
  generateWorkerPrompt,
  type GenerateWorkerPromptResult,
  type WorkerPromptStores,
} from "./usecases/generate-worker-prompt.usecase.js";
export { recommendWorkerFit } from "./usecases/worker-fit-recommendation.usecase.js";
export {
  injectAgentBlocks,
  removeAgentBlocks,
  type InjectResult,
  type RemoveResult,
} from "./usecases/manage-agents.usecase.js";

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
