import type { Command } from "commander";
import type { Services } from "@/services.js";
import { registerDeployRollbackCommand } from "./commands/deploy-rollback.command.js";
import { registerDeployGateCommand, type DeployGateCommandDeps } from "./commands/deploy-gate.command.js";

export { registerDeployRollbackCommand } from "./commands/deploy-rollback.command.js";
export { registerDeployGateCommand } from "./commands/deploy-gate.command.js";
export { buildDeployServices } from "./services.js";
export type { DeployServices } from "./services.js";

interface RegisterDeployDeps {
  readonly getServices: () => Services;
}

/**
 * Registers the `deploy` parent command with all subcommands (`rollback`, `gate`).
 * `rootProgram` is the Commander root (used for --json flag resolution by subcommands).
 */
export function registerDeployCommand(
  deployCmd: Command,
  rootProgram: Command,
  deps: RegisterDeployDeps,
): void {
  registerDeployRollbackCommand(deployCmd, rootProgram, deps);
  registerDeployGateCommand(deployCmd, rootProgram, deps as DeployGateCommandDeps);
}
