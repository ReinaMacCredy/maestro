import type { Command } from "commander";
import { getServices } from "../services.js";
import { reportHandoff } from "../usecases/report-handoff.usecase.js";
import { output } from "../lib/output.js";
import type { HandoffEnvelope } from "../domain/types.js";

export function registerHandoffReportCommand(program: Command): void {
  program
    .command("handoff-report")
    .description("Mark a handoff as completed with a summary report")
    .addHelpText("after", `
Examples:
  maestro handoff-report --content "Implemented note command, 12 tests added" --json
  maestro handoff-report --content "PR created: #42" --id 2026-03-28-001
`)
    .requiredOption("--content <text>", "Summary of work done")
    .option("--id <handoff-id>", "Report on a specific handoff (default: latest picked-up)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      const envelope = await reportHandoff(services.handoffStore, {
        id: opts.id,
        content: opts.content,
      });

      output(isJson, envelope, (e) => formatText(e));
    });
}

function formatText(envelope: HandoffEnvelope): string[] {
  return [
    `[ok] Handoff ${envelope.handoff.id} marked as completed`,
    `  Report: ${envelope.report ?? ""}`,
  ];
}
