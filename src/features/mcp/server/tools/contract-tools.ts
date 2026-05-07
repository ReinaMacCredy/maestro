import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import {
  amendContract,
  type ContractAmendment,
  generateContractAmendmentId,
  getCurrentContract,
} from "@/features/task/index.js";
import type { Services } from "@/services.js";
import { fail, fromMaestroError, ok, toCallToolResult } from "../errors.js";
import { ContractAmendInput, ContractShowInput } from "../schemas/inputs.js";

interface RegisterDeps {
  readonly getServices: () => Services;
}

export function registerContractTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_contract_show",
    {
      title: "Show a task contract",
      description:
        "Show the current contract for a task, or a specific version when `version` is provided. Returns code CONTRACT_NOT_FOUND when no contract has been proposed. Read-only.",
      inputSchema: ContractShowInput,
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        if (args.version !== undefined) {
          const match = await services.contractVersionStore.readVersion(
            args.taskId,
            args.version,
          );
          if (match === undefined) {
            return toCallToolResult(
              fail(
                "CONTRACT_VERSION_NOT_FOUND",
                `Contract version ${args.version} not found for task ${args.taskId}`,
                ["Inspect history via the CLI: maestro contract history"],
              ),
            );
          }
          return toCallToolResult(ok({ contract: match }));
        }
        const current = await getCurrentContract(
          services.contractVersionStore,
          services.contractStore,
          args.taskId,
        );
        if (current === undefined) {
          return toCallToolResult(
            fail("CONTRACT_NOT_FOUND", `No contract found for task ${args.taskId}`, [
              "Create one with the contract verbs in the CLI",
            ]),
          );
        }
        return toCallToolResult(ok({ contract: current }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "CONTRACT_SHOW_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_contract_amend",
    {
      title: "Amend a task contract scope",
      description:
        "Add or remove paths from filesExpected on the current contract. Records a versioned amendment and a contract-amendment evidence row. Returns code NO_SCOPE_CHANGES if all paths are already covered. Each successful amend creates a new version.",
      inputSchema: ContractAmendInput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const addPaths = args.addPaths ?? [];
        const removePaths = args.removePaths ?? [];
        if (addPaths.length === 0 && removePaths.length === 0) {
          return toCallToolResult(
            fail(
              "VALIDATION_ERROR",
              "At least one of addPaths or removePaths must be provided",
              ["Pass addPaths or removePaths to mutate the contract"],
            ),
          );
        }

        const before = await getCurrentContract(
          services.contractVersionStore,
          services.contractStore,
          args.taskId,
        );
        if (before === undefined) {
          return toCallToolResult(
            fail("CONTRACT_NOT_FOUND", `No contract found for task ${args.taskId}`, [
              "Propose a contract before amending",
            ]),
          );
        }

        const { result: newFilesExpected, skipped: skippedAddPaths } =
          applyPathChanges(before.scope.filesExpected, addPaths, removePaths);

        const scopeChanged =
          newFilesExpected.length !== before.scope.filesExpected.length ||
          newFilesExpected.some((p, i) => p !== before.scope.filesExpected[i]);

        if (!scopeChanged) {
          return toCallToolResult(
            fail("NO_SCOPE_CHANGES", "No scope changes to apply", [
              "All paths are already covered or were absent",
            ]),
          );
        }

        const amendment: ContractAmendment = {
          id: generateContractAmendmentId(),
          at: new Date().toISOString(),
          by: "maestro-mcp",
          reason: args.reason,
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
            taskId: args.taskId,
            amendment,
            addedPaths: addPaths,
            removedPaths: removePaths,
          },
        );

        return toCallToolResult(
          ok({
            amendmentId,
            newVersion,
            skippedAddPaths,
          }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "CONTRACT_AMEND_FAILED"));
      }
    },
  );
}

function applyPathChanges(
  existing: readonly string[],
  add: readonly string[],
  remove: readonly string[],
): { result: string[]; skipped: string[] } {
  const removeSet = new Set(remove);
  const result = existing.filter((p) => !removeSet.has(p));
  const skipped: string[] = [];
  for (const p of add) {
    if (result.includes(p) || matchesAnyGlob(result, p)) {
      skipped.push(p);
      continue;
    }
    result.push(p);
  }
  return { result, skipped };
}
