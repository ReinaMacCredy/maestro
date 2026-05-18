import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { requestVerdict } from "@/features/verdict/index.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { VerdictRequestInput, VerdictShowInput } from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerVerdictTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_verdict_show",
    {
      title: "Show a verdict",
      description:
        "Show the latest verdict for a task, or a specific verdict version when `id` is provided. Returns code VERDICT_NOT_FOUND when no verdict has been computed. Read-only.",
      inputSchema: VerdictShowInput,
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
        const verdict = args.id
          ? await services.verdictStore.readVersion(args.taskId, args.id)
          : await services.verdictStore.readLatest(args.taskId);
        if (verdict === undefined) {
          return toCallToolResult(
            fail("VERDICT_NOT_FOUND", `No verdict found for task ${args.taskId}`, {
              hints: ["Compute one with maestro_verdict_request"],
            }),
          );
        }
        return toCallToolResult(ok({ verdict }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "VERDICT_SHOW_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_verdict_request",
    {
      title: "Compute a new verdict",
      description:
        "Compute a new verdict for a task and persist it. Returns PASS/FAIL/HUMAN/BLOCK decision and full verdict payload. Each call writes a new verdict row.",
      inputSchema: VerdictRequestInput,
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
        const verdict = await requestVerdict(
          { taskId: args.taskId, base: args.base },
          {
            contractVersionStore: services.contractVersionStore,
            contractStore: services.contractStore,
            runStateStore: services.runStateStore,
            evidenceStore: services.legacyEvidenceStore,
            verdictStore: services.verdictStore,
            specStore: services.trustSpecStore,
            getEffectiveRiskPolicy: services.getEffectiveRiskPolicy,
            getEffectiveAutopilotPolicy: services.getEffectiveAutopilotPolicy,
            getEffectiveReleasePolicy: services.getEffectiveReleasePolicy,
            getEffectiveSensitivePathsGlobs: services.getEffectiveSensitivePathsGlobs,
            riskServices: {
              computeRisk: services.computeRisk,
              deriveRiskClassFromDiff: services.deriveRiskClassFromDiff,
            },
            runTrustVerifier: services.runTrustVerifier,
            gitAnchor: services.gitAnchor,
            projectRoot: services.projectRoot,
          },
        );
        return toCallToolResult(ok({ verdict }));
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        // The verifier shells out to `git rev-parse HEAD^{tree}` for the
        // anchor. On a freshly init'd repo with no commits, git emits an
        // "ambiguous argument" or "unknown revision" error — that's
        // unactionable to an agent. Translate it into a typed code with a
        // clear next step.
        if (/ambiguous argument 'HEAD|unknown revision 'HEAD|Needed a single revision/i.test(message)) {
          return toCallToolResult(fail("NO_COMMITS", "Repository has no commits — cannot anchor a verdict", {
            hints: [
              "Run `git commit` (or stage + commit) so HEAD resolves to a tree",
              "Then re-run `maestro_verdict_request` for this task",
            ],
          }));
        }
        return toCallToolResult(fromMaestroError(err, "VERDICT_REQUEST_FAILED"));
      }
    },
  );
}
