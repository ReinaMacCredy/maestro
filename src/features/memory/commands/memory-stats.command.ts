import type { Command } from "commander";
import type { MemoryStats } from "../domain/memory-types.js";
import { output } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { getMemoryStats } from "../usecases/memory-stats.usecase.js";

interface MemoryStatsCommandDeps {
  readonly getServices: () => Pick<
    Services,
    "correctionStore" | "learningStore" | "ratchetStore" | "projectGraphStore"
  >;
}

export function registerMemoryStatsCommand(
  program: Command,
  deps: MemoryStatsCommandDeps = { getServices },
): void {
  program
    .command("memory-stats")
    .description("Show memory system statistics")
    .addHelpText("after", `
Examples:
  maestro memory-stats
  maestro memory-stats --json
`)
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = opts.json ?? program.opts().json;

      const stats = await getMemoryStats(
        services.correctionStore,
        services.learningStore,
        services.ratchetStore,
        services.projectGraphStore,
      );

      output(isJson, stats, formatStats);
    });
}

function formatStats(stats: MemoryStats): string[] {
  const lines: string[] = ["Memory System Stats", ""];

  lines.push(`Corrections: ${stats.corrections.total} (${stats.corrections.hard} hard, ${stats.corrections.soft} soft)`);

  let learnLine = `Learnings: ${stats.learnings.rawCount} raw`;
  if (stats.learnings.compiledAt) {
    learnLine += ` / compiled ${stats.learnings.compiledAt}`;
    if (stats.learnings.staleDays !== undefined) {
      learnLine += ` (${stats.learnings.staleDays}d ago)`;
    }
  } else {
    learnLine += " / not compiled";
  }
  lines.push(learnLine);

  let ratchetLine = `Ratchet: ${stats.ratchet.assertions} assertions`;
  if (stats.ratchet.lastResult) {
    ratchetLine += ` / last: ${stats.ratchet.lastResult}`;
  }
  lines.push(ratchetLine);

  lines.push(`Graph: ${stats.graph.projects} projects, ${stats.graph.links} links`);

  return lines;
}
