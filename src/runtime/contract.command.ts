import { Command } from "commander";
import {
  amendContract,
  generateContractAmendmentId,
  getCurrentContract,
} from "@/service/index.js";
import { readContractHistoryWithBackfill } from "@/service/contract-helpers.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import type { Contract, ContractAmendment } from "@/types/contract.js";
import type { Services } from "@/services.js";

interface ContractCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "contractStore" | "contractVersionStore" | "evidenceStore"
  >;
}

function applyPathChanges(
  existing: readonly string[],
  add: readonly string[],
  remove: readonly string[],
): { result: string[]; skipped: string[] } {
  const removeSet = new Set(remove);
  const result = existing.filter((p) => !removeSet.has(p));
  const present = new Set(result);
  const skipped: string[] = [];
  for (const p of add) {
    if (present.has(p) || matchesAnyGlob(result, p)) {
      skipped.push(p);
      continue;
    }
    result.push(p);
    present.add(p);
  }
  return { result, skipped };
}

function collectPath(value: string, previous: readonly string[] | undefined): string[] {
  return [...(previous ?? []), value];
}

function parseVersion(value: string): number {
  const n = Number.parseInt(value, 10);
  if (!Number.isFinite(n) || n < 1) {
    throw new Error(`--version must be a positive integer (got '${value}')`);
  }
  return n;
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
    .command("show <taskId>")
    .description("Show the current contract for a task (or --version N)")
    .option("--version <n>", "show a specific version instead of the current one", parseVersion)
    .option("--json", "Output as JSON")
    .action(async function (
      this: Command,
      taskId: string,
      flags: { version?: number; json?: boolean },
    ): Promise<void> {
      try {
        const services = deps.getServices();
        let match: Contract | undefined;
        let notFoundCode: string;
        let notFoundMsg: string;
        if (flags.version !== undefined) {
          match = await services.contractVersionStore.readVersion(taskId, flags.version);
          notFoundCode = "CONTRACT_VERSION_NOT_FOUND";
          notFoundMsg = `Contract version ${flags.version} not found for task ${taskId}`;
        } else {
          match = await getCurrentContract(
            services.contractVersionStore,
            services.contractStore,
            taskId,
          );
          notFoundCode = "CONTRACT_NOT_FOUND";
          notFoundMsg = `No contract found for task ${taskId}`;
        }
        if (!match) {
          console.error(JSON.stringify({ error: notFoundMsg, code: notFoundCode }));
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
        for (let i = 0; i < versions.length; i++) {
          const c = versions[i];
          const ver = i + 1;
          console.log(
            `v${ver}\t${c.status}\tfilesExpected=${c.scope.filesExpected.length}\tamendments=${c.amendments.length}`,
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
    .option(
      "--add <path>",
      "add a path to filesExpected (repeatable)",
      collectPath as unknown as (v: string, prev: string[]) => string[],
      [] as readonly string[],
    )
    .option(
      "--remove <path>",
      "remove a path from filesExpected (repeatable)",
      collectPath as unknown as (v: string, prev: string[]) => string[],
      [] as readonly string[],
    )
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

        const services = deps.getServices();
        const before = await getCurrentContract(
          services.contractVersionStore,
          services.contractStore,
          taskId,
        );
        if (!before) {
          console.error(
            JSON.stringify({
              error: `No contract found for task ${taskId}`,
              code: "CONTRACT_NOT_FOUND",
            }),
          );
          process.exitCode = 1;
          return;
        }

        const { result: newFilesExpected, skipped: skippedAddPaths } =
          applyPathChanges(before.scope.filesExpected, addPaths, removePaths);

        const beforeSet = new Set(before.scope.filesExpected);
        const afterSet = new Set(newFilesExpected);
        const scopeChanged =
          beforeSet.size !== afterSet.size ||
          [...beforeSet].some((p) => !afterSet.has(p));
        if (!scopeChanged) {
          console.error(
            JSON.stringify({
              error: "No scope changes to apply",
              code: "NO_SCOPE_CHANGES",
            }),
          );
          process.exitCode = 1;
          return;
        }

        const amendment: ContractAmendment = {
          id: generateContractAmendmentId(),
          at: new Date().toISOString(),
          by: flags.by,
          reason: flags.reason,
          before: { scope: before.scope },
          after: {
            scope: {
              filesExpected: newFilesExpected,
              filesForbidden: before.scope.filesForbidden,
            },
          },
        };

        const { newVersion, amendmentId } = await amendContract(
          services.contractVersionStore,
          services.contractStore,
          services.evidenceStore,
          {
            taskId,
            amendment,
            addedPaths: addPaths,
            removedPaths: removePaths,
          },
        );

        const result = {
          amendmentId,
          newVersion,
          ...(skippedAddPaths.length > 0 ? { skippedAddPaths } : {}),
        };
        console.log(JSON.stringify(result, null, 2));
      } catch (err) {
        console.error(`maestro contract amend: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });
}
