import type { Command } from "commander";
import { getServices } from "../services.js";
import { output } from "../lib/output.js";
import { MaestroError } from "../domain/errors.js";

export function registerHandoffDropCommand(program: Command): void {
  program
    .command("handoff-drop")
    .description("Delete a single handoff by ID")
    .addHelpText("after", `
Examples:
  maestro handoff-drop --id 2026-03-28-001
  maestro handoff-drop --id 2026-03-28-001 --json
`)
    .requiredOption("--id <handoff-id>", "ID of the handoff to delete")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      const existingIds = await services.handoffStore.listIds();
      if (!existingIds.includes(opts.id)) {
        throw new MaestroError(`Handoff not found: ${opts.id}`, [
          "Run: maestro handoff --list --all",
        ]);
      }

      await services.handoffStore.delete(opts.id);

      output(isJson, { id: opts.id, deleted: true }, () => [
        `[ok] Handoff ${opts.id} deleted`,
      ]);
    });
}
