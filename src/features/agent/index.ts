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
  agentSkillsRoot,
} from "./domain/agents.js";

export {
  generateAgentPrompt,
  type GenerateAgentPromptResult,
} from "./usecases/generate-agent-prompt.usecase.js";

export {
  injectAgentBlocks,
  removeAgentBlocks,
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
