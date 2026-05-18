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
import { join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import type {
  LatestVerdictSummary,
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
  readonly terse?: boolean;
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
    throw new Error("not initialized -- run 'maestro init'");
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

  const verdictsByTaskId = await readVerdictsByTaskId(allTasks, deps.verdictStore);
  const latestTransitionByTaskId = indexLatestTransitionByTaskId(transitions);

  const project_state = buildProjectState(allTasks, verdictsByTaskId, staleHandoffCount, now);
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
  const maestro_health = deps.terse
    ? fullHealth.entries.filter((e) => e.status !== "ok")
    : fullHealth;

  return {
    maestro_health,
    project_state,
    missions,
    next_ready,
    recent_transitions,
  };
}

async function readVerdictsByTaskId(
  allTasks: readonly Task[],
  verdictStore: VerdictStorePort,
): Promise<ReadonlyMap<string, Verdict>> {
  const entries = await Promise.all(
    allTasks.map(async (t): Promise<[string, Verdict] | undefined> => {
      // A single corrupt verdict file must not poison the entire cold-start
      // view — agents need the rest of the report to triage.
      try {
        const v = await verdictStore.readLatest(t.id);
        return v ? [t.id, v] : undefined;
      } catch {
        return undefined;
      }
    }),
  );
  const map = new Map<string, Verdict>();
  for (const e of entries) if (e) map.set(e[0], e[1]);
  return map;
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
  verdictsByTaskId: ReadonlyMap<string, Verdict>,
  stale_handoff_count: number,
  now: number,
): ProjectVerifiedState {
  let latest: LatestVerdictSummary | undefined;
  for (const v of verdictsByTaskId.values()) {
    if (!latest || v.computedAt.localeCompare(latest.computedAt) > 0) {
      latest = {
        taskId: v.taskId,
        decision: v.decision,
        computedAt: v.computedAt,
      };
    }
  }

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
  };
}

function buildMissionGroups(
  allMissions: readonly Mission[],
  allTasks: readonly Task[],
  verdictsByTaskId: ReadonlyMap<string, Verdict>,
  latestTransitionByTaskId: ReadonlyMap<string, TransitionEvidenceRow>,
): MissionGroup[] {
  // Active missions surfaces "what's still in flight." Tasks that have
  // reached a terminal state (shipped, abandoned) are work history and
  // belong in Recent transitions, not under their parent mission.
  // The deeper fix — mission auto-rollup to `completed` when every child
  // ships — is filed as a follow-up.
  const tasksByMissionId = new Map<string, Task[]>();
  const unscoped: Task[] = [];
  for (const t of allTasks) {
    if (isTerminalTaskState(t.state)) continue;
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
  for (const mission of allMissions) {
    if (!ACTIVE_MISSION_STATUS.has(mission.status)) continue;
    const tasks = (tasksByMissionId.get(mission.id) ?? []).map((t) =>
      enrichTask(t, verdictsByTaskId, latestTransitionByTaskId),
    );
    groups.push({ mission, tasks });
  }

  if (unscoped.length > 0) {
    groups.push({
      mission: { id: "(unscoped)", title: "(unscoped)", synthetic: true },
      tasks: unscoped.map((t) =>
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
  // Repo Task has no priority field. Substitute oldest-ready (smallest
  // updated_at), so tasks that have been ready longest surface first.
  return allTasks
    .filter((t) => t.state === "ready")
    .slice()
    .sort((a, b) => a.updated_at.localeCompare(b.updated_at))[0];
}

async function countStaleHandoffs(
  emitter: HandoffEmitterPort,
  now: number,
): Promise<number> {
  const envelopes = await emitter.list();
  const results = await Promise.all(
    envelopes.map(async (e): Promise<0 | 1> => {
      if (!isStaleEnvelope(e, now)) return 0;
      const pickup = await emitter.getPickup(e.id);
      return pickup ? 0 : 1;
    }),
  );
  let stale = 0;
  for (const r of results) stale += r;
  return stale;
}

function isStaleEnvelope(envelope: HandoffEnvelope, now: number): boolean {
  const createdAt = Date.parse(envelope.created_at);
  if (Number.isNaN(createdAt)) return false;
  return now - createdAt > ONE_DAY_MS;
}
