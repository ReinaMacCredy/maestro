import type { Command } from "commander";
import type { Services } from "@/services.js";
import { registerDeployRollbackCommand } from "./commands/deploy-rollback.command.js";
import { registerDeployGateCommand } from "./commands/deploy-gate.command.js";

export { registerDeployRollbackCommand } from "./commands/deploy-rollback.command.js";
export { registerDeployGateCommand } from "./commands/deploy-gate.command.js";
export { buildDeployServices } from "./services.js";
export type { DeployServices } from "./services.js";

export function registerDeployCommand(
  deployCmd: Command,
  rootProgram: Command,
  deps: { readonly getServices: () => Services },
): void {
  registerDeployRollbackCommand(deployCmd, rootProgram, deps);
  registerDeployGateCommand(deployCmd, rootProgram, deps);
}
