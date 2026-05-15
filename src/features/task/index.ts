/**
 * TEMPORARY SHIM -- D-task phase 5.
 *
 * src/features/task/ is deleted except for this file. All content moved to
 * src/shared/domain/legacy-task/. This shim exists solely to keep
 * src/features/mcp/server/tools/{task,handoff,contract}-tools.ts compiling
 * until D-task-MCP rewires them to v2 use cases.
 *
 * Do NOT add new exports here. Do NOT import from this shim in new code.
 * Remove this file when D-task-MCP lands.
 */

// Symbols used by task-tools.ts
export {
  blockTasks,
  claimTask,
  createTask,
  listTasks,
  planTasks,
  unblockTasks,
  updateTask,
} from "@/shared/domain/legacy-task/index.js";
export type {
  BatchInput,
  ListTasksFilters,
} from "@/shared/domain/legacy-task/index.js";

// Symbol used by handoff-tools.ts
export { buildTaskOwnerId } from "@/shared/domain/legacy-task/index.js";

// Symbols used by contract-tools.ts
export {
  amendContract,
  generateContractAmendmentId,
  getCurrentContract,
} from "@/shared/domain/legacy-task/index.js";
// ContractAmendment is canonical in @/v2/types/contract.js
export type { ContractAmendment } from "@/v2/types/contract.js";
