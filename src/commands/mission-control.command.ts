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
      };
      const homeSnapshotDeps = {
        handoffStore: services.handoffStore,
        config: services.config,
        cass: services.cass,
        git: services.git,
      };

      const missionId = await resolveMissionId(opts.mission);
      const snapshot = missionId
        ? await buildMissionSnapshot(missionId, snapshotDeps)
        : await buildHomeSnapshot(homeSnapshotDeps, process.cwd());

      // --json mode: emit full snapshot
      if (isJson) {
        output(true, snapshot, () => []);
        return;
      }

      // --once mode: render one frame from snapshot
      if (opts.once) {
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

      await renderDashboard({
        snapshot,
        snapshotDeps,
        homeSnapshotDeps,
        missionId,
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
) {
  const services = getServices();
  const mission = await services.missionStore.get(missionId);

  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List available missions: maestro mission list",
    ]);
  }

  return buildSnapshot(snapshotDeps, missionId);
}
