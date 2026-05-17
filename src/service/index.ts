// v2/service: use cases. Allowed to import from types, config, repo.
export * from "./emit-transition-evidence.js";
export * from "./spec-new.usecase.js";
export * from "./spec-validate.usecase.js";
export * from "./task-from-spec.usecase.js";
export * from "./task-claim.usecase.js";
export * from "./task-block.usecase.js";
export * from "./task-abandon.usecase.js";
export * from "./architecture-lint.usecase.js";
export * from "./task-verify.usecase.js";
export * from "./task-ship.usecase.js";
export * from "./plan-from-spec.usecase.js";
export * from "./plan-show.usecase.js";
export * from "./plan-decompose.usecase.js";
export * from "./try-advance-plan.usecase.js";
export * from "./principle-scan.usecase.js";
export * from "./principle-promote.usecase.js";
export * from "./setup-check.usecase.js";
export * from "./check-cost-budget.js";
export * from "./contract-helpers.js";
export * from "./contract-amend.usecase.js";
// Contract helpers re-exported here so MCP tools can import from @/service
// without depending on @/shared/domain/legacy-task.
export { getCurrentContract } from "@/shared/domain/legacy-task/usecases/get-current-contract.usecase.js";
export { buildTaskOwnerId } from "@/shared/domain/legacy-task/usecases/task-continuation.usecase.js";
