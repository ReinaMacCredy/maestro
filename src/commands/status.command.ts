import type { Command } from "commander";
import { getServices } from "../services.js";
import { checkStatus } from "../usecases/check-status.usecase.js";
import { output } from "../lib/output.js";

export function registerStatusCommand(program: Command): void {
  program
    .command("status")
    .description("Show current maestro state")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const status = await checkStatus(
        services.handoffStore,
        services.config,
        services.cass,
        services.git,
        process.cwd(),
      );

      const isJson = opts.json ?? program.opts().json;
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
        lines.push(
          s.cassAvailable
            ? "[ok] CASS available"
            : "[!] CASS not available. Install: brew install dicklesworthstone/tap/cass",
        );

        if (s.pendingHandoffs.length > 0) {
          lines.push(
            "",
            `${s.pendingHandoffs.length} pending handoff(s):`,
          );
          for (const e of s.pendingHandoffs) {
            lines.push(`  ${e.handoff.id}  ${e.handoff.message}`);
          }
        } else {
          lines.push("", "No pending handoffs");
        }

        return lines;
      });
    });
}
