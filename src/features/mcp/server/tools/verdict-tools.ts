import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { requestVerdict } from "@/features/verdict/index.js";
import type { Services } from "@/services.js";
import { fail, fromMaestroError, ok, toCallToolResult } from "../errors.js";
import { VerdictRequestInput, VerdictShowInput } from "../schemas/inputs.js";

interface RegisterDeps {
  readonly getServices: () => Services;
}

export function registerVerdictTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "verdict_show",
    {
      title: "Show a verdict",
      description:
        "Show the latest verdict for a task, or a specific verdict version when `id` is provided.",
      inputSchema: VerdictShowInput,
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const verdict = args.id
          ? await services.verdictStore.readVersion(args.taskId, args.id)
          : await services.verdictStore.readLatest(args.taskId);
        if (verdict === undefined) {
          return toCallToolResult(
            fail("VERDICT_NOT_FOUND", `No verdict found for task ${args.taskId}`, [
              "Compute one with verdict_request",
            ]),
          );
        }
        return toCallToolResult(ok({ verdict }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "VERDICT_SHOW_FAILED"));
      }
    },
  );

  server.registerTool(
    "verdict_request",
    {
      title: "Compute a new verdict",
      description:
        "Compute a new verdict for a task and persist it. Returns PASS/FAIL/HUMAN/BLOCK decision and full verdict payload.",
      inputSchema: VerdictRequestInput,
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const verdict = await requestVerdict(
          { taskId: args.taskId, base: args.base },
          {
            contractVersionStore: services.contractVersionStore,
            contractStore: services.contractStore,
            runStateStore: services.runStateStore,
            evidenceStore: services.evidenceStore,
            verdictStore: services.verdictStore,
            specStore: services.specStore,
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
        return toCallToolResult(fromMaestroError(err, "VERDICT_REQUEST_FAILED"));
      }
    },
  );
}
