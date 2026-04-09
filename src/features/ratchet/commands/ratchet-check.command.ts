import type { Command } from "commander";
import { output } from "../lib/output.js";
import { getServices } from "../services.js";
import { checkRatchet, type RatchetCheckResult } from "../usecases/ratchet-check.usecase.js";

export function registerRatchetCheckCommand(program: Command): void {
  program
    .command("ratchet-check")
    .description("Run the regression ratchet suite")
    .addHelpText("after", `
Examples:
  maestro ratchet-check
  maestro ratchet-check --json
`)
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      const result = await checkRatchet(services.ratchetStore, process.cwd());

      output(isJson, result, formatCheck);
    });
}

function formatCheck(result: RatchetCheckResult): string[] {
  if (result.totalCount === 0) {
    return ["No ratchet assertions defined", "  Use `maestro ratchet-promote` to promote corrections"];
  }

  const lines: string[] = [
    `Ratchet: ${result.passCount}/${result.totalCount} passed ${result.passed ? "[ok]" : "[!] REGRESSION"}`,
  ];

  for (const r of result.results) {
    const status = r.passed ? "[ok]" : "[!]";
    lines.push(`  ${status} ${r.assertion.rule}`);
    if (r.detail) lines.push(`       ${r.detail}`);
  }

  if (result.previousBaseline) {
    lines.push(`  Previous: ${result.previousBaseline.passCount} pass (${result.previousBaseline.lastRunAt})`);
  }

  return lines;
}
