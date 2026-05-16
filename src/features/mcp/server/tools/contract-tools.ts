import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { getCurrentContract } from "@/service/index.js";
import { amendContractScope } from "@/service/contract-amend.usecase.js";
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

        const outcome = await amendContractScope(deps.getServices(), {
          taskId: args.taskId,
          addPaths,
          removePaths,
          reason: args.reason,
          by: "maestro-mcp",
        });

        if (outcome.kind === "no-contract") {
          return toCallToolResult(
            fail("CONTRACT_NOT_FOUND", `No contract found for task ${args.taskId}`, {
              hints: ["Propose a contract before amending"],
            }),
          );
        }
        if (outcome.kind === "no-changes") {
          return toCallToolResult(
            fail("NO_SCOPE_CHANGES", "No scope changes to apply", {
              hints: ["All paths are already covered or were absent"],
            }),
          );
        }

        return toCallToolResult(
          ok({
            amendmentId: outcome.amendmentId,
            newVersion: outcome.newVersion,
            ...(outcome.skippedAddPaths.length > 0
              ? { skippedAddPaths: outcome.skippedAddPaths }
              : {}),
          }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "CONTRACT_AMEND_FAILED"));
      }
    },
  );
}

