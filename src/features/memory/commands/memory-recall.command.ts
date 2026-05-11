import type { Command } from "commander";
import { output } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { recallMemory, type RecallResult } from "../usecases/memory-recall.usecase.js";

interface MemoryRecallCommandDeps {
  readonly getServices: () => Pick<Services, "correctionStore" | "learningStore">;
}

export function registerMemoryRecallCommand(
  program: Command,
  deps: MemoryRecallCommandDeps = { getServices },
): void {
  program
    .command("memory-recall")
    .description("Recall relevant corrections and learnings for a task")
    .addHelpText("after", `
Examples:
  maestro memory-recall --task "install dependencies"
  maestro memory-recall --task "refactor auth" --files "src/auth.ts,src/middleware.ts"
  maestro memory-recall --json
`)
    .option("--task <description>", "Task description for context matching")
    .option("--files <paths>", "Comma-separated file paths for glob matching")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = opts.json ?? program.opts().json;

      const filePaths = opts.files
        ? opts.files.split(",").map((f: string) => f.trim()).filter(Boolean)
        : undefined;

      const result = await recallMemory(
        services.correctionStore,
        services.learningStore,
        { taskDescription: opts.task, filePaths },
      );

      output(isJson, result, formatRecall);
    });
}

function formatRecall(result: RecallResult): string[] {
  const lines: string[] = [];

  if (result.corrections.length === 0) {
    lines.push("No matching corrections found");
  } else {
    lines.push(`${result.corrections.length} correction(s) recalled:`);
    for (const c of result.corrections) {
      const sev = c.severity === "hard" ? "[!]" : "[ ]";
      lines.push(`  ${sev} ${c.rule}  (${c.trigger.keywords.join(", ")})`);
    }
  }

  if (result.compiledLearnings) {
    lines.push("", "Compiled learnings:");
    lines.push(`  ${result.compiledLearnings.summary}`);
  }

  return lines;
}
