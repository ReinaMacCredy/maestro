import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { checkCostBudget } from "../usecases/check-cost-budget.js";
import { readCurrentContractWithBackfill } from "../usecases/read-current-contract-with-backfill.js";

interface TaskBudgetDeps {
  readonly getServices: () => Pick<
    Services,
    "contractVersionStore" | "contractStore" | "runStateStore"
  >;
}

export function registerTaskBudgetCommand(
  taskCmd: Command,
  program: Command,
  deps: TaskBudgetDeps = { getServices },
): void {
  taskCmd
    .command("budget")
    .description("Show cost-budget consumption against the contract limits")
    .requiredOption("--task <id>", "Task id")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      const contract = await readCurrentContractWithBackfill(
        services.contractVersionStore,
        services.contractStore,
        taskId,
      );
      if (contract === undefined) {
        console.log(`No contract for task ${taskId}`);
        return;
      }

      const runState = await services.runStateStore.read(taskId);
      const budgetCheck = checkCostBudget(contract, runState);

      const retryCount = runState?.retryCount ?? 0;
      const wallClockElapsedSeconds = runState?.wallClockElapsedSeconds ?? 0;
      const tokensUsed = runState?.tokensUsed;

      const budget = contract.costBudget;
      const hasBudget = budget !== undefined;
      const maxRetries = budget?.maxRetries;
      const maxWallClockSeconds = budget?.maxWallClockSeconds;
      const maxTokens = budget?.maxTokens;

      if (isJson) {
        const jsonOut: Record<string, unknown> = {
          taskId,
          hasBudget,
          retryCount,
          wallClockElapsedSeconds,
          tokensUsed,
          exhausted: budgetCheck.exhausted,
        };
        if (maxRetries !== undefined) {
          jsonOut["maxRetries"] = maxRetries;
        }
        if (maxWallClockSeconds !== undefined) {
          jsonOut["maxWallClockSeconds"] = maxWallClockSeconds;
        }
        if (budgetCheck.reason !== undefined) {
          jsonOut["reason"] = budgetCheck.reason;
        }
        if (maxTokens !== undefined) {
          jsonOut["maxTokens"] = maxTokens;
        }
        process.stdout.write(JSON.stringify(jsonOut) + "\n");
        return;
      }

      // Text mode: small table
      console.log(`Budget for task ${taskId}:`);
      if (!hasBudget) {
        console.log("  (no costBudget set on contract — no limits enforced)");
        console.log(`  Retries:    ${retryCount} (no limit)`);
        console.log(`  Wall clock: ${wallClockElapsedSeconds}s (no limit)`);
        console.log("  Set limits via costBudget in the contract draft template:");
        console.log("    costBudget: { maxRetries: 3, maxWallClockSeconds: 1800 }");
        return;
      }
      console.log(`  Retries:    ${retryCount}/${maxRetries}`);
      console.log(`  Wall clock: ${wallClockElapsedSeconds}s/${maxWallClockSeconds}s`);
      if (maxTokens !== undefined) {
        console.log(`  Tokens:     ${tokensUsed ?? 0}/${maxTokens}`);
      }
      if (budgetCheck.exhausted) {
        console.log(`  Exhausted:  yes (${budgetCheck.reason})`);
      } else {
        console.log(`  Exhausted:  no`);
      }
    });
}
