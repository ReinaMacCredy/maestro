/**
 * Milestone command handler
 * Implements CLI commands: milestone list|status|seal
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output } from "../lib/output.js";
import {
  listMilestones,
  getMilestoneStatus,
  sealMilestone,
  type ListMilestonesResult,
  type GetMilestoneStatusResult,
  type SealMilestoneResult,
} from "../usecases/milestone-lifecycle.usecase.js";
import { MaestroError } from "../domain/errors.js";

/** Resolve --json flag from leaf, group, or root options */
function resolveJsonFlag(opts: Record<string, unknown>, program: Command): boolean {
  // Leaf option takes precedence
  if (opts.json !== undefined) return opts.json as boolean;
  // Then group option
  if (opts.jsonGroup !== undefined) return opts.jsonGroup as boolean;
  // Then root option
  return program.opts().json as boolean ?? false;
}

export function registerMilestoneCommand(program: Command): void {
  const milestoneCmd = program
    .command("milestone")
    .description("Milestone lifecycle management")
    .option("--json", "Output as JSON");

  milestoneCmd
    .command("list")
    .description("List all milestones for a mission with progress")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await listMilestones(
        services.missionStore,
        services.featureStore,
        services.assertionStore,
        opts.mission,
      );

      output(isJson, result, formatMilestoneList);
    });

  milestoneCmd
    .command("status <milestoneId>")
    .description("Show detailed status for a specific milestone")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--json", "Output as JSON")
    .action(async (milestoneId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await getMilestoneStatus(
        services.missionStore,
        services.featureStore,
        services.assertionStore,
        opts.mission,
        milestoneId,
      );

      output(isJson, result, formatMilestoneStatus);
    });

  milestoneCmd
    .command("seal <milestoneId>")
    .description("Seal a milestone after validation (requires all assertions to be passed or waived)")
    .requiredOption("--mission <id>", "Mission ID (required)")
    .option("--json", "Output as JSON")
    .action(async (milestoneId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await sealMilestone(
        services.missionStore,
        services.featureStore,
        services.assertionStore,
        opts.mission,
        milestoneId,
      );

      // If sealing failed due to non-terminal assertions, throw an error with helpful info
      if (!result.sealed) {
        const hints = [
          `Blocking assertions (${result.blockingAssertionIds.length}): ${result.blockingAssertionIds.join(", ")}`,
          "All assertions must be 'passed' or 'waived' to seal a milestone.",
          `Use 'maestro validate show --mission ${opts.mission} --milestone ${milestoneId}' to view assertions`,
          `Use 'maestro validate update <assertionId> --mission ${opts.mission} --status passed' to pass assertions`,
        ];

        if (result.progress.waivedAssertionIds.length > 0) {
          hints.push(`Note: ${result.progress.waivedAssertionIds.length} assertion(s) are currently waived: ${result.progress.waivedAssertionIds.join(", ")}`);
        }

        throw new MaestroError(
          `Cannot seal milestone ${milestoneId}: ${result.blockingAssertionIds.length} assertion(s) not in terminal state`,
          hints,
        );
      }

      output(isJson, result, formatSealResult);
    });
}

/** Format milestone list for text output */
function formatMilestoneList(result: ListMilestonesResult): string[] {
  if (result.milestones.length === 0) {
    return ["No milestones found"];
  }

  const lines: string[] = [
    `Mission: ${result.mission.title} (${result.mission.id})`,
    `Status: ${result.mission.status}`,
    "",
    `${result.milestones.length} milestone(s):`,
    "",
  ];

  for (const m of result.milestones) {
    const status = m.status.padEnd(12);
    lines.push(`${m.milestone.order + 1}. ${m.milestone.id}  ${status}  ${m.milestone.title}`);
    lines.push(`   Features: ${m.completedFeatures}/${m.featureCount} (${m.featureCompletionPct}%)`);
    lines.push(`   Assertions: ${m.terminalAssertions}/${m.assertionCount} (${m.assertionCompletionPct}%)`);
    
    if (m.waivedAssertions > 0) {
      lines.push(`   Waived: ${m.waivedAssertions} assertion(s)`);
    }
    
    lines.push("");
  }

  return lines;
}

/** Format milestone status for text output */
function formatMilestoneStatus(result: GetMilestoneStatusResult): string[] {
  const p = result.progress;
  
  const lines: string[] = [
    `Milestone: ${result.milestone.id}`,
    `  Title: ${result.milestone.title}`,
    `  Order: ${result.milestone.order + 1}`,
    `  Status: ${p.status}`,
    "",
    `  Features: ${p.completedFeatures}/${p.featureCount} completed (${p.featureCompletionPct}%)`,
    `  Assertions: ${p.terminalAssertions}/${p.assertionCount} terminal (${p.assertionCompletionPct}%)`,
    `    - Passed: ${p.passedAssertions}`,
    `    - Waived: ${p.waivedAssertions}`,
  ];

  if (p.waivedAssertionIds.length > 0) {
    lines.push("");
    lines.push("  Waived assertions:");
    for (const id of p.waivedAssertionIds) {
      lines.push(`    - ${id}`);
    }
  }

  return lines;
}

/** Format seal result for text output */
function formatSealResult(result: SealMilestoneResult): string[] {
  const lines: string[] = [
    `[ok] Milestone sealed: ${result.milestone.id}`,
    `  Title: ${result.milestone.title}`,
  ];

  if (result.autoTransitioned) {
    lines.push(`  Auto-transitioned: executing -> validating`);
  }

  lines.push(`  Status: ${result.progress.status}`);
  lines.push(`  All ${result.progress.terminalAssertions} assertion(s) are in terminal state`);

  if (result.progress.waivedAssertions > 0) {
    lines.push(`  Waived assertions: ${result.progress.waivedAssertions}`);
    for (const id of result.progress.waivedAssertionIds) {
      lines.push(`    - ${id}`);
    }
  }

  return lines;
}
