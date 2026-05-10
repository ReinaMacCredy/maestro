import type { Command } from "commander";
import { getServices } from "@/services.js";
import { checkStatus } from "@/infra/usecases/check-status.usecase.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";

export function registerStatusCommand(program: Command): void {
  program
    .command("status")
    .description("Show current maestro state")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const status = await checkStatus(
        services.config,
        services.git,
        process.cwd(),
      );

      output(isJson, status, (s) => {
        const lines: string[] = [];

        if (s.initialized) {
          lines.push(`[ok] Initialized (config: ${s.configSource})`);
        } else {
          lines.push("[!] Not initialized. Run: maestro init");
        }

        lines.push(
          s.gitAvailable ? "[ok] Git available" : "[!] Not in a git repo",
        );

        if (s.legacyHandoffCount > 0) {
          lines.push(
            `[--] Found ${s.legacyHandoffCount} legacy handoff artifact(s) under .maestro/handoffs/`,
          );
        }

        return lines;
      });
    });
}
