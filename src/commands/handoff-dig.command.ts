import type { Command } from "commander";
import { getServices } from "../services.js";
import { digHandoff } from "../usecases/dig-handoff.usecase.js";
import { output } from "../lib/output.js";

export function registerHandoffDigCommand(program: Command): void {
  program
    .command("handoff-dig")
    .description("Search previous session via CASS, scoped to a handoff")
    .addHelpText("after", `
Examples:
  maestro handoff-dig "token refresh" --json
  maestro handoff-dig "auth adapter" --id 2026-03-28-001 --limit 5
`)
    .argument("<query>", "Search query")
    .option("--id <handoff-id>", "Scope search to a specific handoff")
    .option("--limit <n>", "Max results", "10")
    .option("--json", "Output as JSON")
    .action(async (query: string, opts) => {
      const services = getServices();
      const results = await digHandoff(
        services.handoffStore,
        services.cass,
        query,
        {
          id: opts.id,
          limit: parseInt(opts.limit, 10),
        },
      );

      const isJson = opts.json ?? program.opts().json;
      output(isJson, results, (r) => {
        if (r.hits.length === 0) {
          return [`No results for "${r.query}"`];
        }
        return [
          `${r.totalMatches} result(s) for "${r.query}":`,
          "",
          ...r.hits.map(
            (hit) =>
              `  [${hit.agent}] ${hit.sourcePath}:${hit.lineNumber}\n    ${hit.snippet.slice(0, 120)}`,
          ),
        ];
      });
    });
}
