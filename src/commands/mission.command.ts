/**
 * Mission command handler
 * Implements CLI commands: mission create|list|show|approve|reject|update
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output, resolveJsonFlag } from "../lib/output.js";
import {
  createMission,
  expandWorkflowTemplate,
  listMissions,
  showMission,
  approveMission,
  rejectMission,
  updateMission,
  type CreateMissionResult,
} from "../usecases/mission-lifecycle.usecase.js";
import { generateMissionReport, type MissionReport } from "../usecases/mission-report.usecase.js";
import { MaestroError } from "../domain/errors.js";
import { readText } from "../lib/fs.js";
import type { Mission, UpdateMissionInput, MissionStatus } from "../domain/mission-types.js";

const DEFAULT_TEXT_MISSION_LIST_LIMIT = 10;

interface MissionListTextView {
  readonly visibleMissions: readonly Mission[];
  readonly totalMissions: number;
  readonly truncated: boolean;
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
    .option("--workflow <template>", "Use a workflow template for milestone structure")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (!opts.file) {
        throw new MaestroError("--file is required", [
          "Usage: maestro mission create --file plan.json",
          "Or: maestro mission create --file plan.json --workflow plan-review-implement-review",
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
            throw new MaestroError("Invalid JSON from stdin", [
              "Provide a valid mission plan JSON document on stdin",
            ]);
          }
        } else {
          const content = await readText(opts.file);
          if (content === undefined) {
            throw new MaestroError(`Plan file not found: ${opts.file}`);
          }
          try {
            planData = JSON.parse(content);
          } catch {
            throw new MaestroError(`Invalid JSON in plan file: ${opts.file}`, [
              "Fix the JSON syntax and retry mission creation",
            ]);
          }
        }

      // Inject milestones from workflow template if specified
      if (opts.workflow) {
        const plan = planData as Record<string, unknown>;
        if (plan.milestones && Array.isArray(plan.milestones) && plan.milestones.length > 0) {
          throw new MaestroError(
            "Cannot use --workflow with a plan that already defines milestones",
            ["Remove the milestones array from the plan file, or remove --workflow"],
          );
        }
        const config = await services.config.load(process.cwd());
        plan.milestones = expandWorkflowTemplate(opts.workflow, config);
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
    .option("--limit <number>", "Limit the number of missions shown")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const hasExplicitLimit = opts.limit !== undefined;

      const missions = await listMissions(services.missionStore, {
        status: opts.status,
        limit: hasExplicitLimit ? Number.parseInt(String(opts.limit), 10) : undefined,
      });

      if (isJson) {
        output(true, missions, () => []);
        return;
      }

      output(false, createMissionListTextView(missions, hasExplicitLimit), formatMissionList);
    });

  missionCmd
    .command("show <id>")
    .description("Show mission details with milestone progress")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const report = await generateMissionReport(
        services.missionStore,
        services.featureStore,
        services.assertionStore,
        id,
      );

      output(isJson, report, formatMissionReport);
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
    .option("--status <status>", "New status (draft, approved, rejected, executing, paused, validating, completed, failed)")
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
function formatMissionList(view: MissionListTextView): string[] {
  if (view.totalMissions === 0) {
    return ["No missions found"];
  }

  const lines: string[] = [
    view.truncated
      ? `${view.visibleMissions.length} newest mission(s) shown (total: ${view.totalMissions})`
      : `${view.visibleMissions.length} mission(s)`,
    "",
  ];

  for (const m of view.visibleMissions) {
    const status = m.status.padEnd(12);
    const title = m.title.slice(0, 40).padEnd(40);
    lines.push(`${m.id}  ${status}  ${title}`);
  }

  if (view.truncated) {
    lines.push("");
    lines.push(`Output truncated to the newest ${DEFAULT_TEXT_MISSION_LIST_LIMIT} missions.`);
    lines.push(`Use 'maestro mission list --limit ${view.totalMissions}' to see more.`);
  }

  return lines;
}

function createMissionListTextView(
  missions: readonly Mission[],
  hasExplicitLimit: boolean,
): MissionListTextView {
  if (hasExplicitLimit || missions.length <= DEFAULT_TEXT_MISSION_LIST_LIMIT) {
    return {
      visibleMissions: missions,
      totalMissions: missions.length,
      truncated: false,
    };
  }

  return {
    visibleMissions: missions.slice(0, DEFAULT_TEXT_MISSION_LIST_LIMIT),
    totalMissions: missions.length,
    truncated: true,
  };
}

/** Format mission report with milestone progress for text output */
function formatMissionReport(report: MissionReport): string[] {
  const { mission, effectiveMissionStatus, milestones, summary } = report;
  
  const lines: string[] = [
    `Mission: ${mission.id}`,
    `  Title: ${mission.title}`,
    `  Status: ${effectiveMissionStatus}`,
    `  Created: ${mission.createdAt}`,
    `  Updated: ${mission.updatedAt}`,
  ];

  if (effectiveMissionStatus !== mission.status) {
    lines.push(`  Stored status: ${mission.status}`);
  }

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
  lines.push("Progress Summary:");
  lines.push(`  Features: ${summary.totalCompletedFeatures}/${summary.totalFeatures} (${summary.overallFeaturePct}%)`);
  lines.push(`  Assertions: ${summary.totalTerminalAssertions}/${summary.totalAssertions} (${summary.overallAssertionPct}%)`);
  if (summary.totalWaivedAssertions > 0) {
    lines.push(`  Waived Assertions: ${summary.totalWaivedAssertions}`);
  }

  // Compact progress line
  const compactParts = milestones.map((mp) => {
    const id = mp.milestoneId;
    const st = mp.status;
    if (st === "pending" || st === "sealed") return `[${id}: ${st}]`;
    return `[${id}: ${st} ${mp.completedFeatures}/${mp.featureCount}]`;
  });
  lines.push("");
  lines.push(`Progress: ${compactParts.join(" ")}`);

  lines.push("");
  lines.push(`Milestones (${milestones.length}):`);
  lines.push("");

  for (const m of milestones) {
    const status = m.status.padEnd(12);
    lines.push(`${m.order + 1}. ${m.milestone.id}  ${status}  ${m.milestone.title}`);
    lines.push(`   Features: ${m.completedFeatures}/${m.featureCount} (${m.featureCompletionPct}%)`);
    lines.push(`   Assertions: ${m.terminalAssertions}/${m.assertionCount} (${m.assertionCompletionPct}%)`);
    
    if (m.waivedAssertions > 0) {
      lines.push(`   Waived: ${m.waivedAssertions} assertion(s)`);
    }
    lines.push("");
  }

  lines.push(`Features (${mission.features.length}):`);
  for (const fid of mission.features) {
    lines.push(`  - ${fid}`);
  }

  return lines;
}
