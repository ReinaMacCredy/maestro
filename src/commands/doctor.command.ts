import type { Command } from "commander";
import { getServices } from "../services.js";
import { runDoctor } from "../usecases/run-doctor.usecase.js";
import { output } from "../lib/output.js";

export function registerDoctorCommand(program: Command): void {
  program
    .command("doctor")
    .description("Verify maestro dependencies and configuration")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const checks = await runDoctor(
        services.git,
        services.config,
        process.cwd(),
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
                : "[!!]";
          lines.push(`${marker} ${check.name}: ${check.message}`);
          if (check.fix) {
            lines.push(`     Fix: ${check.fix}`);
          }
        }

        const fails = list.filter((c) => c.status === "fail").length;
        if (fails > 0) {
          lines.push("", `${fails} issue(s) found`);
        } else {
          lines.push("", "All checks passed");
        }

        return lines;
      });

      const hasFails = checks.some((c) => c.status === "fail");
      if (hasFails) process.exit(1);
    });
}
