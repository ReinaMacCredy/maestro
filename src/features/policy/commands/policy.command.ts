import type { Command } from "commander";
import type { Services } from "@/services.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { detectPendingLoosenings as defaultDetectPendingLoosenings } from "../usecases/detect-pending-loosenings.usecase.js";
import type { PendingLoosening } from "../usecases/detect-pending-loosenings.usecase.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { registerPolicyCheckCommand } from "./policy-check.command.js";

interface PolicyPendingDeps {
  readonly getServices: () => Services;
  readonly detectPendingLoosenings?: (opts: { projectRoot: string }) => Promise<readonly PendingLoosening[]>;
}

export function registerPolicyCommand(
  program: Command,
  deps: PolicyPendingDeps,
): void {
  const policyCmd = program
    .command("policy")
    .description("Policy management and inspection");

  registerPolicyPendingCommand(policyCmd, program, deps);
  registerPolicyCheckCommand(policyCmd, program, deps);
}

function registerPolicyPendingCommand(
  parent: Command,
  root: Command,
  deps: PolicyPendingDeps,
): void {
  parent
    .command("pending")
    .description("List loosenings that are pending the 30-day soak window")
    .addHelpText("after", `
Examples:
  maestro policy pending
  maestro policy pending --json
`)
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, root);
      const projectRoot = resolveMaestroProjectRoot(process.cwd());
      const loosenings = await (deps.detectPendingLoosenings ?? defaultDetectPendingLoosenings)({ projectRoot });
      output(isJson, loosenings, formatLoosenings);
    });
}

function formatLoosenings(items: readonly PendingLoosening[]): string[] {
  if (items.length === 0) {
    return ["No pending loosenings."];
  }
  return items.map((l) => {
    const effectiveDate = l.effectiveAt.slice(0, 10);
    return `${l.file}  [${l.kind}]  ${l.edit.description}  (effective ${effectiveDate})`;
  });
}
