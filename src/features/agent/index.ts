export {
  type AgentConfigSpec,
  SUPPORTED_AGENTS,
  REFERENCE_FILE,
  BLOCK_START_MARKER,
  BLOCK_END_MARKER,
  agentConfigPath,
  agentConfigDirPath,
  agentReferencePath,
  agentLegacyConfigPaths,
} from "./domain/agents.js";

export { workerSkillNotFound } from "./domain/errors.js";

export {
  generateWorkerPrompt,
  type GenerateWorkerPromptResult,
  type WorkerPromptStores,
} from "./usecases/generate-worker-prompt.usecase.js";
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
  hasReference,
  injectReference,
  removeReference,
} from "./lib/agent-block.js";
