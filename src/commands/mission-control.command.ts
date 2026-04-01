/**
 * Mission Control command handler
 * Registers: maestro mission-control [--mission <id>] [--json] [--preview [screen]]
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output, resolveJsonFlag } from "../lib/output.js";
import { MaestroError } from "../domain/errors.js";
import { buildHomeSnapshot, buildSnapshot } from "../tui/state/snapshot.js";
import type { MissionControlSnapshot } from "../tui/state/types.js";
import { PREVIEW_SCREENS, isPreviewScreen, type PreviewScreen } from "../tui/app/preview-state.js";
import { renderDashboard, renderPreviewFrame } from "../tui/index.js";
import { recoverMissionRuntimeFailures } from "../usecases/runtime-recovery.usecase.js";

export type MissionControlSnapshotLoadMode = "read" | "supervise";

export function registerMissionControlCommand(program: Command): void {
  program
    .command("mission-control")
    .description("Interactive mission control dashboard")
    .option("--mission <id>", "Mission ID (auto-selects if omitted)")
    .option("--json", "Output snapshot as JSON")
    .option("--preview [screen]", `Render a read-only preview frame (${PREVIEW_SCREENS.join(", ")})`)
    .option("--feature <id>", "Select a feature for dashboard, features, or dependencies previews")
    .option("--handoff <id>", "Select a handoff for handoffs previews")
    .addHelpText("after", `
Examples:
  maestro mission-control --preview
  maestro mission-control --mission <id> --preview features
  maestro mission-control --mission <id> --preview dependencies --feature <id>
  maestro mission-control --preview handoffs --handoff <id>
  maestro mission-control --json
`)
    .action(async (opts) => {
      const isJson = resolveJsonFlag(opts, program);
      const previewScreen = resolvePreviewScreen(opts.preview);

      if (isJson && previewScreen) {
        throw new MaestroError("Choose either --json or --preview", [
          "Use `maestro mission-control --json` for machine-readable output",
          "Use `maestro mission-control --preview` for a read-only terminal preview",
        ]);
      }

      if ((opts.feature || opts.handoff) && !previewScreen) {
        throw new MaestroError("Preview selectors require --preview", [
          "Use `maestro mission-control --preview dashboard --feature <id>`",
          "Use `maestro mission-control --preview handoffs --handoff <id>`",
        ]);
      }

      const services = getServices();
      const snapshotDeps = {
        missionStore: services.missionStore,
        featureStore: services.featureStore,
        assertionStore: services.assertionStore,
        checkpointStore: services.checkpointStore,
        handoffStore: services.handoffStore,
        config: services.config,
        cass: services.cass,
        git: services.git,
        runtimeStore: services.runtimeStore,
        cwd: process.cwd(),
      };
      const homeSnapshotDeps = {
        handoffStore: services.handoffStore,
        config: services.config,
        cass: services.cass,
        git: services.git,
      };

      const missionId = await resolveMissionId(opts.mission);
      const loadSnapshot = (mode: MissionControlSnapshotLoadMode) =>
        loadMissionControlSnapshot(
          snapshotDeps,
          homeSnapshotDeps,
          mode,
          missionId,
        );
      const loadReadSnapshot = async (): Promise<MissionControlSnapshot> =>
        redactSnapshotForReadOutput(await loadSnapshot("read"));

      if (isJson) {
        output(true, await loadReadSnapshot(), () => []);
        return;
      }

      if (previewScreen) {
        const frame = renderPreviewFrame({
          snapshot: await loadReadSnapshot(),
          screen: previewScreen,
          featureId: opts.feature,
          handoffId: opts.handoff,
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

      const snapshot = await loadSnapshot("supervise");

      await renderDashboard({
        snapshot,
        snapshotDeps,
        reloadSnapshot: () => loadSnapshot("supervise"),
      });
    });
}

function resolvePreviewScreen(value: unknown): PreviewScreen | undefined {
  if (value === undefined || value === false) return undefined;
  if (value === true) return "dashboard";

  if (typeof value !== "string") {
    throw new MaestroError("Invalid value for --preview", [
      `Use one of: ${PREVIEW_SCREENS.join(", ")}`,
    ]);
  }

  const normalizedValue = value.toLowerCase();
  if (isPreviewScreen(normalizedValue)) {
    return normalizedValue;
  }

  throw new MaestroError(`Unknown preview screen '${value}'`, [
    `Use one of: ${PREVIEW_SCREENS.join(", ")}`,
    "Try `maestro mission-control --preview` for the default dashboard preview",
  ]);
}

/**
 * Resolve mission ID: explicit > executing/paused > newest.
 */
async function resolveMissionId(explicit?: string): Promise<string | undefined> {
  if (explicit) return explicit;

  const services = getServices();
  const ids = await services.missionStore.listIds();

  if (ids.length === 0) {
    return undefined;
  }

  // Try to find an active mission
  for (const id of ids) {
    const m = await services.missionStore.get(id);
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
  mode: MissionControlSnapshotLoadMode,
) {
  const mission = await snapshotDeps.missionStore.get(missionId);

  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List available missions: maestro mission list",
    ]);
  }

  if (mode === "supervise") {
    await recoverMissionRuntimeFailures(
      snapshotDeps.missionStore,
      snapshotDeps.featureStore,
      snapshotDeps.runtimeStore,
      missionId,
    );
  }

  return buildSnapshot(snapshotDeps, missionId);
}

export async function loadMissionControlSnapshot(
  snapshotDeps: Parameters<typeof buildSnapshot>[0],
  homeSnapshotDeps: Parameters<typeof buildHomeSnapshot>[0],
  mode: MissionControlSnapshotLoadMode,
  missionId?: string,
) {
  return missionId
    ? buildMissionSnapshot(missionId, snapshotDeps, mode)
    : buildHomeSnapshot(homeSnapshotDeps, process.cwd());
}

function redactSnapshotForReadOutput(
  snapshot: MissionControlSnapshot,
): MissionControlSnapshot {
  const redactPendingHandoff = (
    handoff: MissionControlSnapshot["pendingHandoffs"][number],
  ) => ({
    id: handoff.id,
    agent: handoff.agent,
    message: "Details hidden in read-only output",
  });

  return {
    ...snapshot,
    pendingHandoffs: snapshot.pendingHandoffs.map(redactPendingHandoff),
    home: snapshot.home
      ? {
        ...snapshot.home,
        pendingHandoffs: snapshot.home.pendingHandoffs.map(redactPendingHandoff),
      }
      : null,
  };
}
