import type { Command } from "commander";
import { type Services } from "@/services.js";
import { runDoctor } from "../usecases/run-doctor.usecase.js";
import { output } from "@/shared/lib/output.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";

interface DoctorCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "config" | "git" | "taskStore" | "verdictStore"
  >;
}

export function registerDoctorCommand(
  program: Command,
  deps: DoctorCommandDeps,
): void {
  program
    .command("doctor")
    .description("Verify maestro health (3 fast dimensions; --full adds build + tests)")
    .option("--json", "Output as JSON")
    .option("--full", "Also run build and tests (warn-only)")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const projectDir = resolveMaestroProjectRoot(process.cwd());
      const checks = await runDoctor({
        taskStore: services.taskStore,
        verdictStore: services.verdictStore,
        config: services.config,
        projectDir,
        full: opts.full === true,
      });

      const scaffoldFailed = checks.some(
        (c) => c.name === "scaffold" && c.status === "fail",
      );

      const isJson = opts.json ?? program.opts().json;
      output(isJson, checks, (list) => {
        const lines: string[] = [];
        for (const check of list) {
          const marker =
            check.status === "ok"
              ? "[ok]"
              : check.status === "warn"
                ? "[--]"
                : "[!]";
          lines.push(`${marker} ${check.name}: ${check.message}`);
          if (check.fix) {
            lines.push(`     Fix: ${check.fix}`);
          }
        }
        lines.push(
          "",
          scaffoldFailed
            ? "Scaffold check failed -- run `maestro setup` and re-run"
            : "All required checks passed",
        );
        return lines;
      });

      // Exit code semantics: only scaffold failure gates the exit code.
      // Other checks (verdict-freshness, build, tests) produce warnings but
      // don't fail doctor. This keeps the fast form (what init.sh calls)
      // sub-second and ensures doctor is a status check, not a gate.
      if (scaffoldFailed) process.exit(1);
    });
}
