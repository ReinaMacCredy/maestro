import type { Command } from "commander";
import { getServices } from "../services.js";
import { output } from "../lib/output.js";
import { MaestroError } from "../domain/errors.js";

export function registerHandoffCleanupCommand(program: Command): void {
  program
    .command("handoff-cleanup")
    .description("Delete all handoffs in the current project")
    .addHelpText("after", `
Examples:
  maestro handoff-cleanup              # shows count, asks for --force
  maestro handoff-cleanup --force      # deletes all handoffs
  maestro handoff-cleanup --force --json
`)
    .option("--force", "Actually delete all handoffs (required)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      const ids = await services.handoffStore.listIds();

      if (ids.length === 0) {
        output(isJson, { count: 0, deleted: false }, () => [
          "No handoffs to clean up",
        ]);
        return;
      }

      if (!opts.force) {
        throw new MaestroError(
          `${ids.length} handoff(s) would be deleted`,
          ["Re-run with --force to confirm: maestro handoff-cleanup --force"],
        );
      }

      await Promise.all(ids.map((id) => services.handoffStore.delete(id)));

      output(isJson, { count: ids.length, deleted: true, ids }, (d) => [
        `[ok] Deleted ${d.count} handoff(s)`,
      ]);
    });
}
