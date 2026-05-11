import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import { recordEvidence as defaultRecordEvidence } from "@/features/evidence/index.js";
import type { RollbackExercisedPayload, WitnessLevel } from "@/features/evidence/index.js";

interface DeployRollbackCommandDeps {
  readonly getServices: () => Pick<Services, "evidenceStore" | "taskStore">;
  readonly recordEvidence?: typeof defaultRecordEvidence;
  readonly spawnSync?: (cmd: string) => { exitCode: number };
  readonly isCI?: () => boolean;
}

function defaultSpawnSync(cmd: string): { exitCode: number } {
  const result = Bun.spawnSync(["sh", "-c", cmd], { stderr: "inherit", stdout: "inherit" });
  return { exitCode: result.exitCode ?? 1 };
}

const defaultIsCI = (): boolean => process.env.GITHUB_ACTIONS === "true";

export function registerDeployRollbackCommand(
  parent: Command,
  program: Command,
  deps: DeployRollbackCommandDeps,
): void {
  parent
    .command("rollback")
    .description("Run a rollback command and record a witnessed rollback-exercised Evidence row")
    .requiredOption("--task <id>", "Task this rollback belongs to")
    .requiredOption("--command <override>", "Shell command to execute as the rollback")
    .option("--json", "Output as JSON")
    .action(async (opts: { task: string; command: string; json?: boolean }): Promise<void> => {
      const { task: taskId, command: cmd } = opts;
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      const task = await services.taskStore.get(taskId);
      if (task === undefined) {
        throw new MaestroError(`Task not found: ${taskId}`, [
          "Run `maestro task list` to see available tasks",
        ]);
      }

      const { exitCode } = (deps.spawnSync ?? defaultSpawnSync)(cmd);

      const witnessLevel: WitnessLevel = (deps.isCI ?? defaultIsCI)()
        ? "witnessed-by-ci"
        : "witnessed-by-maestro";

      const payload: RollbackExercisedPayload = {
        command: cmd,
        exit: exitCode,
      };

      const row = await (deps.recordEvidence ?? defaultRecordEvidence)(services.evidenceStore, {
        task_id: taskId,
        kind: "rollback-exercised",
        payload,
        witness_level: witnessLevel,
      });

      output(isJson, {
        task_id: taskId,
        command: cmd,
        exit: exitCode,
        witness_level: witnessLevel,
        evidence_id: row.id,
      }, (r) => [
        exitCode === 0 ? `[ok] Rollback succeeded.` : `[!] Rollback exited with code ${exitCode}.`,
        `  Evidence: ${r.evidence_id}`,
        `  Task:     ${r.task_id}`,
        `  Command:  ${r.command}`,
        `  Exit:     ${r.exit}`,
        `  Witness:  ${r.witness_level}`,
      ]);

      if (exitCode !== 0) {
        process.exit(1);
      }
    });
}
