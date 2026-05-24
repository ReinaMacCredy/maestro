import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { type TaskState } from "../types/task-state.js";
import { generateTaskId, type Task, type TaskId } from "../types/task.js";
import { assertMissionActive } from "./assert-mission-active.js";
import { tryAdvanceMission } from "./try-advance-mission.usecase.js";

export class TaskSplitInvalidStateError extends Error {
  readonly taskId: TaskId;
  readonly state: TaskState;
  constructor(taskId: TaskId, state: TaskState) {
    super(`Cannot split task ${taskId} in state '${state}'; must be one of: claimed, doing`);
    this.name = "TaskSplitInvalidStateError";
    this.taskId = taskId;
    this.state = state;
  }
}

export class TaskSplitNotClaimantError extends Error {
  readonly taskId: TaskId;
  readonly assignee?: string;
  readonly attemptedBy: string;
  constructor(taskId: TaskId, attemptedBy: string, assignee?: string) {
    super(
      `Task ${taskId} is assigned to '${assignee ?? "<unassigned>"}', not '${attemptedBy}'`,
    );
    this.name = "TaskSplitNotClaimantError";
    this.taskId = taskId;
    this.attemptedBy = attemptedBy;
    if (assignee !== undefined) this.assignee = assignee;
  }
}

export class EmptyChildInputsError extends Error {
  constructor() {
    super("task split requires at least one non-empty title");
    this.name = "EmptyChildInputsError";
  }
}

export interface TaskSplitDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly missionStore?: MissionStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskSplitInput {
  readonly id: TaskId;
  readonly titles: readonly string[];
  readonly parallel?: boolean;
  readonly agentId?: string;
}

const SPLITTABLE_STATES: ReadonlySet<TaskState> = new Set(["claimed", "doing"]);

export async function taskSplit(
  deps: TaskSplitDeps,
  input: TaskSplitInput,
): Promise<readonly Task[]> {
  const parent = await deps.taskStore.get(input.id);
  if (!parent) throw new TaskNotFoundError(input.id);
  await assertMissionActive(deps.missionStore, parent.mission_id, "task:split");
  if (!SPLITTABLE_STATES.has(parent.state)) {
    throw new TaskSplitInvalidStateError(input.id, parent.state);
  }
  if (input.agentId !== undefined && parent.assignee !== input.agentId) {
    throw new TaskSplitNotClaimantError(input.id, input.agentId, parent.assignee);
  }
  if (input.titles.length === 0) throw new EmptyChildInputsError();
  if (input.titles.some((t) => t.trim().length === 0)) {
    throw new EmptyChildInputsError();
  }

  const idGen = deps.idFactory ?? generateTaskId;
  const childIds = input.titles.map(() => idGen());
  const childInputs = input.titles.map((title, i) => ({
    id: childIds[i] as TaskId,
    slug: `${parent.slug}-${i + 1}`,
    title: title.trim(),
    state: "draft" as TaskState,
    blocked_by:
      input.parallel || i === 0 ? [] : [childIds[i - 1] as TaskId],
    parent_id: parent.id,
    ...(parent.mission_id !== undefined ? { mission_id: parent.mission_id } : {}),
    ...(parent.spec_path !== undefined ? { spec_path: parent.spec_path } : {}),
    ...(parent.worktree_path !== undefined ? { worktree_path: parent.worktree_path } : {}),
  }));

  const parentPatch = {
    blocked_by: [...parent.blocked_by, ...childIds],
  };

  const { children } = await deps.taskStore.splitTask({
    parentId: parent.id,
    parentPatch,
    childInputs,
  });

  if (deps.missionStore && parent.mission_id) {
    await tryAdvanceMission(
      {
        missionStore: deps.missionStore,
        taskStore: deps.taskStore,
        evidenceStore: deps.evidenceStore,
        ...(deps.clock ? { clock: deps.clock } : {}),
        ...(deps.idFactory ? { idFactory: deps.idFactory } : {}),
      },
      { mission_id: parent.mission_id, trigger_task_verb: "task:split" },
    );
  }

  return children;
}
