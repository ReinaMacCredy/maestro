import { Command } from "commander";
import { getCurrentContract } from "@/service/index.js";
import { amendContractScope } from "@/service/contract-amend.usecase.js";
import { readContractHistoryWithBackfill } from "@/service/contract-helpers.js";
import { parsePositiveInt } from "@/shared/lib/cli-options.js";
import type { Contract } from "@/types/contract.js";
import type { Services } from "@/services.js";

interface ContractCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "contractStore" | "contractVersionStore" | "evidenceStore"
  >;
}

function collectPath(value: string, previous: readonly string[]): string[] {
  return [...previous, value];
}

export function registerContractCommands(
  program: Command,
  deps: ContractCommandDeps,
): void {
  const contract = program
    .command("contract")
    .description("Inspect and amend task contracts")
    .option("--json", "Output as JSON");

  contract
    .command("show [taskId]")
    .description("Show the current contract for a task (or --version N). Accepts task id as positional or via --task for consistency with sibling verbs.")
    .option("--task <id>", "task id (alternative to positional)")
    .option("--version <n>", "show a specific version instead of the current one", parsePositiveInt)
    .action(async function (
      this: Command,
      taskIdArg: string | undefined,
      flags: { task?: string; version?: number },
    ): Promise<void> {
      try {
        const taskId = taskIdArg ?? flags.task;
        if (taskId === undefined || taskId.length === 0) {
          console.error("maestro contract show: task id required (positional <taskId> or --task <id>)");
          process.exitCode = 1;
          return;
        }
        const services = deps.getServices();
        const match: Contract | undefined =
          flags.version !== undefined
            ? await services.contractVersionStore.readVersion(taskId, flags.version)
            : await getCurrentContract(
                services.contractVersionStore,
                services.contractStore,
                taskId,
              );
        if (!match) {
          const code =
            flags.version !== undefined ? "CONTRACT_VERSION_NOT_FOUND" : "CONTRACT_NOT_FOUND";
          const error =
            flags.version !== undefined
              ? `Contract version ${flags.version} not found for task ${taskId}`
              : `No contract found for task ${taskId}`;
          console.error(JSON.stringify({ error, code }));
          process.exitCode = 1;
          return;
        }
        console.log(JSON.stringify({ contract: match }, null, 2));
      } catch (err) {
        console.error(`maestro contract show: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });

  contract
    .command("history <taskId>")
    .description("List every recorded version of the contract for a task")
    .option("--json", "Output as JSON")
    .action(async function (
      this: Command,
      taskId: string,
      flags: { json?: boolean },
    ): Promise<void> {
      try {
        const services = deps.getServices();
        const versions = await readContractHistoryWithBackfill(
          services.contractVersionStore,
          services.contractStore,
          taskId,
        );
        const wantJson =
          flags.json === true || this.optsWithGlobals().json === true;
        if (wantJson) {
          console.log(JSON.stringify({ taskId, versions }, null, 2));
          return;
        }
        if (versions.length === 0) {
          console.log(`No contract history for ${taskId}`);
          return;
        }
        for (const [i, c] of versions.entries()) {
          console.log(
            `v${i + 1}\t${c.status}\tfilesExpected=${c.scope.filesExpected.length}\tamendments=${c.amendments.length}`,
          );
        }
      } catch (err) {
        console.error(`maestro contract history: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });

  contract
    .command("amend <taskId>")
    .description(
      "Add or remove paths on the current contract's filesExpected (versioned + evidence)",
    )
    .option("--add <path>", "add a path to filesExpected (repeatable)", collectPath, [])
    .option("--remove <path>", "remove a path from filesExpected (repeatable)", collectPath, [])
    .requiredOption("--reason <text>", "human-readable explanation of the amendment")
    .option("--by <actor>", "actor id recorded on the amendment", "maestro-cli")
    .option("--json", "Output as JSON")
    .action(async function (
      this: Command,
      taskId: string,
      flags: {
        add?: readonly string[];
        remove?: readonly string[];
        reason: string;
        by: string;
        json?: boolean;
      },
    ): Promise<void> {
      try {
        const addPaths = flags.add ?? [];
        const removePaths = flags.remove ?? [];
        if (addPaths.length === 0 && removePaths.length === 0) {
          console.error(
            "maestro contract amend: at least one of --add or --remove is required",
          );
          process.exitCode = 1;
          return;
        }

        const outcome = await amendContractScope(deps.getServices(), {
          taskId,
          addPaths,
          removePaths,
          reason: flags.reason,
          by: flags.by,
        });

        if (outcome.kind === "no-contract") {
          console.error(
            JSON.stringify({
              error: `No contract found for task ${taskId}`,
              code: "CONTRACT_NOT_FOUND",
            }),
          );
          process.exitCode = 1;
          return;
        }
        if (outcome.kind === "no-changes") {
          console.error(
            JSON.stringify({
              error: "No scope changes to apply",
              code: "NO_SCOPE_CHANGES",
            }),
          );
          process.exitCode = 1;
          return;
        }

        console.log(
          JSON.stringify(
            {
              amendmentId: outcome.amendmentId,
              newVersion: outcome.newVersion,
              ...(outcome.skippedAddPaths.length > 0
                ? { skippedAddPaths: outcome.skippedAddPaths }
                : {}),
            },
            null,
            2,
          ),
        );
      } catch (err) {
        console.error(`maestro contract amend: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });
}
