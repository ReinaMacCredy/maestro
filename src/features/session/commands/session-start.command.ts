import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { listOpenProjectHandoffIdsForTask } from "@/features/handoff";
import { type Services } from "@/services.js";
import { sessionStart } from "../usecases/session-start.usecase.js";

interface SessionStartDeps {
  readonly getServices: () => Pick<
    Services,
    | "taskStore"
    | "taskContinuationStore"
    | "taskContinuationHistory"
    | "specStore"
    | "verdictStore"
    | "evidenceStore"
    | "runStateStore"
    | "contractStore"
    | "contractVersionStore"
    | "handoffStore"
    | "projectRoot"
  >;
}

export function registerSessionStartCommand(
  sessionCmd: Command,
  program: Command,
  deps: SessionStartDeps,
): void {
  sessionCmd
    .command("start <taskId>")
    .description("Open a task: write an orient digest, baseline-verify, and record a session-start anchor")
    .option("--json", "Output as JSON")
    .action(async (taskId: string, opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const currentProjectRoot = resolveMaestroProjectRoot(process.cwd());

      const result = await sessionStart(
        {
          taskStore: services.taskStore,
          continuationStore: services.taskContinuationStore,
          continuationHistory: services.taskContinuationHistory,
          specStore: services.specStore,
          verdictStore: services.verdictStore,
          evidenceStore: services.evidenceStore,
          runStateStore: services.runStateStore,
          contractStore: services.contractStore,
          contractVersionStore: services.contractVersionStore,
          listOpenHandoffIds: (id) =>
            listOpenProjectHandoffIdsForTask(services.handoffStore, id, {
              taskStore: services.taskStore,
              currentProjectRoot,
            }),
          repoRoot: services.projectRoot,
        },
        { taskId, projectRoot: services.projectRoot },
      );

      if (isJson) {
        process.stdout.write(
          JSON.stringify(
            {
              taskId,
              orientPath: result.orientPath,
              headSha: result.headSha,
            },
            null,
            2,
          ) + "\n",
        );
        return;
      }
      console.log(`Session started for ${taskId}`);
      console.log(`  Orient digest: ${result.orientPath}`);
      console.log(`  Anchor commit: ${result.headSha || "(unknown)"}`);
    });
}
