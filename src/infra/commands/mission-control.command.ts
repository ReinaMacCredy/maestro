/**
 * Mission Control command handler
 * Registers: maestro mission-control [--mission <id>] [--json] [--preview [screen]]
 */
import type { Command } from "commander";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
// renderDashboard / renderPreviewFrame / runRenderCheck pull the OpenTUI +
// React graph (~250-400ms cold start). --json mode never renders, so we
// dynamic-import these only inside the branches that actually need them.
// Saves ~half the wall time on `mission-control --json`, which is the
// agent-facing snapshot path.
import {
  buildHomeSnapshot,
  buildSnapshot,
  type SnapshotBuildOptions,
} from "@/tui/state/snapshot.js";
import { buildMissionControlSnapshotDemand } from "@/tui/state/snapshot-demand.js";
import { CachingGitPort, CachingConfigPort } from "@/tui/state/snapshot-poll-cache.js";
import type { MissionControlSnapshot } from "@/tui/state/types.js";
import {
  PREVIEW_SCREENS,
  getApplicablePreviewScreens,
  isPreviewScreen,
  type PreviewScreen,
} from "@/tui/app/preview-state.js";

export interface MissionControlSnapshotLoader {
  load: (options?: SnapshotBuildOptions) => Promise<MissionControlSnapshot>;
}

type PreviewScreenOrAll = PreviewScreen | "all";

const PREVIEW_SCREEN_ALIASES: Readonly<Record<string, PreviewScreenOrAll>> = {
  all: "all",
  dash: "dashboard",
  dashboard: "dashboard",
  home: "dashboard",
  feat: "features",
  feature: "features",
  features: "features",
  dep: "dependencies",
  deps: "dependencies",
  dependency: "dependencies",
  dependencies: "dependencies",
  cfg: "config",
  config: "config",
  settings: "config",
  memory: "memory",
  mem: "memory",
  graph: "graph",
  agents: "agents",
  agent: "agents",
  dispatch: "dispatch",
  events: "events",
  event: "events",
  tasks: "tasks",
  task: "tasks",
  timeline: "timeline",
  principles: "principles",
  principle: "principles",
  help: "help",
  autopilot: "autopilot",
};

