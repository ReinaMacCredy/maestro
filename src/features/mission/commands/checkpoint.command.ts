/**
 * Checkpoint command handler
 * Implements CLI commands: checkpoint save|list|load
 */
import type { Command } from "commander";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import {
  saveCheckpoint,
  listCheckpoints,
  loadCheckpoint,
  type SaveCheckpointResult,
  type ListCheckpointsResult,
  type LoadCheckpointResult,
} from "../usecases/checkpoint-lifecycle.usecase.js";

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
    .action(async (opts): Promise<void> => {
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
    .action(async (opts): Promise<void> => {
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
    .description("Load the latest checkpoint snapshot for a mission and restore changed state")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await loadCheckpoint(
        services.missionStore,
        services.featureStore,
        services.assertionStore,
        services.checkpointStore,
        opts.mission,
      );

      output(isJson, result, formatLoadResult);
    });
}

/** Format save result for text output */
function formatSaveResult(result: SaveCheckpointResult): string[] {
  const cp = result.checkpoint;
  const featureCount = Object.keys(cp.featureStatuses).length;
  const assertionCount = Object.keys(cp.assertionResults).length;

  return [
    `[ok] Checkpoint saved: ${cp.id}`,
    `  Mission: ${cp.missionId}`,
    `  Milestone: ${cp.currentMilestoneId}`,
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
    const featureCount = Object.keys(cp.featureStatuses).length;
    const assertionCount = Object.keys(cp.assertionResults).length;
    const date = new Date(cp.timestamp).toLocaleString();

    lines.push(`${cp.id}`);
    lines.push(`  Milestone: ${cp.currentMilestoneId}`);
    lines.push(`  Time: ${date}`);
    lines.push(`  Features: ${featureCount}, Assertions: ${assertionCount}`);
    lines.push("");
  }

  return lines;
}

/** Format load result for text output */
function formatLoadResult(result: LoadCheckpointResult): string[] {
  const cp = result.checkpoint;
  const featureCount = Object.keys(cp.featureStatuses).length;
  const assertionCount = Object.keys(cp.assertionResults).length;
  const totalRestored = result.restored.featureCount + result.restored.assertionCount;

  const lines = [
    `[ok] Checkpoint restored: ${cp.id}`,
    `  Mission: ${cp.missionId}`,
    `  Milestone: ${cp.currentMilestoneId}`,
    `  Timestamp: ${cp.timestamp}`,
    `  Features captured: ${featureCount}`,
    `  Assertions captured: ${assertionCount}`,
    `  Features restored: ${result.restored.featureCount}`,
    `  Assertions restored: ${result.restored.assertionCount}`,
  ];

  if (totalRestored === 0) {
    lines.push("  No state changes were needed; current state already matches the checkpoint.");
  }

  return lines;
}
