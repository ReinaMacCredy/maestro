/**
 * Checkpoint command handler
 * Implements CLI commands: checkpoint save|list|load
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output } from "../lib/output.js";
import {
  saveCheckpoint,
  listCheckpoints,
  loadCheckpoint,
  type SaveCheckpointResult,
  type ListCheckpointsResult,
  type LoadCheckpointResult,
} from "../usecases/checkpoint-lifecycle.usecase.js";

/** Resolve --json flag from leaf, group, or root options */
function resolveJsonFlag(opts: Record<string, unknown>, program: Command): boolean {
  // Leaf option takes precedence
  if (opts.json !== undefined) return opts.json as boolean;
  // Then group option
  if (opts.jsonGroup !== undefined) return opts.jsonGroup as boolean;
  // Then root option
  return program.opts().json as boolean ?? false;
}

export function registerCheckpointCommand(program: Command): void {
  const checkpointCmd = program
    .command("checkpoint")
    .description("Checkpoint lifecycle management - save/load mission state snapshots")
    .option("--json", "Output as JSON");

  checkpointCmd
    .command("save")
    .description("Save a timestamped snapshot of current mission state")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await saveCheckpoint(
        services.missionStore,
        services.featureStore,
        services.assertionStore,
        services.checkpointStore,
        opts.mission,
      );

      output(isJson, result, formatSaveResult);
    });

  checkpointCmd
    .command("list")
    .description("List all checkpoints for a mission (newest first)")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await listCheckpoints(
        services.missionStore,
        services.checkpointStore,
        opts.mission,
      );

      output(isJson, result, formatListResult);
    });

  checkpointCmd
    .command("load")
    .description("Read the latest checkpoint snapshot for a mission (metadata only)")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await loadCheckpoint(
        services.missionStore,
        services.checkpointStore,
        opts.mission,
      );

      output(isJson, result, formatLoadResult);
    });
}

/** Format save result for text output */
function formatSaveResult(result: SaveCheckpointResult): string[] {
  const cp = result.checkpoint;
  const featureCount = Object.keys(cp.featureStates).length;
  const assertionCount = Object.keys(cp.assertionStates).length;

  return [
    `[ok] Checkpoint saved: ${cp.id}`,
    `  Mission: ${cp.missionId}`,
    `  Milestone: ${cp.milestoneId}`,
    `  Timestamp: ${cp.timestamp}`,
    `  Features captured: ${featureCount}`,
    `  Assertions captured: ${assertionCount}`,
  ];
}

/** Format list result for text output */
function formatListResult(result: ListCheckpointsResult): string[] {
  if (result.checkpoints.length === 0) {
    return [`No checkpoints found for mission ${result.mission.id}`];
  }

  const lines: string[] = [
    `Mission: ${result.mission.title} (${result.mission.id})`,
    "",
    `${result.checkpoints.length} checkpoint(s) (newest first):`,
    "",
  ];

  for (const cp of result.checkpoints) {
    const featureCount = Object.keys(cp.featureStates).length;
    const assertionCount = Object.keys(cp.assertionStates).length;
    const date = new Date(cp.timestamp).toLocaleString();

    lines.push(`${cp.id}`);
    lines.push(`  Milestone: ${cp.milestoneId}`);
    lines.push(`  Time: ${date}`);
    lines.push(`  Features: ${featureCount}, Assertions: ${assertionCount}`);
    lines.push("");
  }

  return lines;
}

/** Format load result for text output */
function formatLoadResult(result: LoadCheckpointResult): string[] {
  const cp = result.checkpoint;
  const featureCount = Object.keys(cp.featureStates).length;
  const assertionCount = Object.keys(cp.assertionStates).length;

  const lines: string[] = [
    `[ok] Checkpoint snapshot loaded (metadata only): ${cp.id}`,
    `  Mission: ${cp.missionId}`,
    `  Milestone: ${cp.milestoneId}`,
    `  Timestamp: ${cp.timestamp}`,
    `  Features captured: ${featureCount}`,
    `  Assertions captured: ${assertionCount}`,
    "",
    `[!] ${result.warning}`,
  ];

  return lines;
}
