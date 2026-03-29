/**
 * Mission command handler
 * Implements CLI commands: mission create|list|show|approve|reject|update
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output } from "../lib/output.js";
import {
  createMission,
  listMissions,
  showMission,
  approveMission,
  rejectMission,
  updateMission,
  type CreateMissionResult,
} from "../usecases/mission-lifecycle.usecase.js";
import { MaestroError } from "../domain/errors.js";
import { readJson } from "../lib/fs.js";
import type { Mission, UpdateMissionInput, MissionStatus } from "../domain/mission-types.js";

/** Resolve --json flag from leaf, group, or root options */
function resolveJsonFlag(opts: Record<string, unknown>, program: Command): boolean {
  // Leaf option takes precedence
  if (opts.json !== undefined) return opts.json as boolean;
  // Then group option
  if (opts.jsonGroup !== undefined) return opts.jsonGroup as boolean;
  // Then root option
  return program.opts().json as boolean ?? false;
}

export function registerMissionCommand(program: Command): void {
  const missionCmd = program
    .command("mission")
    .description("Mission lifecycle management")
    .option("--json", "Output as JSON");

  missionCmd
    .command("create")
    .description("Create a new mission from a plan file")
    .option("--file <path>", "Path to plan JSON file (use - for stdin)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (!opts.file) {
        throw new MaestroError("--file is required", [
          "Usage: maestro mission create --file plan.json",
          "Or: maestro mission create --file - < plan.json",
        ]);
      }

      // Read plan file
      let planData: unknown;
      if (opts.file === "-") {
        // Read from stdin
        const stdin = await new Response(Bun.stdin).text();
        try {
          planData = JSON.parse(stdin);
        } catch {
          throw new MaestroError("Invalid JSON from stdin");
        }
      } else {
        planData = await readJson<unknown>(opts.file);
        if (!planData) {
          throw new MaestroError(`Plan file not found: ${opts.file}`);
        }
      }

      const result = await createMission(
        services.missionStore,
        services.featureStore,
        services.assertionStore,
        planData as Parameters<typeof createMission>[3],
      );

      output(isJson, result, (r) => [
        `[ok] Mission created: ${r.mission.id}`,
        `  Title: ${r.mission.title}`,
        `  Status: ${r.mission.status}`,
        `  Milestones: ${r.mission.milestones.length}`,
        `  Features: ${r.features.length}`,
      ]);
    });

  missionCmd
    .command("list")
    .description("List all missions")
    .option("--status <status>", "Filter by status (draft, approved, executing, etc.)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const missions = await listMissions(services.missionStore, {
        status: opts.status,
      });

      output(isJson, missions, formatMissionList);
    });

  missionCmd
    .command("show <id>")
    .description("Show mission details")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const mission = await showMission(services.missionStore, id);
      if (!mission) {
        throw new MaestroError(`Mission ${id} not found`, [
          "List missions: maestro mission list",
          `Check that mission ID '${id}' is correct`,
        ]);
      }

      output(isJson, mission, formatMissionDetails);
    });

  missionCmd
    .command("approve <id>")
    .description("Approve a draft mission")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const mission = await approveMission(services.missionStore, id);

      output(isJson, mission, (m) => [
        `[ok] Mission approved: ${m.id}`,
        `  Title: ${m.title}`,
        `  Approved at: ${m.approvedAt}`,
      ]);
    });

  missionCmd
    .command("reject <id>")
    .description("Reject a draft mission")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const mission = await rejectMission(services.missionStore, id);

      output(isJson, mission, (m) => [
        `[ok] Mission rejected: ${m.id}`,
        `  Title: ${m.title}`,
        `  Rejected at: ${m.rejectedAt}`,
      ]);
    });

  missionCmd
    .command("update <id>")
    .description("Update mission status or metadata")
    .option("--status <status>", "New status (draft, approved, executing, validating, completed, failed)")
    .option("--title <title>", "New title")
    .option("--description <desc>", "New description")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const input: UpdateMissionInput = {
        ...(opts.status && { status: opts.status as MissionStatus }),
        ...(opts.title && { title: opts.title }),
        ...(opts.description && { description: opts.description }),
      };

      if (Object.keys(input).length === 0) {
        throw new MaestroError("No update specified", [
          "Usage: maestro mission update <id> --status <status>",
          "Or: maestro mission update <id> --title <title>",
          "Or: maestro mission update <id> --description <desc>",
        ]);
      }

      const mission = await updateMission(services.missionStore, id, input);

      output(isJson, mission, (m) => [
        `[ok] Mission updated: ${m.id}`,
        `  Status: ${m.status}`,
        `  Title: ${m.title}`,
      ]);
    });
}

/** Format mission list for text output */
function formatMissionList(missions: readonly Mission[]): string[] {
  if (missions.length === 0) {
    return ["No missions found"];
  }

  const lines: string[] = [`${missions.length} mission(s)`, ""];

  for (const m of missions) {
    const status = m.status.padEnd(12);
    const title = m.title.slice(0, 40).padEnd(40);
    lines.push(`${m.id}  ${status}  ${title}`);
  }

  return lines;
}

/** Format mission details for text output */
function formatMissionDetails(mission: Mission): string[] {
  const lines: string[] = [
    `Mission: ${mission.id}`,
    `  Title: ${mission.title}`,
    `  Status: ${mission.status}`,
    `  Created: ${mission.createdAt}`,
    `  Updated: ${mission.updatedAt}`,
  ];

  if (mission.approvedAt) {
    lines.push(`  Approved: ${mission.approvedAt}`);
  }
  if (mission.rejectedAt) {
    lines.push(`  Rejected: ${mission.rejectedAt}`);
  }
  if (mission.completedAt) {
    lines.push(`  Completed: ${mission.completedAt}`);
  }

  lines.push("");
  lines.push(`Description: ${mission.description || "(none)"}`);
  lines.push("");
  lines.push(`Milestones (${mission.milestones.length}):`);

  const sortedMilestones = [...mission.milestones].sort((a, b) => a.order - b.order);
  for (const ms of sortedMilestones) {
    lines.push(`  ${ms.id}: ${ms.title} (order: ${ms.order})`);
  }

  lines.push("");
  lines.push(`Features (${mission.features.length}):`);
  for (const fid of mission.features) {
    lines.push(`  - ${fid}`);
  }

  return lines;
}
