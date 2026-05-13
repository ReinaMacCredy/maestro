import type { Command } from "commander";
import { output } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import { lintMemory, type LintResult } from "../usecases/memory-lint.usecase.js";

interface MemoryLintCommandDeps {
  readonly getServices: () => Pick<Services, "correctionStore" | "learningStore" | "ratchetStore">;
}

export function registerMemoryLintCommand(
  program: Command,
  deps: MemoryLintCommandDeps,
): void {
  program
    .command("memory-lint")
    .description("Check memory system health")
    .addHelpText("after", `
Examples:
  maestro memory-lint
  maestro memory-lint --json
`)
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = opts.json ?? program.opts().json;

      const result = await lintMemory(
        services.correctionStore,
        services.learningStore,
        services.ratchetStore,
      );

      output(isJson, result, formatLint);
    });
}

function formatLint(result: LintResult): string[] {
  if (result.healthy) {
    return ["[ok] Memory system healthy -- no warnings"];
  }

  const lines: string[] = [`[!] ${result.warnings.length} warning(s)`];
  for (const w of result.warnings) {
    lines.push(`  [${w.category}] ${w.message}`);
  }
  return lines;
}
