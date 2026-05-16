import { createHash } from "node:crypto";
import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import { recordEvidence } from "@/features/evidence/index.js";
import { deriveRiskClassFromDiff } from "@/features/risk/index.js";
import { readCurrentContractWithBackfill, readDraftContract } from "@/service/contract-helpers.js";
import { checkPlan } from "../usecases/check-plan.js";
import { validatePlanInput } from "../domain/plan-validators.js";
import type { PlanCheckFinding, PlanCheckResult } from "../domain/types.js";
import type { PlanCheckPayload } from "@/features/evidence/index.js";

interface PlanCheckCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "contractVersionStore" | "contractStore" | "evidenceStore" | "specStore"
  >;
}

export function registerPlanCheckCommand(
  parent: Command,
  program: Command,
  deps: PlanCheckCommandDeps,
): void {
  parent
    .command("check")
    .description("Check a plan file against the contract, spec, and risk class")
    .requiredOption("--task <id>", "Task ID")
    .requiredOption("--plan-file <path>", "Path to the plan file (JSON or YAML)")
    .option("--json", "Output as JSON")
    .addHelpText("after", `
Plan file shape (YAML or JSON):
  intendedFiles: [<path>, ...]                       # required, paths the plan touches
  proofSet:                                          # required, may be empty
    - criterionId: <id>
      evidenceKinds: [command|manual-note|...]
  riskClass: <low|medium|high|critical>              # required
  notes: <string>                                    # optional

Example:
  intendedFiles: [src/foo.ts]
  proofSet: []
  riskClass: medium

Checks (exit code is always 0; agents react to findings):
  scope-widens         Plan touches paths outside the contract scope
  missing-proof        Plan claims a criterion is met but no evidence row exists
                       (only fires when the contract is linked to a mission spec)
  risk-class-too-low   Plan declares a class lower than what the diff implies
`)
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;
      const planFilePath: string = opts.planFile;

      let planText: string | undefined;
      try {
        planText = await readText(planFilePath);
      } catch (err: unknown) {
        const code = (err as NodeJS.ErrnoException).code;
        if (code === "EISDIR") {
          throw new MaestroError(`Plan file is a directory: ${planFilePath}`, [
            "Pass a path to a YAML or JSON file, not a directory",
          ]);
        }
        const msg = err instanceof Error ? err.message : String(err);
        throw new MaestroError(`Cannot read plan file: ${planFilePath}`, [msg]);
      }
      if (planText === undefined) {
        throw new MaestroError(`Plan file not found: ${planFilePath}`, [
          "Check the path and re-run",
        ]);
      }

      let planRaw: unknown;
      try {
        planRaw = parseYaml<unknown>(planText);
      } catch (err: unknown) {
        const yamlErr = err as { linePos?: Array<{ line: number; col: number }> };
        const pos = yamlErr.linePos?.[0];
        const where = pos !== undefined ? ` at line ${pos.line}, col ${pos.col}` : "";
        const msg = err instanceof Error ? err.message : String(err);
        throw new MaestroError(
          `Plan file is not valid YAML/JSON: ${planFilePath}${where}`,
          [msg, "Fix the syntax and re-run `maestro plan check`"],
        );
      }
      const plan = validatePlanInput(planRaw, planFilePath);
      const planFileSha = createHash("sha256").update(planText).digest("hex");

      const contract = await readCurrentContractWithBackfill(
        services.contractVersionStore,
        services.contractStore,
        taskId,
      );
      if (contract === undefined) {
        const draft = await readDraftContract(services.contractStore, taskId);
        if (draft !== undefined) {
          throw new MaestroError(
            `Contract ${draft.id} for task ${taskId} is in draft status — lock it first`,
            [`maestro task contract lock ${taskId}`],
          );
        }
        throw new MaestroError(`No contract found for task: ${taskId}`, [
          "Run `maestro contract show --task <id>` to inspect the contract",
        ]);
      }

      const spec = contract.missionId !== undefined
        ? await services.specStore.read(contract.missionId)
        : undefined;

      const derived = deriveRiskClassFromDiff({ changedPaths: plan.intendedFiles });

      const result: PlanCheckResult = checkPlan({
        plan,
        contract,
        spec,
        derivedRiskClass: derived.class,
      });

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

      if (isJson) {
        console.log(JSON.stringify(result, null, 2));
      } else {
        printFindings(result);
      }
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
