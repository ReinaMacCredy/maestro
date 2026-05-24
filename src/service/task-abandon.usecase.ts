import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { assertTaskTransition, isTerminalTaskState } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";
import { tryAdvanceMission } from "./try-advance-mission.usecase.js";

export class TaskSplitCascadeBlockedError extends Error {
  readonly taskId: TaskId;
  readonly nonTerminalDescendants: readonly TaskId[];
  constructor(taskId: TaskId, nonTerminalDescendants: readonly TaskId[]) {
    super(
      `Cannot abandon ${taskId}: ${nonTerminalDescendants.length} non-terminal descendant(s) (${nonTerminalDescendants.join(", ")}). Re-run with --cascade to abandon them recursively.`,
    );
    this.name = "TaskSplitCascadeBlockedError";
    this.taskId = taskId;
    this.nonTerminalDescendants = nonTerminalDescendants;
  }
}

export interface TaskAbandonDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly missionStore?: MissionStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskAbandonInput {
  readonly id: TaskId;
  readonly reason: string;
  readonly cascade?: boolean;
}

export async function taskAbandon(deps: TaskAbandonDeps, input: TaskAbandonInput): Promise<Task> {
  const existing = await deps.taskStore.get(input.id);
  if (!existing) throw new TaskNotFoundError(input.id);

  // Walk descendants via parent_id graph (post-order, deepest first).
  const allTasks = await deps.taskStore.list();
  const descendants = collectDescendantsPostOrder(allTasks, existing.id);
  const nonTerminal = descendants.filter((d) => !isTerminalTaskState(d.state));

  if (nonTerminal.length > 0) {
    if (input.cascade !== true) {
      throw new TaskSplitCascadeBlockedError(
        input.id,
        nonTerminal.map((d) => d.id),
      );
    }
    // Cascade: abandon each non-terminal descendant post-order. Recursive call
    // with cascade=false is safe — by post-order, each descendant's own
    // descendants are already terminal by the time we recurse on it.
    for (const d of nonTerminal) {
      await taskAbandon(deps, {
        id: d.id,
        reason: "cascade from parent abandon",
        cascade: false,
      });
    }
  }

  assertTaskTransition(existing.state, "abandoned");
  const updated = await deps.taskStore.update(input.id, {
    state: "abandoned",
    abandon_reason: input.reason,
  });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      observabilityStore: deps.observabilityStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      task_id: existing.id,
      from_state: existing.state,
      to_state: "abandoned",
      trigger_verb: "task:abandon",
      reason: input.reason,
    },
  );

  // pruneBlockedBy: best-effort, N writes. Not atomic — if a peer mutation
  // races, the prune step may leave stale references. Same risk profile as the
  // existing mission-rollup pattern; acceptable per v2 conventions.
  const peers = await deps.taskStore.list();
  for (const peer of peers) {
    if (peer.id === updated.id) continue;
    if (!peer.blocked_by.includes(updated.id)) continue;
    await deps.taskStore.update(peer.id, {
      blocked_by: peer.blocked_by.filter((b) => b !== updated.id),
    });
  }

  if (deps.missionStore) {
    await tryAdvanceMission(
      {
        missionStore: deps.missionStore,
        taskStore: deps.taskStore,
        evidenceStore: deps.evidenceStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      { mission_id: updated.mission_id, trigger_task_verb: "task:abandon" },
    );
  }
  return updated;
}

function collectDescendantsPostOrder(
  tasks: readonly Task[],
  rootId: TaskId,
): readonly Task[] {
  const childrenByParent = new Map<TaskId, Task[]>();
  for (const t of tasks) {
    if (t.parent_id === undefined) continue;
    const bucket = childrenByParent.get(t.parent_id);
    if (bucket) bucket.push(t);
    else childrenByParent.set(t.parent_id, [t]);
  }
  const out: Task[] = [];
  const visit = (id: TaskId): void => {
    const kids = childrenByParent.get(id) ?? [];
    for (const k of kids) {
      visit(k.id);
      out.push(k);
    }
  };
  visit(rootId);
  return out;
}
