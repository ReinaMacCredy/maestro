import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import {
  formatStateSinceLines,
  stateSince,
} from "../usecases/state-since.usecase.js";

interface StateDeps {
  readonly getServices: () => Pick<
    Services,
    "evidenceStore" | "verdictStore" | "taskStore"
  >;
}

export function registerStateCommand(
  program: Command,
  deps: StateDeps = { getServices },
): void {
  const stateCmd = program
    .command("state")
    .description("Cross-store state queries");

  stateCmd
    .command("since <iso>")
    .description("Stream evidence + verdict events ordered chronologically since the given ISO timestamp")
    .option("--until <iso>", "Cap the window at this timestamp")
    .option("--task <id>", "Limit to a single task id")
    .option("--json", "Output as JSON")
    .action(async (sinceArg: string, opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await stateSince(
        {
          evidenceStore: services.evidenceStore,
          verdictStore: services.verdictStore,
          taskStore: services.taskStore,
        },
        {
          since: sinceArg,
          until: typeof opts.until === "string" ? opts.until : undefined,
          taskId: typeof opts.task === "string" ? opts.task : undefined,
        },
      );

      output(isJson, result, formatStateSinceLines);
    });
}
