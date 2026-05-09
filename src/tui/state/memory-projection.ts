import { basename } from "node:path";
import { buildMemoryStats, type CorrectionStorePort, type LearningStorePort } from "@/features/memory";
import { getGraphContext, type ProjectGraphStorePort } from "@/features/graph";
import type { RatchetStorePort } from "@/features/memory-ratchet";
import { cached, setCachedEntry, type CacheEntry } from "@/tui/state/snapshot-poll-cache.js";
import type { MissionControlMemorySnapshot } from "./types.js";

const MEMORY_SNAPSHOT_TTL_MS = 30_000;
const memorySnapshotCache = new Map<string, CacheEntry<MissionControlMemorySnapshot | null>>();

export async function buildMissionControlMemorySnapshot(
  deps: {
    correctionStore?: CorrectionStorePort;
    learningStore?: LearningStorePort;
    ratchetStore?: RatchetStorePort;
    projectGraphStore?: ProjectGraphStorePort;
    cwd: string;
  },
): Promise<MissionControlMemorySnapshot | null> {
  if (!deps.correctionStore || !deps.learningStore || !deps.ratchetStore) {
    return null;
  }

  const hit = cached(memorySnapshotCache.get(deps.cwd));
  if (hit !== undefined) return hit;

  const [corrections, rawLearnings, compiledLearnings, ratchetSuite, ratchetBaseline, graphContext] = await Promise.all([
    deps.correctionStore.list(),
    deps.learningStore.listRaw(),
    deps.learningStore.readCompiled(),
    deps.ratchetStore.getSuite(),
    deps.ratchetStore.getBaseline(),
    deps.projectGraphStore
      ? getGraphContext(deps.projectGraphStore, basename(deps.cwd))
      : Promise.resolve(undefined),
  ]);
  const stats = buildMemoryStats({
    corrections,
    rawLearningCount: rawLearnings.length,
    compiledLearnings,
    ratchetSuite,
    ratchetBaseline,
    graphProjects: graphContext?.totalProjects ?? 0,
    graphLinks: graphContext?.totalEdges ?? 0,
  });

  const result: MissionControlMemorySnapshot = {
    stats,
    corrections,
    rawLearnings,
    compiledLearnings,
    ratchetSuite,
    ratchetBaseline,
    graphContext: graphContext
      ? {
          currentProject: graphContext.currentProject,
          relationships: graphContext.relationships.map((relationship) => ({
            project: relationship.project,
            direction: relationship.direction,
            edge: relationship.edge,
          })),
          totalProjects: graphContext.totalProjects,
          totalEdges: graphContext.totalEdges,
        }
      : undefined,
  };
  setCachedEntry(memorySnapshotCache, deps.cwd, result, MEMORY_SNAPSHOT_TTL_MS);
  return result;
}
