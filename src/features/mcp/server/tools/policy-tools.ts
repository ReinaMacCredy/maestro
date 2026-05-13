import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { resolveDefaultBase, resolveHeadSha } from "@/shared/lib/git-base.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { loadSensitivePathsGlobs } from "@/features/policy/index.js";
import { maxRiskClass } from "@/features/risk/index.js";
import {
  readCurrentContractWithBackfill,
  type RiskClass,
} from "@/features/task/index.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { PolicyCheckInput } from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerPolicyTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_policy_check",
    {
      title: "Check policy compliance for a task",
      description:
        "Compute the effective risk class, autopilot rules, and sensitive-path matches for a task's current diff. Read-only.",
      inputSchema: PolicyCheckInput,
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
        const contract = await readCurrentContractWithBackfill(
          services.contractVersionStore,
          services.contractStore,
          args.taskId,
        );
        if (contract === undefined) {
          return toCallToolResult(
            fail("CONTRACT_NOT_FOUND", `No contract found for task ${args.taskId}`, {
              hints: ["Create a contract before running policy_check"],
            }),
          );
        }

        const baseRef = contract.claimedAtCommit ?? (await resolveDefaultBase());
        const headSha = await resolveHeadSha();

        const [changedPaths, riskPolicy, autopilotPolicy, releasePolicy, sensitiveGlobs] =
          await Promise.all([
            services.gitAnchor.collectChangedPaths(services.projectRoot, baseRef, headSha),
            services.getEffectiveRiskPolicy(),
            services.getEffectiveAutopilotPolicy(),
            services.getEffectiveReleasePolicy(),
            loadSensitivePathsGlobs(services.projectRoot),
          ]);

        const derivedRiskResult = services.deriveRiskClassFromDiff(
          { changedPaths, sensitivePathsPolicy: sensitiveGlobs },
          riskPolicy,
        );

        const contractRiskClass: RiskClass = contract.riskClass ?? "medium";
        const effectiveRiskClass = maxRiskClass(contractRiskClass, derivedRiskResult.class);

        const matchedSensitivePaths = sensitiveGlobs.length > 0
          ? changedPaths.filter((p) => matchesAnyGlob(sensitiveGlobs, p))
          : [];

        return toCallToolResult(
          ok({
            taskId: args.taskId,
            contractRiskClass,
            derivedRiskClass: derivedRiskResult.class,
            effectiveRiskClass,
            matchedRiskPolicyRow: derivedRiskResult.matchedRow
              ? {
                  signal: derivedRiskResult.matchedRow.signal,
                  description: derivedRiskResult.matchedRow.description,
                }
              : null,
            autoMergeAllowed: autopilotPolicy.autoMergeAllowed[effectiveRiskClass] ?? false,
            requiredWitnessLevel: autopilotPolicy.requiredWitnessLevel[effectiveRiskClass],
            releaseRules: {
              requireSignedCommits: releasePolicy.requireSignedCommits,
              requireProofMapComplete: releasePolicy.requireProofMapComplete,
            },
            sensitivePaths: {
              globs: sensitiveGlobs,
              matchedPaths: matchedSensitivePaths,
            },
          }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "POLICY_CHECK_FAILED"));
      }
    },
  );
}
