// I/O layer for the MissionControlSnapshot. Reads stores, ingests pending
// replies, and returns a `SnapshotProjectionInput` that the pure projection
// layer (projection.ts) consumes.
import type {
  MissionStorePort,
  FeatureStorePort,
  AssertionStorePort,
  CheckpointStorePort,
  Missions,
} from "@/shared/domain/legacy-mission";
import type { PrincipleStorePort } from "@/features/principle";
import type { AgentReply, ReplyStorePort } from "@/features/reply";
import type { ConfigPort } from "@/infra/ports/config.port.js";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { HandoffStorePort } from "@/features/handoff";
import type { TaskQueryPort } from "@/features/task";
import type { ContractVersionStorePort, ContractStoreQueryPort } from "@/v2/repo/contract-store.port.js";
import type { RunStateStorePort } from "@/v2/repo/run-state-store.port.js";
import type { EvidenceStorePort } from "@/features/evidence";
import type { VerdictStorePort } from "@/features/verdict";
import { buildAutopilotSnapshot } from "./autopilot-screen.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { buildMissionControlEnvironmentSummary } from "./environment-projection.js";
import { buildMissionControlMemorySnapshot } from "./memory-projection.js";
import {
  loadAndIngestReplies,
  loadPrincipleEffectiveness,
  safeListReplies,
} from "./reply-projection.js";
import { buildTaskBoard } from "./task-board.js";
import type {
  SnapshotProjectionInput,
  HomeProjectionInput,
} from "./projection.js";

export interface SnapshotDeps {
  missions: Missions;
  missionStore: MissionStorePort;
  featureStore: FeatureStorePort;
  assertionStore: AssertionStorePort;
  checkpointStore: CheckpointStorePort;
  config: ConfigPort;
  git: GitPort;
  handoffStore?: HandoffStorePort;
  taskStore?: TaskQueryPort;
  evidenceStore?: EvidenceStorePort;
  replyStore?: ReplyStorePort;
  principleStore?: PrincipleStorePort;
  verdictStore?: VerdictStorePort;
  runStateStore?: RunStateStorePort;
  contractVersionStore?: ContractVersionStorePort;
  contractStore?: ContractStoreQueryPort;
  cwd: string;
}

export interface HomeSnapshotDeps {
  config: ConfigPort;
  git: GitPort;
  handoffStore?: HandoffStorePort;
  taskStore?: TaskQueryPort;
  evidenceStore?: EvidenceStorePort;
  replyStore?: ReplyStorePort;
  principleStore?: PrincipleStorePort;
  cwd: string;
}

export interface SnapshotBuildOptions {
  includeTaskBoard?: boolean;
  includeReplies?: boolean;
}

export async function loadSnapshotInput(
  deps: SnapshotDeps,
  missionId: string,
  options: SnapshotBuildOptions,
): Promise<SnapshotProjectionInput> {
  const currentProjectRoot = options.includeReplies === true
    ? resolveMaestroProjectRoot(deps.cwd)
    : undefined;
  const taskBoardPromise = options.includeTaskBoard === true
    ? buildTaskBoard(deps.taskStore, deps.evidenceStore)
    : Promise.resolve(undefined);
  const autopilotPromise = (
    deps.taskStore !== undefined
    && deps.verdictStore !== undefined
    && deps.runStateStore !== undefined
    && deps.contractVersionStore !== undefined
    && deps.contractStore !== undefined
  )
    ? buildAutopilotSnapshot(
        {
          taskStore: deps.taskStore,
          verdictStore: deps.verdictStore,
          runStateStore: deps.runStateStore,
          contractVersionStore: deps.contractVersionStore,
          contractStore: deps.contractStore,
        },
        missionId,
      )
    : Promise.resolve(undefined);

  // Ingest replies FIRST when requested, so the features list below reflects
  // post-ingest state (advanced/kicked-back). Without this the inbox appears
  // stale for one poll cycle.
  const ingest = currentProjectRoot !== undefined
    ? await loadAndIngestReplies({
        missionStore: deps.missionStore,
        featureStore: deps.featureStore,
        assertionStore: deps.assertionStore,
        replyStore: deps.replyStore,
        principleStore: deps.principleStore,
        handoffStore: deps.handoffStore,
        cwd: deps.cwd,
      }, missionId, currentProjectRoot)
    : {
        replies: undefined as readonly AgentReply[] | undefined,
        outcomesCache: undefined,
        handoffsCache: undefined,
      };

  const [
    fullState,
    env,
    configLayers,
    gitState,
    memorySnapshot,
    taskBoard,
    autopilot,
  ] = await Promise.all([
    deps.missions.loadFullState(missionId),
    buildMissionControlEnvironmentSummary(deps.config, deps.git, deps.cwd),
    deps.config.loadLayers(resolveMaestroProjectRoot(deps.cwd)),
    deps.git.getState(deps.cwd),
    buildMissionControlMemorySnapshot({ cwd: deps.cwd }),
    taskBoardPromise,
    autopilotPromise,
  ]);

  // Principle effectiveness piggybacks on includeReplies because the reply
  // ingest is what produces most of the decided outcomes. Reuses the
  // in-memory caches from ingest to avoid re-reading outcomes.jsonl.
  const principleEffectiveness = currentProjectRoot !== undefined
    ? await loadPrincipleEffectiveness(deps, currentProjectRoot, ingest.outcomesCache, ingest.handoffsCache)
    : undefined;

  return {
    mission: fullState.mission,
    features: fullState.features,
    assertions: fullState.assertions,
    checkpoints: fullState.checkpoints,
    env,
    configLayers,
    gitState,
    memorySnapshot: memorySnapshot ?? undefined,
    taskBoard: taskBoard ?? undefined,
    replies: ingest.replies,
    principleEffectiveness,
    autopilot,
  };
}

export async function loadHomeSnapshotInput(
  deps: HomeSnapshotDeps,
  options: SnapshotBuildOptions,
): Promise<HomeProjectionInput> {
  const currentProjectRoot = options.includeReplies === true
    ? resolveMaestroProjectRoot(deps.cwd)
    : undefined;
  const taskBoardPromise = options.includeTaskBoard === true
    ? buildTaskBoard(deps.taskStore, deps.evidenceStore)
    : Promise.resolve(undefined);
  // Replies in home mode: list without ingest (home mode has no mission to
  // update). Home surface is purely read-only per Mission Control contracts.
  const repliesPromise = currentProjectRoot !== undefined && deps.replyStore
    ? safeListReplies(deps.replyStore)
    : Promise.resolve(undefined);
  const principleEffectivenessPromise = currentProjectRoot !== undefined
    ? loadPrincipleEffectiveness(deps, currentProjectRoot)
    : Promise.resolve(undefined);
  const [env, configLayers, gitState, memorySnapshot, taskBoard, replies, principleEffectiveness] = await Promise.all([
    buildMissionControlEnvironmentSummary(deps.config, deps.git, deps.cwd),
    deps.config.loadLayers(resolveMaestroProjectRoot(deps.cwd)),
    deps.git.isRepo(deps.cwd).then((isRepo) => isRepo ? deps.git.getState(deps.cwd) : Promise.resolve(undefined)),
    buildMissionControlMemorySnapshot({ cwd: deps.cwd }),
    taskBoardPromise,
    repliesPromise,
    principleEffectivenessPromise,
  ]);

  return {
    env,
    configLayers,
    gitState,
    memorySnapshot: memorySnapshot ?? undefined,
    taskBoard: taskBoard ?? undefined,
    replies,
    principleEffectiveness,
    cwd: deps.cwd,
  };
}
