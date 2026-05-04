import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import type { Verdict, VerdictDecision } from "@/features/verdict/domain/types.js";
import { requestVerdict } from "@/features/verdict/index.js";
import { readCiEnv } from "../domain/ci-env.js";
import { runCiVerify } from "../usecases/run-ci-verify.js";

interface CiVerifyCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "verdictStore"
    | "contractVersionStore"
    | "runStateStore"
    | "evidenceStore"
    | "getEffectiveRiskPolicy"
    | "getEffectiveAutopilotPolicy"
    | "getEffectiveReleasePolicy"
    | "computeRisk"
    | "deriveRiskClassFromDiff"
    | "runTrustVerifier"
    | "gitAnchor"
    | "projectRoot"
  >;
}

function exitCodeForDecision(decision: VerdictDecision): number {
  switch (decision) {
    case "PASS": return 0;
    case "FAIL": return 1;
    case "HUMAN": return 2;
    case "BLOCK": return 3;
  }
}

function printVerdict(verdict: Verdict): void {
  console.log(`Decision:   ${verdict.decision}`);
  console.log(`Risk:       ${verdict.effectiveRiskClass}${verdict.proposedRiskClass !== undefined ? ` (proposed: ${verdict.proposedRiskClass})` : ""}`);
  console.log(`ComputedAt: ${verdict.computedAt}`);
  console.log(`Task:       ${verdict.taskId}`);
  console.log(`ID:         ${verdict.id}`);
  if (verdict.reasons.length > 0) {
    console.log("Reasons:");
    for (const r of verdict.reasons) {
      console.log(`  [${r.category}] ${r.code}: ${r.message}`);
    }
  }
  console.log(`Evidence consulted: ${verdict.evidenceConsulted.length}`);
  console.log(`Trust verifier: ${verdict.trustVerifier.findingsCount} findings (${verdict.trustVerifier.errors} errors, ${verdict.trustVerifier.warns} warns, ${verdict.trustVerifier.infos} infos)`);
}

export function registerCiVerifyCommand(
  parent: Command,
  program: Command,
  deps: CiVerifyCommandDeps = { getServices },
): void {
  parent
    .command("verify")
    .description("Run the verdict pipeline in CI mode")
    .requiredOption("--task <id>", "Task ID (one task per PR is the L5 contract)")
    .option("--pr <number>", "PR number (overrides GITHUB_REF / event JSON detection)", parseInt)
    .option("--base <ref>", "Base git ref for the diff (overrides GITHUB_BASE_REF)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;
      const ciEnv = readCiEnv(process.env);

      const verdict = await runCiVerify(
        {
          taskId,
          pr: typeof opts.pr === "number" ? opts.pr : undefined,
          base: typeof opts.base === "string" ? opts.base : undefined,
        },
        {
          env: ciEnv,
          evidenceStore: services.evidenceStore,
          verdict: { request: requestVerdict },
          verdictDeps: {
            contractVersionStore: services.contractVersionStore,
            runStateStore: services.runStateStore,
            evidenceStore: services.evidenceStore,
            verdictStore: services.verdictStore,
            getEffectiveRiskPolicy: services.getEffectiveRiskPolicy,
            getEffectiveAutopilotPolicy: services.getEffectiveAutopilotPolicy,
            getEffectiveReleasePolicy: services.getEffectiveReleasePolicy,
            riskServices: {
              computeRisk: services.computeRisk,
              deriveRiskClassFromDiff: services.deriveRiskClassFromDiff,
            },
            runTrustVerifier: services.runTrustVerifier,
            gitAnchor: services.gitAnchor,
            projectRoot: services.projectRoot,
          },
        },
      );

      if (isJson) {
        console.log(JSON.stringify(verdict, null, 2));
      } else {
        printVerdict(verdict);
      }

      const exitCode = exitCodeForDecision(verdict.decision);
      if (exitCode !== 0) {
        process.exit(exitCode);
      }
    });
}
