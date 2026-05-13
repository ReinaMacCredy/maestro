import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveDefaultBase as defaultResolveDefaultBase } from "@/shared/lib/git-base.js";
import { type Services } from "@/services.js";
import { recordEvidence as defaultRecordEvidence } from "@/features/evidence/index.js";
import { compareWitnessLevel } from "@/features/evidence/index.js";
import type { DeployReadinessPayload, WitnessLevel, EvidenceStorePort } from "@/features/evidence/index.js";
import { loadOwnersFromBase as defaultLoadOwnersFromBase } from "@/features/policy/index.js";
import type { Owners } from "@/features/policy/index.js";
import type { Spec } from "@/features/spec/index.js";
import type { RecordEvidenceInput } from "@/features/evidence/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import { checkDeployReadiness } from "../usecases/check-deploy-readiness.usecase.js";

export interface DeployGateCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "evidenceStore" | "taskStore" | "specStore" | "projectRoot"
  >;

  readonly recordEvidence?: (
    store: EvidenceStorePort,
    input: RecordEvidenceInput,
  ) => Promise<EvidenceRow>;
  readonly loadOwnersFromBase?: (base: string, projectRoot: string) => Promise<Owners> | Owners;
  readonly resolveDefaultBase?: () => Promise<string>;
  readonly isCI?: () => boolean;
}

const defaultIsCI = (): boolean => process.env.GITHUB_ACTIONS === "true";

export function registerDeployGateCommand(
  parent: Command,
  program: Command,
  deps: DeployGateCommandDeps,
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

      const task = await services.taskStore.get(taskId);
      if (task === undefined) {
        throw new MaestroError(`Task not found: ${taskId}`, [
          "Run `maestro task list` to see available tasks",
        ]);
      }

      const base: string = typeof opts.base === "string" && opts.base.length > 0
        ? opts.base
        : await (deps.resolveDefaultBase ?? defaultResolveDefaultBase)();

      // Rule 12: load owners from base, not PR head, so self-promotion is rejected.
      const owners = await (deps.loadOwnersFromBase ?? defaultLoadOwnersFromBase)(base, services.projectRoot);

      let spec: Spec | undefined;
      if (task.missionId !== undefined) {
        spec = await services.specStore.read(task.missionId);
      }

      const allEvidence = await services.evidenceStore.list({
        task_id: taskId,
        kind: "rollback-exercised",
      });
      const rollbackEvidence = allEvidence.filter(
        (r) =>
          compareWitnessLevel(r.witness_level, "witnessed-by-ci") >= 0 &&
          (r.payload as { exit?: number }).exit === 0,
      );

      const result = checkDeployReadiness({ spec, rollbackEvidence, owners });

      const witnessLevel: WitnessLevel = (deps.isCI ?? defaultIsCI)()
        ? "witnessed-by-ci"
        : "agent-claimed-locally";

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

      const row = await (deps.recordEvidence ?? defaultRecordEvidence)(services.evidenceStore, {
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
          ...failureHint(checks.feature_flag.ok, [
            "    Set spec.rollout_plan.feature_flag in the mission spec",
            `    Edit it: maestro spec edit --mission ${task.missionId ?? "<id>"}`,
          ]),
          `  canary_plan  : ${fmtCheck(checks.canary_plan, checks.canary_plan.stages !== undefined ? `stages=${checks.canary_plan.stages}` : undefined)}`,
          ...failureHint(checks.canary_plan.ok, [
            "    Add at least one stage to spec.rollout_plan.canary.stages",
            `    Edit it: maestro spec edit --mission ${task.missionId ?? "<id>"}`,
          ]),
          `  rollback     : ${fmtCheck(checks.rollback, checks.rollback.witness_evidence_id !== undefined ? `evd=${checks.rollback.witness_evidence_id}` : undefined)}`,
          ...failureHint(checks.rollback.ok, [
            "    Witness a rollback at witnessed-by-ci or stronger before passing this gate",
            `    Run it: maestro deploy rollback --task ${taskId} --command <cmd>`,
          ]),
          `  owner        : ${fmtCheck(checks.owner, checks.owner.approvers !== undefined ? `approvers=[${checks.owner.approvers.join(",")}]` : undefined)}`,
          ...failureHint(checks.owner.ok, [
            "    Add at least one entry to deploy_approver in .maestro/policies/owners.yaml",
            "    Format: docs/owners-yaml-format.md",
          ]),
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

function failureHint(ok: boolean, hints: readonly string[]): readonly string[] {
  return ok ? [] : hints;
}
