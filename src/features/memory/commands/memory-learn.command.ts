import type { Command } from "commander";
import type { RawLearningEntry } from "../domain/memory-types.js";
import { MaestroError } from "@/shared/errors.js";
import { output } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { appendLearning } from "../usecases/memory-learn.usecase.js";

interface MemoryLearnCommandDeps {
  readonly getServices: () => Pick<Services, "git" | "learningStore">;
}

export function registerMemoryLearnCommand(
  program: Command,
  deps: MemoryLearnCommandDeps = { getServices },
): void {
  program
    .command("memory-learn")
    .description("Append a learning entry for this session")
    .addHelpText("after", `
Examples:
  maestro memory-learn --content "handoff context needs git diff"
  maestro memory-learn --content "TUI render-check misses empty strings" --json
`)
    .option("--content <text>", "Learning content")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      if (!opts.content) {
        throw new MaestroError("--content is required", [
          'maestro memory-learn --content "what you learned"',
        ]);
      }

      const services = deps.getServices();
      const isJson = opts.json ?? program.opts().json;

      const entry = await appendLearning(services.git, services.learningStore, {
        content: opts.content,
        dir: process.cwd(),
      });

      output(isJson, entry, formatEntry);
    });
}

function formatEntry(entry: RawLearningEntry): string[] {
  return [
    "[ok] Learning captured",
    `  Date: ${entry.sessionDate}`,
    `  Branch: ${entry.branch ?? "(none)"}`,
    `  Content: ${entry.content}`,
  ];
}
