import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { HandoffEmitterPort } from "../repo/handoff-emitter.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { assertTaskTransition } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { emitHandoff } from "./emit-handoff.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";
import { tryAdvanceMission } from "./try-advance-mission.usecase.js";

export interface TaskBlockDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly missionStore?: MissionStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly handoffEmitter?: HandoffEmitterPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskBlockInput {
  readonly id: TaskId;
  readonly reason: string;
  /** Caller's own tool name (e.g. 'codex', 'claude-code'); when set, propagates to the auto-emitted handoff envelope as to_agent. */
  readonly tool?: string;
}

export async function taskBlock(deps: TaskBlockDeps, input: TaskBlockInput): Promise<Task> {
  const existing = await deps.taskStore.get(input.id);
  if (!existing) throw new TaskNotFoundError(input.id);
  assertTaskTransition(existing.state, "blocked");
  const updated = await deps.taskStore.update(input.id, {
    state: "blocked",
    block_reason: input.reason,
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
      to_state: "blocked",
      trigger_verb: "task:block",
      reason: input.reason,
    },
  );
  await emitHandoff(
    { emitter: deps.handoffEmitter, clock: deps.clock },
    {
      task_id: updated.id,
      trigger_verb: "task:block",
      reason: input.reason,
      worktree_path: updated.worktree_path,
      spec_path: updated.spec_path,
      to_agent: input.tool,
    },
  );
  if (deps.missionStore) {
    await tryAdvanceMission(
      {
        missionStore: deps.missionStore,
        taskStore: deps.taskStore,
        evidenceStore: deps.evidenceStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      { mission_id: updated.mission_id, trigger_task_verb: "task:block" },
    );
  }
  return updated;
}
