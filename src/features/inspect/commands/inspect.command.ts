import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import {
  inspectRun,
  formatInspectRunLines,
} from "../usecases/inspect-run.usecase.js";

interface InspectDeps {
  readonly getServices: () => Pick<
    Services,
    "evidenceStore" | "verdictStore" | "projectRoot"
  >;
}

export function registerInspectCommand(
  program: Command,
  deps: InspectDeps = { getServices },
): void {
  program
    .command("inspect <taskId>")
    .description("Post-mortem snapshot of a task run (artifacts, evidence, verdicts)")
    .option("--tail <n>", "Number of recent evidence/verdict rows to include (default: 10)")
    .option("--json", "Output as JSON")
    .action(async (taskId: string, opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const tail = typeof opts.tail === "string" ? parseInt(opts.tail, 10) : undefined;
      const result = await inspectRun(
        {
          evidenceStore: services.evidenceStore,
          verdictStore: services.verdictStore,
        },
        {
          projectRoot: services.projectRoot,
          taskId,
          ...(tail !== undefined && Number.isFinite(tail) ? { tail } : {}),
        },
      );
      output(isJson, result, formatInspectRunLines);
    });
}
