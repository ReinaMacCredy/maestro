import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output } from "@/lib/output.js";
import { getServices } from "@/services.js";
import { searchMemory, type SearchResult } from "../usecases/memory-search.usecase.js";

export function registerMemorySearchCommand(program: Command): void {
  program
    .command("memory-search")
    .description("Search corrections and learnings by text")
    .addHelpText("after", `
Examples:
  maestro memory-search "bun"
  maestro memory-search "async" --json
`)
    .argument("<query>", "Search text")
    .option("--json", "Output as JSON")
    .action(async (query: string, opts) => {
      if (!query.trim()) {
        throw new MaestroError("Search query cannot be empty", [
          'maestro memory-search "keyword"',
        ]);
      }

      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      const result = await searchMemory(
        services.correctionStore,
        services.learningStore,
        query,
      );

      output(isJson, result, formatSearch);
    });
}

function formatSearch(result: SearchResult): string[] {
  const lines: string[] = [];

  if (result.corrections.length > 0) {
    lines.push(`${result.corrections.length} correction(s):`);
    for (const c of result.corrections) {
      lines.push(`  [${c.severity}] ${c.rule}`);
    }
  }

  if (result.learnings.length > 0) {
    if (lines.length > 0) lines.push("");
    lines.push(`${result.learnings.length} learning(s):`);
    for (const l of result.learnings) {
      lines.push(`  ${l.sessionDate}: ${l.content.slice(0, 80)}`);
    }
  }

  if (lines.length === 0) {
    lines.push("No results found");
  }

  return lines;
}
