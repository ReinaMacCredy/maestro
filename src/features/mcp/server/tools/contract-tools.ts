import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import {
  amendContract,
  generateContractAmendmentId,
  getCurrentContract,
} from "@/service/index.js";
import type { ContractAmendment } from "@/types/contract.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { ContractAmendInput, ContractShowInput } from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

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
    async (args): Promise<CallToolResult> => {
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
                { hints: ["Inspect history via the CLI: maestro contract history"] },
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
            fail("CONTRACT_NOT_FOUND", `No contract found for task ${args.taskId}`, {
              hints: ["Create one with the contract verbs in the CLI"],
            }),
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
        "Add or remove paths from filesExpected on the current contract. Records a versioned amendment and a contract-amendment evidence row. Error codes: CONTRACT_NOT_FOUND, NO_SCOPE_CHANGES, VALIDATION_ERROR. Each successful amend creates a new version.",
      inputSchema: ContractAmendInput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const addPaths = args.addPaths ?? [];
        const removePaths = args.removePaths ?? [];
        if (addPaths.length === 0 && removePaths.length === 0) {
          return toCallToolResult(
            fail(
              "VALIDATION_ERROR",
              "At least one of addPaths or removePaths must be provided",
              { hints: ["Pass addPaths or removePaths to mutate the contract"] },
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
            fail("CONTRACT_NOT_FOUND", `No contract found for task ${args.taskId}`, {
              hints: ["Propose a contract before amending"],
            }),
          );
        }

        const { result: newFilesExpected, skipped: skippedAddPaths } =
          applyPathChanges(before.scope.filesExpected, addPaths, removePaths);

        // Compare as sets so simultaneous remove+re-add of the same path
        // (which reorders the array but leaves the semantic scope intact)
        // doesn't trigger a spurious amendment.
        const beforeSet = new Set(before.scope.filesExpected);
        const afterSet = new Set(newFilesExpected);
        const scopeChanged =
          beforeSet.size !== afterSet.size ||
          [...beforeSet].some((p) => !afterSet.has(p));

        if (!scopeChanged) {
          return toCallToolResult(
            fail("NO_SCOPE_CHANGES", "No scope changes to apply", {
              hints: ["All paths are already covered or were absent"],
            }),
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
            ...(skippedAddPaths.length > 0 ? { skippedAddPaths } : {}),
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