export function registerMissionControlCommand(program: Command): void {
  program
    .command("mission-control")
    .description("Interactive mission control dashboard")
    .option("--mission <id>", "Mission ID (auto-selects if omitted)")
    .option("--json", "Output snapshot as JSON")
      .option("--preview [screen]", `Render a read-only preview frame (${PREVIEW_SCREENS.join(", ")}; aliases: feat, cfg, deps, mem)`)
      .option("--screen <name>", "Alias for --preview <name> (programmatic surface)")
      .option("--filter <key=value>", "Narrow JSON output by key (supported: task=<id>, feature=<id>)", collectFilters, {})
      .option("--feature <id>", "Select a feature for dashboard, features, or dependencies previews")
    .option("--size <WxH>", "Render dimensions (e.g. 120x40); overrides terminal detection")
    .option("--format <type>", "Output format: plain or ansi (default: auto-detect TTY)")
    .option("--render-check", "Validate all preview screens and report results as JSON")
    .addHelpText("after", `
  Examples:
    maestro mission-control --preview
    maestro mission-control --preview all --size 120x40 --format plain
    maestro mission-control --preview features --size 200x60
    maestro mission-control --mission <id> --preview dependencies --feature <id>
    maestro mission-control --render-check
    maestro mission-control --render-check --size 120x40
    maestro mission-control --json
  `)
        .action(async (opts): Promise<void> => {
          const isJson = resolveJsonFlag(opts, program);
          const previewArg = opts.preview ?? (typeof opts.screen === "string" ? opts.screen : undefined);
          const previewScreen = resolvePreviewScreen(previewArg);
          const renderSize = parseSize(opts.size);
          const renderFormat = validateFormat(opts.format);
          const filters: Record<string, string> = (opts.filter as Record<string, string>) ?? {};

        if (isJson && previewScreen) {
        throw new MaestroError("Choose either --json or --preview", [
          "Use `maestro mission-control --json` for machine-readable output",
          "Use `maestro mission-control --preview` for a read-only terminal preview",
        ]);
      }

      if (opts.renderCheck && (isJson || previewScreen)) {
        throw new MaestroError("--render-check cannot be combined with --json or --preview", [
          "Use `maestro mission-control --render-check` on its own",
          "Use `maestro mission-control --render-check --size 120x40` for specific dimensions",
        ]);
      }

        if (opts.feature && (!previewScreen || previewScreen === "all" || opts.renderCheck)) {
          throw new MaestroError("Preview selectors require a single --preview screen", [
            "Use `maestro mission-control --preview dashboard --feature <id>`",
            "Use `maestro mission-control --preview features --feature <id>`",
          ]);
        }

      const services = getServices();
        const snapshotDeps = {
          missions: services.missions,
          missionStore: services.missionStore,
          featureStore: services.featureStore,
          assertionStore: services.assertionStore,
          checkpointStore: services.checkpointStore,
          config: services.config,
          git: services.git,
          correctionStore: services.correctionStore,
          learningStore: services.learningStore,
          ratchetStore: services.ratchetStore,
          projectGraphStore: services.projectGraphStore,
          handoffStore: services.handoffStore,
          taskStore: services.taskStore,
          evidenceStore: services.evidenceStore,
          replyStore: services.replyStore,
          principleStore: services.principleStore,
          verdictStore: services.verdictStore,
          runStateStore: services.runStateStore,
          contractVersionStore: services.contractVersionStore,
          contractStore: services.contractStore,
          cwd: process.cwd(),
        };
        const snapshotLoader = createMissionControlSnapshotLoader(
          snapshotDeps,
          opts.mission,
        );
        const loadReadSnapshot = async (
          options?: SnapshotBuildOptions,
        ): Promise<MissionControlSnapshot> =>
          redactSnapshotForReadOutput(await snapshotLoader.load(options));

        if (isJson) {
          const snapshot = await loadReadSnapshot(buildMissionControlSnapshotDemand({ mode: "json" }));
          const payload = Object.keys(filters).length > 0
            ? { filter: filters, snapshot, narrow: narrowSnapshot(snapshot, filters) }
            : snapshot;
          output(true, payload, () => []);
          return;
        }

            if (opts.renderCheck) {
                const snapshot = await loadReadSnapshot(buildMissionControlSnapshotDemand({ mode: "render-check" }));
              const { runRenderCheck } = await import("@/tui/opentui/index.js");
              const result = await runRenderCheck(snapshot, {
                width: renderSize?.width,
                height: renderSize?.height,
              });
        console.log(JSON.stringify(result, null, 2));
        return;
      }

          if (previewScreen === "all") {
              const snapshot = await loadReadSnapshot(buildMissionControlSnapshotDemand({ mode: "preview-all" }));
            const screens = getAllApplicableScreens(snapshot);
              const { renderPreviewFrame } = await import("@/tui/opentui/index.js");
              for (const screen of screens) {
                console.log(`--- ${screen} ---`);
                const frame = await renderPreviewFrame({
                  snapshot,
                  screen,
                  width: renderSize?.width,
            height: renderSize?.height,
            format: renderFormat,
          });
          console.log(frame);
        }
        console.log(`--- rendered ${screens.length} screens ---`);
        return;
      }

            if (previewScreen) {
                const { renderPreviewFrame } = await import("@/tui/opentui/index.js");
                const frame = await renderPreviewFrame({
                  snapshot: await loadReadSnapshot(buildMissionControlSnapshotDemand({
                    mode: "preview-screen",
                    screen: previewScreen,
                  })),
                  screen: previewScreen,
                featureId: opts.feature,
          width: renderSize?.width,
          height: renderSize?.height,
          format: renderFormat,
        });
        console.log(frame);
        return;
      }

      if (!process.stdout.isTTY || !process.stdin.isTTY) {
        throw new MaestroError("Interactive mode requires TTY input and output", [
          "Use --preview for non-interactive output",
          "Use --json for machine-readable output",
        ]);
      }

      const snapshot = await snapshotLoader.load();

      const { renderDashboard } = await import("@/tui/opentui/index.js");
      await renderDashboard({
        snapshot,
        snapshotDeps,
        reloadSnapshot: (options) => snapshotLoader.load(options),
      });
    });
}

