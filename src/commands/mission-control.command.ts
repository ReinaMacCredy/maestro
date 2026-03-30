/**
 * Mission Control command handler
 * Registers: maestro mission-control [--mission <id>] [--json] [--once]
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output, resolveJsonFlag } from "../lib/output.js";
import { MaestroError } from "../domain/errors.js";
import { buildSnapshot } from "../tui/snapshot.js";
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

      const missionId = await resolveMissionId(opts.mission);
      const mission = await services.missionStore.get(missionId);

      if (!mission) {
        throw new MaestroError(`Mission ${missionId} not found`, [
          "List available missions: maestro mission list",
        ]);
      }

      const snapshotDeps = {
        missionStore: services.missionStore,
        featureStore: services.featureStore,
        assertionStore: services.assertionStore,
        checkpointStore: services.checkpointStore,
      };

      // --json mode: emit full snapshot
      if (isJson) {
        const snapshot = await buildSnapshot(snapshotDeps, missionId);
        output(true, snapshot, () => []);
        return;
      }

      // --once mode: render one frame from snapshot
      if (opts.once) {
        const snapshot = await buildSnapshot(snapshotDeps, missionId);
        const frame = renderOnceFrame({ snapshot });
        console.log(frame);
        return;
      }

      // Interactive mode: requires TTY
      if (!process.stdout.isTTY) {
        throw new MaestroError("Interactive mode requires a TTY", [
          "Use --once for non-interactive output",
          "Use --json for machine-readable output",
        ]);
      }

      await renderDashboard({
        snapshot: await buildSnapshot(snapshotDeps, missionId),
        snapshotDeps,
        missionId,
      });
    });
}

/**
 * Resolve mission ID: explicit > executing/paused > newest.
 */
async function resolveMissionId(explicit?: string): Promise<string> {
  if (explicit) return explicit;

  const services = getServices();
  const ids = await services.missionStore.listIds();

  if (ids.length === 0) {
    throw new MaestroError("No missions found", [
      "Create a mission first: maestro mission create --file plan.json",
    ]);
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
