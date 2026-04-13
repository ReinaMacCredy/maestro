/**
 * Mission Control command handler
 * Registers: maestro mission-control [--mission <id>] [--json] [--preview [screen]]
 */
import type { Command } from "commander";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import type { MissionStorePort } from "@/features/mission";
import {
  renderDashboard,
  renderPreviewFrame,
  runRenderCheck,
} from "@/tui/opentui/index.js";
import {
  buildHomeSnapshot,
  buildSnapshot,
  type SnapshotBuildOptions,
} from "@/tui/state/snapshot.js";
import { CachingGitPort, CachingConfigPort } from "@/tui/lib/snapshot-poll-cache.js";
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
  handoff: "handoffs",
  handoffs: "handoffs",
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
  help: "help",
};

export function registerMissionControlCommand(program: Command): void {
  program
    .command("mission-control")
    .description("Interactive mission control dashboard")
    .option("--mission <id>", "Mission ID (auto-selects if omitted)")
    .option("--json", "Output snapshot as JSON")
      .option("--preview [screen]", `Render a read-only preview frame (${PREVIEW_SCREENS.join(", ")}; aliases: feat, handoff, cfg, deps, mem)`)
      .option("--feature <id>", "Select a feature for dashboard, features, or dependencies previews")
    .option("--handoff <id>", "Select a handoff for handoffs previews")
    .option("--size <WxH>", "Render dimensions (e.g. 120x40); overrides terminal detection")
    .option("--format <type>", "Output format: plain or ansi (default: auto-detect TTY)")
    .option("--render-check", "Validate all preview screens and report results as JSON")
    .addHelpText("after", `
  Examples:
    maestro mission-control --preview
    maestro mission-control --preview all --size 120x40 --format plain
    maestro mission-control --preview features --size 200x60
    maestro mission-control --mission <id> --preview dependencies --feature <id>
    maestro mission-control --preview handoffs --handoff <id>
    maestro mission-control --render-check
    maestro mission-control --render-check --size 120x40
    maestro mission-control --json
  `)
        .action(async (opts) => {
          const isJson = resolveJsonFlag(opts, program);
          const previewScreen = resolvePreviewScreen(opts.preview);
          const renderSize = parseSize(opts.size);
          const renderFormat = validateFormat(opts.format);

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

        if ((opts.feature || opts.handoff) && (!previewScreen || previewScreen === "all" || opts.renderCheck)) {
          throw new MaestroError("Preview selectors require a single --preview screen", [
            "Use `maestro mission-control --preview dashboard --feature <id>`",
            "Use `maestro mission-control --preview handoffs --handoff <id>`",
            "Use `maestro mission-control --preview features --feature <id>`",
          ]);
        }

      const services = getServices();
        const snapshotDeps = {
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
          replyStore: services.replyStore,
          principleStore: services.principleStore,
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
          output(true, await loadReadSnapshot({ includeTaskBoard: true }), () => []);
          return;
        }

            if (opts.renderCheck) {
                const snapshot = await loadReadSnapshot({ includeTaskBoard: true });
              const result = await runRenderCheck(snapshot, {
                width: renderSize?.width,
                height: renderSize?.height,
              });
        console.log(JSON.stringify(result, null, 2));
        return;
      }

          if (previewScreen === "all") {
              const snapshot = await loadReadSnapshot({ includeTaskBoard: true });
            const screens = getAllApplicableScreens(snapshot);
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
                const frame = await renderPreviewFrame({
                  snapshot: await loadReadSnapshot({
                    includeTaskBoard: previewScreen === "tasks",
                  }),
                  screen: previewScreen,
                featureId: opts.feature,
          handoffId: opts.handoff,
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
      `Use one of: all, ${PREVIEW_SCREENS.join(", ")} (aliases: feat, handoff, cfg, deps, mem)`,
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

/**
 * Resolve mission ID: explicit > executing/paused > newest.
 */
async function resolveMissionId(explicit?: string): Promise<string | undefined> {
  const services = getServices();
  return resolveMissionIdFromStore(services.missionStore, explicit);
}

async function resolveMissionIdFromStore(
  missionStore: MissionStorePort,
  explicit?: string,
): Promise<string | undefined> {
  if (explicit) return explicit;

  const ids = await missionStore.listIds();

  if (ids.length === 0) {
    return undefined;
  }

  // Try to find an active mission
  for (const id of ids) {
    const m = await missionStore.get(id);
    if (m && (m.status === "executing" || m.status === "paused")) {
      return m.id;
    }
  }

  // Fall back to newest (first in list, which is sorted newest-first)
  return ids[0]!;
}

async function buildMissionSnapshot(
  missionId: string,
  snapshotDeps: Parameters<typeof buildSnapshot>[0],
  options?: SnapshotBuildOptions,
) {
  const mission = await snapshotDeps.missionStore.get(missionId);

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
      load: async (options) => {
        if (!explicitMissionId && !resolvedMissionId) {
          resolvedMissionId = await resolveMissionIdFromStore(cachedSnapshotDeps.missionStore);
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
) {
  return missionId
    ? buildMissionSnapshot(missionId, snapshotDeps, options)
    : buildHomeSnapshot(snapshotDeps, options);
}

function redactSnapshotForReadOutput(
  snapshot: MissionControlSnapshot,
): MissionControlSnapshot {
  const redactPendingHandoff = (
    handoff: MissionControlSnapshot["pendingHandoffs"][number],
    ) => ({
      id: handoff.id,
      agent: handoff.agent,
      timestamp: handoff.timestamp,
      message: "Details hidden in read-only output",
    });
  const redactEventStreamEntry = (
    entry: NonNullable<MissionControlSnapshot["eventStream"]>[number],
  ) => entry.kind === "handoff"
    ? {
        ...entry,
        detail: "Details hidden in read-only output",
      }
    : entry;

  return {
    ...snapshot,
    eventStream: snapshot.eventStream?.map(redactEventStreamEntry),
    pendingHandoffs: snapshot.pendingHandoffs.map(redactPendingHandoff),
    home: snapshot.home
      ? {
        ...snapshot.home,
        pendingHandoffs: snapshot.home.pendingHandoffs.map(redactPendingHandoff),
      }
      : null,
  };
}
