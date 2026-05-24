import type { TaskStorePort } from "@/repo/task-store.port.js";
import type {
  MissionStatus,
  Mission,
  MissionStorePort,
} from "@/shared/domain/legacy-mission";
import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
import type {
  EvidenceStorePort,
  TransitionEvidenceRow,
} from "@/repo/evidence-store.port.js";
import type {
  HandoffEmitterPort,
  HandoffEnvelope,
} from "@/repo/handoff-emitter.port.js";
import type { Task } from "@/types/task.js";
import { isTerminalTaskState } from "@/types/task-state.js";
import { dirExists } from "@/shared/lib/fs.js";
import { setupCheck } from "@/service/setup-check.usecase.js";
import {
  loadLatestVerdictsByTask,
  type LatestVerdictSummary,
} from "@/service/load-latest-verdicts.usecase.js";
import { join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import type {
  MissionGroup,
  ProjectVerifiedState,
  StatusReport,
  TaskSignal,
  TaskWithSignal,
} from "@/infra/domain/status-types.js";

export interface BuildStatusReportDeps {
  readonly taskStore: TaskStorePort;
  readonly featureMissionStore: MissionStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly handoffEmitter: HandoffEmitterPort;
  readonly projectDir: string;
}

const ONE_DAY_MS = 86_400_000;

const ACTIVE_MISSION_STATUS: ReadonlySet<MissionStatus> = new Set<MissionStatus>([
  "draft",
  "approved",
  "executing",
  "paused",
  "validating",
]);

export async function buildStatusReport(
  deps: BuildStatusReportDeps,
): Promise<StatusReport> {
  const maestroDir = join(deps.projectDir, MAESTRO_DIR);
  if (!(await dirExists(maestroDir))) {
    throw new Error("not initialized -- run 'maestro setup'");
  }

  const now = Date.now();
  const [fullHealth, allTasks, allMissions, transitionRows, staleHandoffCount] =
    await Promise.all([
      setupCheck({ repoRoot: deps.projectDir }),
      deps.taskStore.list(),
      deps.featureMissionStore.list(),
      deps.evidenceStore.list({ kind: "transition" }),
      countStaleHandoffs(deps.handoffEmitter, now),
    ]);

  const transitions = transitionRows.filter(
    (r): r is TransitionEvidenceRow => r.kind === "transition",
  );

  const { byTaskId: verdictsByTaskId, latest: latestVerdict, corruptCount } =
    await loadLatestVerdictsByTask(allTasks, deps.verdictStore);
  const latestTransitionByTaskId = indexLatestTransitionByTaskId(transitions);

  const project_state = buildProjectState(
    allTasks,
    latestVerdict,
    staleHandoffCount,
    corruptCount,
    now,
  );
  const missions = buildMissionGroups(
    allMissions,
    allTasks,
    verdictsByTaskId,
    latestTransitionByTaskId,
  );
  const next_ready = pickNextReady(allTasks);
  const recent_transitions = transitions
    .slice()
    .sort((a, b) => b.timestamp.localeCompare(a.timestamp))
    .slice(0, 10);

  return {
    maestro_health: fullHealth,
    project_state,
    missions,
    next_ready,
    recent_transitions,
  };
}

function indexLatestTransitionByTaskId(
  transitions: readonly TransitionEvidenceRow[],
): ReadonlyMap<string, TransitionEvidenceRow> {
  const map = new Map<string, TransitionEvidenceRow>();
  for (const row of transitions) {
    if (!row.task_id) continue;
    const prev = map.get(row.task_id);
    if (!prev || row.timestamp.localeCompare(prev.timestamp) > 0) {
      map.set(row.task_id, row);
    }
  }
  return map;
}

function buildProjectState(
  allTasks: readonly Task[],
  latest: LatestVerdictSummary | undefined,
  stale_handoff_count: number,
  corrupt_verdict_count: number,
  now: number,
): ProjectVerifiedState {
  const stuck_verifying_count = allTasks.filter((t) => {
    if (t.state !== "verifying") return false;
    const updated = Date.parse(t.updated_at);
    if (Number.isNaN(updated)) return false;
    return now - updated > ONE_DAY_MS;
  }).length;

  return {
    latest_verdict: latest,
    stuck_verifying_count,
    stale_handoff_count,
    corrupt_verdict_count,
  };
}

function buildMissionGroups(
  allMissions: readonly Mission[],
  allTasks: readonly Task[],
  verdictsByTaskId: ReadonlyMap<string, Verdict>,
  latestTransitionByTaskId: ReadonlyMap<string, TransitionEvidenceRow>,
): MissionGroup[] {
  const activeTasks = filterActiveTasksForMissions(allTasks);
  const tasksByMissionId = new Map<string, Task[]>();
  const unscoped: Task[] = [];
  for (const t of activeTasks) {
    if (t.mission_id === undefined) {
      unscoped.push(t);
    } else {
      let bucket = tasksByMissionId.get(t.mission_id);
      if (!bucket) {
        bucket = [];
        tasksByMissionId.set(t.mission_id, bucket);
      }
      bucket.push(t);
    }
  }

  const groups: MissionGroup[] = [];
  const consumedMissionIds = new Set<string>();
  for (const mission of allMissions) {
    if (!ACTIVE_MISSION_STATUS.has(mission.status)) continue;
    const tasks = (tasksByMissionId.get(mission.id) ?? []).map((t) =>
      enrichTask(t, verdictsByTaskId, latestTransitionByTaskId),
    );
    consumedMissionIds.add(mission.id);
    groups.push({ mission, tasks });
  }

  // Tasks whose mission_id points at a non-active mission (e.g. completed
  // mission archived but task still open) or a missing mission would
  // otherwise be silently dropped. Route them through the unscoped group so
  // they remain visible to operators.
  const orphaned: Task[] = [];
  for (const [missionId, bucket] of tasksByMissionId) {
    if (!consumedMissionIds.has(missionId)) orphaned.push(...bucket);
  }
  const otherTasks = [...unscoped, ...orphaned];
  if (otherTasks.length > 0) {
    groups.push({
      mission: { id: "(unscoped)", title: "(unscoped)", synthetic: true },
      tasks: otherTasks.map((t) =>
        enrichTask(t, verdictsByTaskId, latestTransitionByTaskId),
      ),
    });
  }

  return groups;
}

function enrichTask(
  task: Task,
  verdictsByTaskId: ReadonlyMap<string, Verdict>,
  latestTransitionByTaskId: ReadonlyMap<string, TransitionEvidenceRow>,
): TaskWithSignal {
  const verdict = verdictsByTaskId.get(task.id);
  if (verdict) {
    return {
      task,
      signal: {
        kind: "verdict",
        decision: verdict.decision,
        computedAt: verdict.computedAt,
      } satisfies TaskSignal,
    };
  }
  const latest = latestTransitionByTaskId.get(task.id);
  if (latest) {
    return {
      task,
      signal: {
        kind: "transition",
        to_state: String(latest.to_state),
        trigger_verb: latest.trigger_verb,
        timestamp: latest.timestamp,
      } satisfies TaskSignal,
    };
  }
  return { task, signal: { kind: "none" } };
}

function pickNextReady(allTasks: readonly Task[]): Task | undefined {
  // Repo Task has no priority field. Sort by `created_at` (immutable) so
  // post-ready mutations (assignee swap, blocker delta) don't reshuffle the
  // pick. `updated_at` would be wrong here — every #mutate touches it.
  return allTasks
    .filter((t) => t.state === "ready")
    .slice()
    .sort((a, b) => a.created_at.localeCompare(b.created_at))[0];
}

async function countStaleHandoffs(
  emitter: HandoffEmitterPort,
  now: number,
): Promise<number> {
  const [envelopes, pickups] = await Promise.all([
    emitter.list(),
    emitter.listPickups(),
  ]);
  const pickedUpIds = new Set(pickups.map((p) => p.envelope_id));
  return envelopes.filter(
    (e) => isStaleEnvelope(e, now) && !pickedUpIds.has(e.id),
  ).length;
}

function isStaleEnvelope(envelope: HandoffEnvelope, now: number): boolean {
  const createdAt = Date.parse(envelope.created_at);
  if (Number.isNaN(createdAt)) return false;
  return now - createdAt > ONE_DAY_MS;
}

/**
 * Filter out terminal-state tasks (shipped, abandoned) from mission display.
 * Terminal tasks are work history and belong in Recent transitions, not under
 * their parent mission in the Active missions section.
 */
function filterActiveTasksForMissions(tasks: readonly Task[]): Task[] {
  return tasks.filter((t) => !isTerminalTaskState(t.state));
}
