import { createHash } from "node:crypto";
import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { recordEvidence } from "@/features/evidence/index.js";
import { deriveRiskClassFromDiff } from "@/features/risk/index.js";
import { checkPlan } from "../usecases/check-plan.js";
import type { PlanCheckFinding, PlanCheckResult, PlanInput } from "../domain/types.js";
import type { PlanCheckPayload } from "@/features/evidence/index.js";

interface PlanCheckCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "contractVersionStore" | "evidenceStore" | "specStore"
  >;
}

export function registerPlanCheckCommand(
  parent: Command,
  program: Command,
  deps: PlanCheckCommandDeps = { getServices },
): void {
  parent
    .command("check")
    .description("Check a plan file against the contract, spec, and risk class")
    .requiredOption("--task <id>", "Task ID")
    .requiredOption("--plan-file <path>", "Path to the plan file (JSON or YAML)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;
      const planFilePath: string = opts.planFile;

      // Load the plan file
      const planText = await readText(planFilePath);
      if (planText === undefined) {
        throw new MaestroError(`Plan file not found: ${planFilePath}`, [
          "Check the path and re-run",
        ]);
      }

      const plan = parseYaml<PlanInput>(planText);

      // Compute plan file SHA-256
      const planFileSha = createHash("sha256").update(planText).digest("hex");

      // Load the contract
      const contract = await services.contractVersionStore.readCurrent(taskId);
      if (contract === undefined) {
        throw new MaestroError(`No contract found for task: ${taskId}`, [
          "Run `maestro contract show --task <id>` to inspect the contract",
        ]);
      }

      // Load spec if the contract references a mission
      const spec = contract.missionId !== undefined
        ? await services.specStore.read(contract.missionId)
        : undefined;

      // Derive risk class from the plan's intended files
      const derived = deriveRiskClassFromDiff({ changedPaths: plan.intendedFiles });

      // Run the plan check
      const result: PlanCheckResult = checkPlan({
        plan,
        contract,
        spec,
        derivedRiskClass: derived.class,
      });

      // Record an evidence row of kind "plan-check"
      const evidencePayload: PlanCheckPayload = {
        planFileSha,
        findings: result.findings.map((f) => ({
          check: f.check,
          severity: f.severity,
          message: f.message,
        })),
        errorCount: result.errorCount,
        warnCount: result.warnCount,
      };

      await recordEvidence(services.evidenceStore, {
        task_id: taskId,
        kind: "plan-check",
        payload: evidencePayload,
        witness_level: "agent-claimed-locally",
      });

      // Output findings
      if (isJson) {
        console.log(JSON.stringify(result, null, 2));
      } else {
        printFindings(result);
      }
      // Exit code always 0 — agents react to findings
    });
}

function printFindings(result: PlanCheckResult): void {
  const { findings, errorCount, warnCount } = result;
  if (findings.length === 0) {
    console.log("[ok] Plan check passed — no findings.");
    return;
  }

  console.log(`Plan check: ${errorCount} error(s), ${warnCount} warning(s)`);
  console.log("");

  for (const finding of findings) {
    const prefix = finding.severity === "error" ? "[!]" : finding.severity === "warn" ? "[~]" : "[i]";
    console.log(`${prefix} ${finding.check}: ${finding.message}`);
    if (finding.paths && finding.paths.length > 0) {
      for (const p of finding.paths) {
        console.log(`      ${p}`);
      }
    }
    if (finding.criterionIds && finding.criterionIds.length > 0) {
      for (const id of finding.criterionIds) {
        console.log(`      criterion: ${id}`);
      }
    }
  }
}

export type { PlanCheckFinding };
