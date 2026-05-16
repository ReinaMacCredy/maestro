import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  principlePromote,
  CorrectionNotFoundError,
  CorrectionNotLintViolationError,
} from "@/service/index.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import {
  PrinciplePromoteInput,
} from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerPrincipleTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_principle_promote",
    {
      title: "Promote a lint-violation evidence row to a principle",
      description:
        "Promote a lint-violation evidence row to a docs/principles/<slug>.md file. The correction_id must reference an evidence row of kind 'lint-violation'. The slug is derived from the rule_id. Error codes: CORRECTION_NOT_FOUND, CORRECTION_NOT_LINT_VIOLATION, PRINCIPLE_PROMOTE_FAILED.",
      inputSchema: PrinciplePromoteInput,
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
        const result = await principlePromote(
          {
            evidenceStore: services.v2.evidenceStore,
            principlesStore: services.v2.principlesStore,
          },
          { correction_id: args.correction_id },
        );
        return toCallToolResult(ok(result));
      } catch (err) {
        if (err instanceof CorrectionNotFoundError) {
          return toCallToolResult(
            fail("CORRECTION_NOT_FOUND", err.message, {
              hints: ["Use maestro_evidence_list to find lint-violation rows"],
            }),
          );
        }
        if (err instanceof CorrectionNotLintViolationError) {
          return toCallToolResult(
            fail("CORRECTION_NOT_LINT_VIOLATION", err.message, {
              hints: ["Only evidence rows of kind 'lint-violation' can be promoted"],
            }),
          );
        }
        return toCallToolResult(fromMaestroError(err, "PRINCIPLE_PROMOTE_FAILED"));
      }
    },
  );
}
