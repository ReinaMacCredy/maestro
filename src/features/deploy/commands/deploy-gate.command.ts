import { execFileSync } from "node:child_process";
import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveDefaultBase } from "@/shared/lib/git-base.js";
import { getServices, type Services } from "@/services.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { DeployReadinessPayload, WitnessLevel, EvidenceStorePort } from "@/features/evidence/index.js";
import { parseOwners, OWNERS_REL_PATH } from "@/features/policy/index.js";
import type { Owners } from "@/features/policy/index.js";
import type { Spec } from "@/features/spec/index.js";
import type { RecordEvidenceInput } from "@/features/evidence/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import { checkDeployReadiness } from "../usecases/check-deploy-readiness.usecase.js";

// Rule 12 pattern: load owners from base branch, not PR head.
function loadOwnersFromBase(base: string, projectRoot: string): Owners {
  let text: string;
  try {
    text = execFileSync(
      "git",
      ["show", `${base}:${OWNERS_REL_PATH}`],
      { cwd: projectRoot, encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] },
    ).trim();
  } catch {
    throw new MaestroError(
      `owners.yaml not found at ${base}:${OWNERS_REL_PATH}`,
      ["Run 'maestro init' to scaffold it, or check the base ref is correct"],
    );
  }
  return parseOwners(text);
}

export interface DeployGateCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "evidenceStore" | "taskStore" | "specStore" | "projectRoot"
  >;

  readonly recordEvidence: (
    store: EvidenceStorePort,
    input: RecordEvidenceInput,
  ) => Promise<EvidenceRow>;
  readonly loadOwnersFromBase: (base: string, projectRoot: string) => Owners;
  readonly resolveDefaultBase: () => Promise<string>;
  readonly isCI: () => boolean;
}

const defaultDeps: DeployGateCommandDeps = {
  getServices,
  recordEvidence,
  loadOwnersFromBase,
  resolveDefaultBase,
  isCI: () => process.env.GITHUB_ACTIONS === "true",
};

export function registerDeployGateCommand(
  parent: Command,
  program: Command,
  deps: DeployGateCommandDeps = defaultDeps,
): void {
  parent
    .command("gate")
    .description("Run deploy-readiness checks and record a deploy-readiness Evidence row")
    .requiredOption("--task <id>", "Task to check deploy readiness for")
    .option("--base <ref>", "Base git ref for loading owners.yaml (default: merge-base with upstream or main)")
    .option("--json", "Output as JSON")
    .action(async (opts: { task: string; base?: string; json?: boolean }): Promise<void> => {
      const { task: taskId } = opts;
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      // Resolve task
      const task = await services.taskStore.get(taskId);
      if (task === undefined) {
        throw new MaestroError(`Task not found: ${taskId}`, [
          "Run `maestro task list` to see available tasks",
        ]);
      }

      // Resolve base ref
      const base: string = typeof opts.base === "string" && opts.base.length > 0
        ? opts.base
        : await deps.resolveDefaultBase();

      // Load owners from base (Rule 12)
      const owners = deps.loadOwnersFromBase(base, services.projectRoot);

      // Load spec if task has missionId
      let spec: Spec | undefined;
      if (task.missionId !== undefined) {
        spec = await services.specStore.read(task.missionId);
      }

      // Collect rollback-exercised evidence at witnessed-by-ci or stronger
      const allEvidence = await services.evidenceStore.list({
        task_id: taskId,
        kind: "rollback-exercised",
      });
      const rollbackEvidence = allEvidence.filter(
        (r) => r.witness_level === "witnessed-by-ci" || r.witness_level === "witnessed-by-maestro",
      );

      // Run the pure use case
      const result = checkDeployReadiness({ spec, rollbackEvidence, owners });

      // Determine witness level
      const witnessLevel: WitnessLevel = deps.isCI()
        ? "witnessed-by-ci"
        : "agent-claimed-locally";

      // Write deploy-readiness Evidence row
      const payload: DeployReadinessPayload = {
        task_id: taskId,
        checks: {
          feature_flag: result.feature_flag,
          canary_plan: result.canary_plan,
          rollback: result.rollback,
          owner: result.owner,
        },
        gate: result.gate,
      };

      const row = await deps.recordEvidence(services.evidenceStore, {
        task_id: taskId,
        kind: "deploy-readiness",
        payload,
        witness_level: witnessLevel,
      });

      const checks = result;
      output(isJson, {
        task_id: taskId,
        evidence_id: row.id,
        gate: result.gate,
        witness_level: witnessLevel,
        checks: {
          feature_flag: checks.feature_flag,
          canary_plan: checks.canary_plan,
          rollback: checks.rollback,
          owner: checks.owner,
        },
      }, (r) => {
        const icon = r.gate === "pass" ? "[ok]" : "[!]";
        const lines = [
          `${icon} deploy gate: ${r.gate.toUpperCase()}`,
          `  Evidence: ${r.evidence_id}`,
          `  Task:     ${r.task_id}`,
          `  Witness:  ${r.witness_level}`,
          "",
          `  feature_flag : ${fmtCheck(checks.feature_flag, checks.feature_flag.value !== undefined ? `flag="${checks.feature_flag.value}"` : undefined)}`,
          `  canary_plan  : ${fmtCheck(checks.canary_plan, checks.canary_plan.stages !== undefined ? `stages=${checks.canary_plan.stages}` : undefined)}`,
          `  rollback     : ${fmtCheck(checks.rollback, checks.rollback.witness_evidence_id !== undefined ? `evd=${checks.rollback.witness_evidence_id}` : undefined)}`,
          `  owner        : ${fmtCheck(checks.owner, checks.owner.approvers !== undefined ? `approvers=[${checks.owner.approvers.join(",")}]` : undefined)}`,
        ];
        return lines;
      });

      if (result.gate === "fail") {
        process.exit(1);
      }
    });
}

function fmtCheck(check: { ok: boolean }, detail: string | undefined): string {
  const status = check.ok ? "pass" : "fail";
  return detail !== undefined ? `${status}  (${detail})` : status;
}