function resolvePreviewScreen(value: unknown): PreviewScreenOrAll | undefined {
  if (value === undefined || value === false) return undefined;
  if (value === true) return "dashboard";

  if (typeof value !== "string") {
    throw new MaestroError("Invalid value for --preview", [
      `Use one of: all, ${PREVIEW_SCREENS.join(", ")}`,
    ]);
  }

  const normalizedValue = value.toLowerCase();
  if (normalizedValue === "all") return "all";
  if (isPreviewScreen(normalizedValue)) {
    return normalizedValue;
  }

  const aliasedValue = PREVIEW_SCREEN_ALIASES[normalizedValue];
  if (aliasedValue) {
    return aliasedValue;
  }

  throw new MaestroError(`Unknown preview screen '${value}'`, [
      `Use one of: all, ${PREVIEW_SCREENS.join(", ")} (aliases: feat, cfg, deps, mem)`,
      "Try `maestro mission-control --preview` for the default dashboard preview",
    ]);
}

function parseSize(value: unknown): { width: number; height: number } | undefined {
  if (!value || typeof value !== "string") return undefined;
  const match = value.match(/^(\d+)x(\d+)$/i);
  if (!match) {
    throw new MaestroError(`Invalid --size format '${value}'`, [
      "Use WxH format, e.g. --size 120x40",
      "Width and height must be positive integers",
    ]);
  }
  const width = parseInt(match[1]!, 10);
  const height = parseInt(match[2]!, 10);
  if (width < 40 || height < 20) {
    throw new MaestroError(`Size ${width}x${height} is too small for rendering`, [
      "Minimum size: 40x20",
      "Recommended: --size 120x40",
    ]);
  }
  return { width, height };
}

function validateFormat(value: unknown): "plain" | "ansi" | undefined {
  if (!value) return undefined;
  if (value === "plain" || value === "ansi") return value;
  throw new MaestroError(`Invalid --format '${value}'`, [
    "Use --format plain for stripped text output",
    "Use --format ansi for ANSI-styled output",
  ]);
}

function getAllApplicableScreens(snapshot: MissionControlSnapshot): PreviewScreen[] {
  return getApplicablePreviewScreens(snapshot);
}

async function buildMissionSnapshot(
  missionId: string,
  snapshotDeps: Parameters<typeof buildSnapshot>[0],
  options?: SnapshotBuildOptions,
): Promise<MissionControlSnapshot> {
  const mission = await snapshotDeps.missions.get(missionId);

  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List available missions: maestro mission list",
    ]);
  }

  return buildSnapshot(snapshotDeps, missionId, options);
}

export function createMissionControlSnapshotLoader(
  snapshotDeps: Parameters<typeof buildSnapshot>[0],
  explicitMissionId?: string,
): MissionControlSnapshotLoader {
  let resolvedMissionId = explicitMissionId;

  // Wrap I/O-heavy ports with TTL caches. spawnSync leaks memory in Bun
  // 1.3.x; repeated YAML parsing pressures GC when polling every 1-2s.
  const cachingGit = new CachingGitPort(snapshotDeps.git);
  const cachingConfig = new CachingConfigPort(snapshotDeps.config);
  const cachedSnapshotDeps = { ...snapshotDeps, git: cachingGit, config: cachingConfig };

  return {
      load: async (options): Promise<void> => {
        if (!explicitMissionId && !resolvedMissionId) {
          resolvedMissionId = await cachedSnapshotDeps.missions.resolveMissionId();
        }

        return loadMissionControlSnapshot(
          cachedSnapshotDeps,
          resolvedMissionId,
          options,
        );
      },
  };
}

