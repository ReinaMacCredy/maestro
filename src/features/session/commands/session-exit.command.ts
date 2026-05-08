import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { sessionExit } from "../usecases/session-exit.usecase.js";

interface SessionExitDeps {
  readonly getServices: () => Pick<
    Services,
    "evidenceStore" | "verdictStore" | "projectRoot"
  >;
  readonly applyExit?: (code: number) => void;
}

export function registerSessionExitCommand(
  sessionCmd: Command,
  program: Command,
  deps: SessionExitDeps = { getServices },
): void {
  sessionCmd
    .command("exit <taskId>")
    .description("Close a task session: verify baseline, record warnings, write progress, and exit non-zero on regression")
    .option("--json", "Output as JSON")
    .action(async (taskId: string, opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await sessionExit(
        {
          evidenceStore: services.evidenceStore,
          verdictStore: services.verdictStore,
        },
        { taskId, projectRoot: services.projectRoot },
      );

      if (isJson) {
        process.stdout.write(
          JSON.stringify(
            {
              taskId,
              exitCode: result.exitCode,
              summary: result.summary,
              warnings: result.warnings,
              progressPath: result.progressPath,
            },
            null,
            2,
          ) + "\n",
        );
      } else {
        console.log(`Session exit for ${taskId}`);
        console.log(`  Lint violations:    ${result.summary.lintViolations}`);
        console.log(`  Baseline clean:     ${result.summary.baselineClean}`);
        console.log(`  Working tree dirty: ${result.summary.dirtyTree}`);
        if (result.summary.verdictDecision !== undefined) {
          console.log(`  Latest verdict:     ${result.summary.verdictDecision}`);
        }
        for (const w of result.warnings) console.error(`[warn] ${w}`);
        console.log(`  Progress digest:    ${result.progressPath}`);
      }

      if (result.exitCode !== 0) {
        (deps.applyExit ?? ((code) => process.exit(code)))(result.exitCode);
      }
    });
}
