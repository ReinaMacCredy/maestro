import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output } from "@/shared/lib/output.js";
import { getServices } from "@/services.js";
import { compileLearnings, type CompileResult } from "../usecases/memory-compile.usecase.js";

export function registerMemoryCompileCommand(program: Command): void {
  program
    .command("memory-compile")
    .description("Compile raw learnings into a summary")
    .addHelpText("after", `
Examples:
  maestro memory-compile --summary "Key learnings from this sprint"
  maestro memory-compile --summary "..." --json
`)
    .option("--summary <text>", "Compiled summary text")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      if (!opts.summary) {
        throw new MaestroError("--summary is required", [
          'maestro memory-compile --summary "compiled summary of learnings"',
        ]);
      }

      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      const result = await compileLearnings(services.learningStore, opts.summary);

      output(isJson, result, formatCompile);
    });
}

function formatCompile(result: CompileResult): string[] {
  return [
    "[ok] Learnings compiled",
    `  Raw entries: ${result.rawEntries.length}`,
    `  Compiled at: ${result.compiled.compiledAt}`,
    `  Summary: ${result.compiled.summary.slice(0, 120)}`,
  ];
}
