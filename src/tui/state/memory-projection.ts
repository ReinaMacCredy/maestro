import { basename } from "node:path";
import { getGraphContext, type ProjectGraphStorePort } from "@/features/graph";
import { cached, setCachedEntry, type CacheEntry } from "@/tui/state/snapshot-poll-cache.js";
import type { MissionControlMemorySnapshot } from "./types.js";

const MEMORY_SNAPSHOT_TTL_MS = 30_000;
const memorySnapshotCache = new Map<string, CacheEntry<MissionControlMemorySnapshot | null>>();

// Memory/ratchet projection was retired with the v1 memory subsystem.
// This file now only surfaces a project graph snapshot for the home pane.
export async function buildMissionControlMemorySnapshot(
  deps: {
    projectGraphStore?: ProjectGraphStorePort;
    cwd: string;
  },
): Promise<MissionControlMemorySnapshot | null> {
  if (!deps.projectGraphStore) return null;

  const hit = cached(memorySnapshotCache.get(deps.cwd));
  if (hit !== undefined) return hit;

  const graphContext = await getGraphContext(
    deps.projectGraphStore,
    basename(deps.cwd),
  );

  if (!graphContext) {
    setCachedEntry(memorySnapshotCache, deps.cwd, null, MEMORY_SNAPSHOT_TTL_MS);
    return null;
  }

  const result: MissionControlMemorySnapshot = {
    graphContext: {
      currentProject: graphContext.currentProject,
      relationships: graphContext.relationships.map((relationship) => ({
        project: relationship.project,
        direction: relationship.direction,
        edge: relationship.edge,
      })),
      totalProjects: graphContext.totalProjects,
      totalEdges: graphContext.totalEdges,
    },
  };
  setCachedEntry(memorySnapshotCache, deps.cwd, result, MEMORY_SNAPSHOT_TTL_MS);
  return result;
}
