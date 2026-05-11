export { registerSetupCommand } from "./commands/setup.command.js";
export { detectHostRuntimes } from "./usecases/detect-host-runtime.usecase.js";
export type { DetectedHostRuntime, HostRuntimeId } from "./usecases/detect-host-runtime.usecase.js";
export { installRuntimeHooks } from "./usecases/install-runtime-hooks.usecase.js";
export type { RuntimeHookInstallResult } from "./usecases/install-runtime-hooks.usecase.js";
export {
  checkTocBudget,
  assertTocBudget,
  DEFAULT_TOC_BUDGET,
} from "./usecases/enforce-toc-budget.usecase.js";
export type { TocBudget, TocBudgetReport } from "./usecases/enforce-toc-budget.usecase.js";
export {
  checkSkillBinaryParity,
  renderDriftError,
} from "./usecases/check-skill-binary-parity.usecase.js";
export type {
  CheckSkillBinaryParityArgs,
  SkillBinaryDriftFinding,
  SkillBinaryParityReport,
} from "./usecases/check-skill-binary-parity.usecase.js";
export { auditInstall } from "./usecases/audit-install.usecase.js";
export type { AuditFinding, AuditInstallArgs, AuditInstallReport } from "./usecases/audit-install.usecase.js";
export { runSetupSelfTest } from "./usecases/run-self-test.usecase.js";
export type { SelfTestReport, SelfTestStep } from "./usecases/run-self-test.usecase.js";
