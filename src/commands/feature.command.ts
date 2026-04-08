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
import {
  runFeatures,
  type RunFeaturesResult,
} from "../usecases/run-features.usecase.js";
import { MaestroError } from "../domain/errors.js";
import type { Feature } from "../domain/mission-types.js";

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
      .command("run")
      .description("Run one or more features through the configured worker CLI")
      .requiredOption("--mission <id>", "Mission ID (required)")
      .option("--feature <id...>", "Feature ID subset to run")
      .option("--worker <slug>", "Override the configured default worker")
      .option("--dry-run", "Generate prompts without spawning workers")
      .option("--json", "Output as JSON")
      .action(async (opts) => {
        const services = getServices();
        const isJson = resolveJsonFlag(opts, program);

        if (!opts.mission) {
          throw new MaestroError("--mission is required", [
            "Usage: maestro feature run --mission <id>",
          ]);
        }

        const config = await services.config.load(process.cwd());
        const result = await runFeatures(
          {
            missionStore: services.missionStore,
            featureStore: services.featureStore,
            assertionStore: services.assertionStore,
            runtimeStore: services.runtimeStore,
            runtimeEventStore: services.runtimeEventStore,
            executionStore: services.executionStore,
            transport: services.transport,
            baseDir: process.cwd(),
            config,
            // Enables auto-injection of corrections + compiled learnings
            // into every worker prompt. Best-effort; never blocks on failure.
            correctionStore: services.correctionStore,
            learningStore: services.learningStore,
          },
          {
            missionId: opts.mission,
            featureIds: opts.feature as string[] | undefined,
            workerOverride: opts.worker as string | undefined,
            dryRun: opts.dryRun as boolean | undefined,
          },
        );

        output(isJson, result, formatRunResult);
        process.exitCode = result.success ? 0 : 1;
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

function formatRunResult(result: RunFeaturesResult): string[] {
  const lines = [
    result.success ? `[ok] Feature run finished for mission: ${result.missionId}` : `[!] Feature run stopped for mission: ${result.missionId}`,
    `  Mode: ${result.dryRun ? "dry-run" : "execute"}`,
  ];

  if (result.stoppedOnFeatureId) {
    lines.push(`  Stopped on: ${result.stoppedOnFeatureId}`);
  }

  for (const outcome of result.outcomes) {
    lines.push("");
    lines.push(`${outcome.featureId}  ${outcome.status}  ${outcome.worker}`);
    lines.push(`  Summary: ${outcome.summary}`);
    if (outcome.durationMs !== undefined) {
      lines.push(`  Duration: ${outcome.durationMs}ms`);
    }
    if (outcome.filesChanged && outcome.filesChanged.length > 0) {
      lines.push(`  Files: ${outcome.filesChanged.join(", ")}`);
    }
  }

  return lines;
}
