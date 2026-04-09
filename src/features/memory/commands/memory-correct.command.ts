import type { Command } from "commander";
import type { Correction } from "../domain/memory-types.js";
import { MaestroError } from "@/shared/errors.js";
import { output } from "@/lib/output.js";
import { getServices } from "@/services.js";
import { captureCorrection } from "../usecases/memory-correct.usecase.js";

export function registerMemoryCorrectCommand(program: Command): void {
  program
    .command("memory-correct")
    .description("Capture a correction rule for future sessions")
    .addHelpText("after", `
Examples:
  maestro memory-correct "use bun not npm" --source "used npm install" --trigger "package,install,npm" --severity hard
  maestro memory-correct "no fire-and-forget" --source "missing await" --trigger "async,Promise" --globs "*.ts"
`)
    .argument("<rule>", "The correction rule (what to do instead)")
    .option("--source <text>", "What went wrong", "")
    .option("--trigger <keywords>", "Comma-separated trigger keywords", "")
    .option("--globs <patterns>", "Comma-separated file glob patterns", "")
    .option("--severity <level>", "soft or hard", "soft")
    .option("--json", "Output as JSON")
    .action(async (rule: string, opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      if (!opts.trigger && !opts.globs) {
        throw new MaestroError("At least --trigger or --globs is required", [
          'maestro memory-correct "rule" --trigger "keyword1,keyword2"',
        ]);
      }

      const keywords = opts.trigger
        ? opts.trigger.split(",").map((k: string) => k.trim()).filter(Boolean)
        : [];
      const fileGlobs = opts.globs
        ? opts.globs.split(",").map((g: string) => g.trim()).filter(Boolean)
        : [];

      const correction = await captureCorrection(services.correctionStore, {
        rule,
        source: opts.source || rule,
        keywords,
        fileGlobs,
        severity: opts.severity === "hard" ? "hard" : "soft",
      });

      output(isJson, correction, formatCreated);
    });
}

function formatCreated(c: Correction): string[] {
  return [
    "[ok] Correction captured",
    `  ID: ${c.id}`,
    `  Rule: ${c.rule}`,
    `  Severity: ${c.severity}`,
    `  Keywords: ${c.trigger.keywords.join(", ") || "(none)"}`,
    `  Globs: ${c.trigger.fileGlobs.join(", ") || "(none)"}`,
  ];
}
