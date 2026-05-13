import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { listOpenProjectHandoffIdsForTask } from "@/features/handoff";
import { type Services } from "@/services.js";
import {
  composeTaskIntrospection,
  formatTaskIntrospectionMarkdown,
} from "../usecases/compose-task-introspection.usecase.js";
import { resolveTaskRef } from "../domain/task-slug.js";

interface TaskIntrospectDeps {
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

export function registerTaskIntrospectCommand(
  taskCmd: Command,
  program: Command,
  deps: TaskIntrospectDeps,
): void {
  taskCmd
    .command("introspect <id-or-slug>")
    .description("Show a task's full context: spec, verdict, budget, lints, blockers, recent activity")
    .option("--json", "Output as JSON")
    .action(async (rawRef: string, opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const currentProjectRoot = resolveMaestroProjectRoot(process.cwd());

      const resolved = await resolveTaskRef(services.taskStore, rawRef);
      const view = await composeTaskIntrospection(
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
          listOpenHandoffIds: (taskId) =>
            listOpenProjectHandoffIdsForTask(services.handoffStore, taskId, {
              taskStore: services.taskStore,
              currentProjectRoot,
            }),
          repoRoot: services.projectRoot,
        },
        resolved.id,
      );

      if (isJson) {
        process.stdout.write(JSON.stringify(view, null, 2) + "\n");
        return;
      }
      console.log(formatTaskIntrospectionMarkdown(view));
    });
}
