/**
 * Mission Control command handler
 * Registers: maestro mission-control [--mission <id>] [--json] [--once]
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output, resolveJsonFlag } from "../lib/output.js";
import { MaestroError } from "../domain/errors.js";
import { buildHomeSnapshot, buildSnapshot } from "../tui/snapshot.js";
import { renderDashboard, renderOnceFrame } from "../tui/index.js";
import { recoverMissionRuntimeFailures } from "../usecases/runtime-recovery.usecase.js";

export type MissionControlSnapshotLoadMode = "read" | "supervise";

export function registerMissionControlCommand(program: Command): void {
  program
    .command("mission-control")
    .description("Interactive mission control dashboard")
    .option("--mission <id>", "Mission ID (auto-selects if omitted)")
    .option("--json", "Output snapshot as JSON")
    .option("--once", "Render one plain-text frame and exit")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

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
        if (isJson) {
          const snapshot = await loadMissionControlSnapshot(snapshotDeps, homeSnapshotDeps, "read", missionId);
          output(true, snapshot, () => []);
          return;
        }

        if (opts.once) {
          const snapshot = await loadMissionControlSnapshot(snapshotDeps, homeSnapshotDeps, "read", missionId);
          const frame = renderOnceFrame({ snapshot });
          console.log(frame);
          return;
      }

      // Interactive mode: requires TTY
      if (!process.stdout.isTTY || !process.stdin.isTTY) {
        throw new MaestroError("Interactive mode requires TTY input and output", [
          "Use --once for non-interactive output",
          "Use --json for machine-readable output",
          ]);
        }

        const snapshot = await loadMissionControlSnapshot(snapshotDeps, homeSnapshotDeps, "supervise", missionId);

        await renderDashboard({
          snapshot,
          snapshotDeps,
          reloadSnapshot: () => loadMissionControlSnapshot(snapshotDeps, homeSnapshotDeps, "supervise", missionId),
        });
      });
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
