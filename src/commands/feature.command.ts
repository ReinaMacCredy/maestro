/**
 * Feature command handler
 * Implements CLI commands: feature list|update
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output, resolveJsonFlag } from "../lib/output.js";
import {
  listFeatures,
  updateFeature,
  parseWorkerReport,
  type ListFeaturesResult,
  type UpdateFeatureResult,
} from "../usecases/feature-lifecycle.usecase.js";
import {
  generateWorkerPrompt,
  type GenerateWorkerPromptResult,
} from "../usecases/generate-worker-prompt.usecase.js";
import { MaestroError } from "../domain/errors.js";

export function registerFeatureCommand(program: Command): void {
  const featureCmd = program
    .command("feature")
    .description("Feature lifecycle management")
    .option("--json", "Output as JSON");

  featureCmd
    .command("list")
    .description("List features for a mission")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--milestone <id>", "Filter by milestone ID")
    .option("--status <status>", "Filter by status (pending, assigned, in-progress, review, done, blocked)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (!opts.mission) {
        // This should never happen due to requiredOption, but handle defensively
        throw new MaestroError("--mission is required", [
          "Usage: maestro feature list --mission <id>",
          "Optional filters: --milestone <id> --status <status>",
        ]);
      }

      const result = await listFeatures(
        services.missionStore,
        services.featureStore,
        opts.mission,
        {
          milestoneId: opts.milestone,
          status: opts.status,
        },
      );

      output(isJson, result, formatFeatureList);
    });

  featureCmd
    .command("update <featureId>")
    .description("Update feature status and/or attach a worker report")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--status <status>", "New status (pending, assigned, in-progress, review, done, blocked)")
    .option("--report <value>", "Worker report as inline JSON or @file.json")
    .option("--retry-reason <reason>", "Reason for retrying (when status is pending)")
    .option("--json", "Output as JSON")
    .action(async (featureId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (!opts.mission) {
        throw new MaestroError("--mission is required", [
          "Usage: maestro feature update <featureId> --mission <id> --status <status>",
          "Optional: --report '{\"content\": \"...\"}' or --report @report.json",
        ]);
      }

      if (!opts.status && !opts.report) {
        throw new MaestroError("No update specified", [
          "Usage: maestro feature update <featureId> --mission <id> --status <status>",
          "Or: maestro feature update <featureId> --mission <id> --report @report.json",
          "Or both: --status <status> --report <report>",
        ]);
      }

      // Parse report if provided
      let report: Awaited<ReturnType<typeof parseWorkerReport>> | undefined;
      if (opts.report) {
        report = await parseWorkerReport(opts.report);
      }

      const result = await updateFeature(
        services.missionStore,
        services.featureStore,
        services.runtimeStore,
        process.cwd(),
        opts.mission,
        featureId,
        {
          status: opts.status,
          report,
          retryReason: opts.retryReason,
        },
      );

      output(isJson, result, formatFeatureUpdate);
    });

    featureCmd
      .command("prompt <featureId>")
    .description("Generate a worker prompt for a feature")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--out <path>", "Write prompt to specified path (also writes to workers/{featureId}/prompt.md)")
    .option("--json", "Output as JSON")
    .action(async (featureId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (!opts.mission) {
        throw new MaestroError("--mission is required", [
          "Usage: maestro feature prompt <featureId> --mission <id>",
          "Optional: --out /path/to/prompt.md",
        ]);
      }

      const result = await generateWorkerPrompt(
        services.missionStore,
        services.featureStore,
        services.assertionStore,
        services.runtimeStore,
        process.cwd(),
        opts.mission,
        featureId,
        opts.out,
        services.correctionStore,
        services.learningStore,
      );

      output(isJson, result, formatPromptResult);
    });
}

/** Format feature list for text output */
function formatFeatureList(result: ListFeaturesResult): string[] {
  if (result.features.length === 0) {
    return ["No features found"];
  }

  const lines: string[] = [
    `${result.filtered} feature(s) (total: ${result.total})`,
    "",
  ];

  for (const f of result.features) {
    const status = f.status.padEnd(12);
    const title = f.title.slice(0, 40).padEnd(40);
    lines.push(`${f.id}  ${status}  ${title}  [${f.milestoneId}]`);
  }

  return lines;
}

/** Format feature update result for text output */
function formatFeatureUpdate(result: UpdateFeatureResult): string[] {
  const lines: string[] = [
    `[ok] Feature updated: ${result.feature.id}`,
    `  Status: ${result.feature.status}`,
    `  Title: ${result.feature.title}`,
  ];

  if (result.missionAutoStarted) {
    lines.push("  Mission: auto-started to executing");
  }

  if (result.reportPersisted) {
    lines.push(`  Report: ${result.reportPersisted}`);
  }

  if (result.feature.report) {
    lines.push(`  Summary: ${result.feature.report.salientSummary}`);
  }

  return lines;
}

/** Format prompt generation result for text output */
function formatPromptResult(result: GenerateWorkerPromptResult): string[] {
  const lines: string[] = [
    `[ok] Worker prompt generated for: ${result.featureId}`,
    `  Worker type: ${result.workerType}`,
  ];

  if (result.writtenTo) {
    for (const path of result.writtenTo) {
      lines.push(`  Written to: ${path}`);
    }
  }

  lines.push("");
  lines.push("--- PROMPT BEGIN ---");
  lines.push("");
  lines.push(result.prompt);
  lines.push("");
  lines.push("--- PROMPT END ---");

  return lines;
}
