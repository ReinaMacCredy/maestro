import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import { scanDocGardening, type DocGardeningResult } from "../usecases/doc-gardening.usecase.js";
import {
  scanSlopCleanup,
  formatSlopCleanupLines,
} from "../usecases/slop-cleanup.usecase.js";
import { regenPlan, formatPlanRegenLines } from "../usecases/plan-regen.usecase.js";

interface GcDeps {
  readonly getServices: () => Pick<
    Services,
    "evidenceStore" | "projectRoot" | "taskStore" | "verdictStore" | "specStore"
  >;
}

export function registerGcCommand(
  program: Command,
  deps: GcDeps,
): void {
  const gcCmd = program
    .command("gc")
    .description("On-demand garbage-collection verbs (doc-gardening, slop-cleanup, plan-regen)")
    .action((): void => {
      gcCmd.outputHelp();
    });

  gcCmd
    .command("doc-gardening")
    .description("Scan repo docs for stale path references and broken local links")
    .option("--task <id>", "Record findings as `doc-gardening` evidence under this task id")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string | undefined = typeof opts.task === "string" ? opts.task : undefined;

      const result = await scanDocGardening(
        { evidenceStore: services.evidenceStore },
        {
          projectRoot: services.projectRoot,
          taskId,
          recordEvidence: taskId !== undefined,
        },
      );

      output(isJson, result, formatDocGardeningLines);
    });

  gcCmd
    .command("slop-cleanup")
    .description("Aggregate architecture-lint violations into a per-file slop report")
    .option("--min-severity <level>", "info | warn | error (default: info)", "info")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const minSeverity = parseMinSeverity(opts.minSeverity);
      const result = await scanSlopCleanup({
        projectRoot: services.projectRoot,
        minSeverity,
      });
      output(isJson, result, formatSlopCleanupLines);
    });

  gcCmd
    .command("plan-regen")
    .description("Regenerate plan-vs-state drift summary for a task")
    .requiredOption("--task <id>", "Task id")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const result = await regenPlan(
        {
          taskStore: services.taskStore,
          verdictStore: services.verdictStore,
          specStore: services.specStore,
          evidenceStore: services.evidenceStore,
        },
        {
          projectRoot: services.projectRoot,
          taskId: opts.task,
        },
      );
      output(isJson, result, formatPlanRegenLines);
    });
}

function parseMinSeverity(value: unknown): "info" | "warn" | "error" {
  if (value === "warn" || value === "error" || value === "info") return value;
  return "info";
}

function formatDocGardeningLines(r: DocGardeningResult): string[] {
  const lines: string[] = [];
  lines.push(`Scanned ${r.scannedFiles} doc file${r.scannedFiles !== 1 ? "s" : ""}`);
  lines.push(`${r.staleReferences.length} stale reference${r.staleReferences.length !== 1 ? "s" : ""} found`);
  for (const s of r.staleReferences) {
    lines.push(`  ${s.file}:${s.line} → ${s.reference} (${s.kind})`);
  }
  if (r.evidenceId !== undefined) {
    lines.push("");
    lines.push(`Evidence: ${r.evidenceId}`);
  }
  return lines;
}
