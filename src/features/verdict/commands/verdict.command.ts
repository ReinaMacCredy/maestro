import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import { getServices, type Services } from "@/services.js";
import type { Verdict, VerdictDecision } from "../domain/types.js";
import { requestVerdict } from "../usecases/request-verdict.usecase.js";

interface VerdictCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "verdictStore"
    | "contractVersionStore"
    | "evidenceStore"
    | "getRiskPolicy"
    | "getAutopilotPolicy"
    | "getReleasePolicy"
    | "getEffectiveRiskPolicy"
    | "getEffectiveAutopilotPolicy"
    | "getEffectiveReleasePolicy"
    | "computeRisk"
    | "deriveRiskClassFromDiff"
    | "getEffectivePolicies"
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
  if (verdict.policiesConsulted.length > 0) {
    const policyNames = verdict.policiesConsulted.map((p) => p.file).join(", ");
    console.log(`Policies consulted: ${policyNames}`);
  }
  console.log(`Trust verifier: ${verdict.trustVerifier.findingsCount} findings (${verdict.trustVerifier.errors} errors, ${verdict.trustVerifier.warns} warns, ${verdict.trustVerifier.infos} infos)`);
}

export function registerVerdictCommand(
  program: Command,
  deps: VerdictCommandDeps = { getServices },
): void {
  const verdictCmd = program
    .command("verdict")
    .description("Show or request a Verdict for a task");

  verdictCmd
    .command("show")
    .description("Show the current verdict for a task")
    .requiredOption("--task <id>", "Task ID")
    .option("--version <verdictId>", "Show a specific verdict by ID (default: latest)")
    .option("--latest", "Show the latest verdict (default)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      let verdict: Verdict | undefined;

      if (typeof opts.version === "string" && opts.version.length > 0) {
        verdict = await services.verdictStore.readVersion(taskId, opts.version);
        if (verdict === undefined) {
          throw new MaestroError(`Verdict ${opts.version} not found for task ${taskId}`, [
            "Run 'maestro verdict show --task <id>' (without --version) to see the latest",
          ]);
        }
      } else {
        verdict = await services.verdictStore.readLatest(taskId);
        if (verdict === undefined) {
          console.log("No verdict yet. Run 'maestro verdict request --task <id>' to generate one.");
          return;
        }
      }

      if (isJson) {
        console.log(JSON.stringify(verdict, null, 2));
      } else {
        printVerdict(verdict);
      }
    });

  verdictCmd
    .command("request")
    .description("Compute a new Verdict for a task and persist it")
    .requiredOption("--task <id>", "Task ID")
    .option("--base <ref>", "Base git ref for the diff (default: merge-base with main or upstream)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      const verdict = await requestVerdict(
        { taskId, base: typeof opts.base === "string" ? opts.base : undefined },
        {
          contractVersionStore: services.contractVersionStore,
          evidenceStore: services.evidenceStore,
          verdictStore: services.verdictStore,
          getRiskPolicy: services.getRiskPolicy,
          getAutopilotPolicy: services.getAutopilotPolicy,
          getReleasePolicy: services.getReleasePolicy,
          getEffectiveRiskPolicy: services.getEffectiveRiskPolicy,
          getEffectiveAutopilotPolicy: services.getEffectiveAutopilotPolicy,
          getEffectiveReleasePolicy: services.getEffectiveReleasePolicy,
          riskServices: {
            computeRisk: services.computeRisk,
            deriveRiskClassFromDiff: services.deriveRiskClassFromDiff,
            getEffectivePolicies: services.getEffectivePolicies,
          },
          runTrustVerifier: services.runTrustVerifier,
          gitAnchor: services.gitAnchor,
          projectRoot: services.projectRoot,
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