export async function loadMissionControlSnapshot(
  snapshotDeps: Parameters<typeof buildSnapshot>[0],
  missionId?: string,
  options?: SnapshotBuildOptions,
): Promise<MissionControlSnapshot> {
  return missionId
    ? buildMissionSnapshot(missionId, snapshotDeps, options)
    : buildHomeSnapshot(snapshotDeps, options);
}

function redactSnapshotForReadOutput(
  snapshot: MissionControlSnapshot,
): MissionControlSnapshot {
  return {
    ...snapshot,
    eventStream: snapshot.eventStream,
  };
}

function collectFilters(
  raw: string,
  acc: Record<string, string> = {},
): Record<string, string> {
  const idx = raw.indexOf("=");
  if (idx === -1) {
    throw new MaestroError(`Invalid --filter value \`${raw}\` (missing \`=\`)`, [
      "Use --filter key=value (e.g. --filter task=tsk-abc123)",
    ]);
  }
  const key = raw.slice(0, idx).trim();
  const value = raw.slice(idx + 1).trim();
  if (!key) {
    throw new MaestroError(`Invalid --filter value \`${raw}\` (empty key)`, [
      "Use --filter key=value (e.g. --filter task=tsk-abc123)",
    ]);
  }
  if (!value) {
    throw new MaestroError(`Invalid --filter value \`${raw}\` (empty value for \`${key}\`)`, [
      "Use --filter key=value (e.g. --filter task=tsk-abc123)",
    ]);
  }
  return { ...acc, [key]: value };
}

export interface NarrowResult {
  readonly task?: {
    readonly id: string;
    readonly status?: string;
    readonly title?: string;
  };
  readonly feature?: {
    readonly id: string;
    readonly title?: string;
    readonly status?: string;
  };
  readonly progressLog: readonly MissionControlSnapshot["progressLog"][number][];
  readonly eventStream: readonly NonNullable<MissionControlSnapshot["eventStream"]>[number][];
}

function findTaskInBoard(
  snapshot: MissionControlSnapshot,
  taskId: string,
): { id: string; title: string; status: string } | undefined {
  const columns = snapshot.taskBoard?.columns;
  if (!columns) return undefined;
  for (const items of Object.values(columns)) {
    const found = items.find((t) => t.id === taskId);
    if (found) return { id: found.id, title: found.title, status: found.status };
  }
  return undefined;
}

function narrowSnapshot(
  snapshot: MissionControlSnapshot,
  filters: Record<string, string>,
): NarrowResult {
  const taskId = filters.task;
  const featureId = filters.feature;
  const matches = (entry: { title: string; detail?: string }): boolean => {
    const haystack = entry.detail ? `${entry.title} ${entry.detail}` : entry.title;
    if (taskId !== undefined && haystack.includes(taskId)) return true;
    if (featureId !== undefined && haystack.includes(featureId)) return true;
    return false;
  };

  const taskRow = taskId
    ? findTaskInBoard(snapshot, taskId)
    : undefined;
  const featureRow = featureId
    ? snapshot.features.find((f) => f.id === featureId)
    : undefined;

  const progressLog = snapshot.progressLog.filter(matches);
  const eventStream = (snapshot.eventStream ?? []).filter(matches);

  return {
    task: taskRow
      ? { id: taskRow.id, status: taskRow.status, title: taskRow.title }
      : undefined,
    feature: featureRow
      ? { id: featureRow.id, title: featureRow.title, status: featureRow.status }
      : undefined,
    progressLog,
    eventStream,
  };
}
