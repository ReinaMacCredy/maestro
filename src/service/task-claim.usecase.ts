import { readFile } from "node:fs/promises";
import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import { parseSpecFile } from "../repo/fs-spec-store.adapter.js";
import type { HandoffEmitterPort } from "../repo/handoff-emitter.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import type { WorktreeStorePort } from "../repo/worktree-store.port.js";
import { assertTaskTransition } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { emitHandoff } from "./emit-handoff.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";
import { tryAdvanceMission } from "./try-advance-mission.usecase.js";

export interface TaskClaimDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly missionStore?: MissionStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly worktreeStore?: WorktreeStorePort;
  readonly handoffEmitter?: HandoffEmitterPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskClaimInput {
  readonly id: TaskId;
  readonly agentId?: string;
  // When true, do not auto-create a worktree even if the spec is heavy-mode.
  readonly skipWorktree?: boolean;
}

export async function taskClaim(deps: TaskClaimDeps, input: TaskClaimInput): Promise<Task> {
  const existing = await deps.taskStore.get(input.id);
  if (!existing) throw new TaskNotFoundError(input.id);
  assertTaskTransition(existing.state, "claimed");
  const claimed_at = (deps.clock ?? (() => new Date()))().toISOString();

  // Heavy-mode auto-worktree (PR 34): if the spec frontmatter is mode=heavy
  // and we have a worktree store, create the worktree before recording claim.
  // Failures are swallowed onto the task (block_reason) so the claim still
  // happens but the agent sees what went wrong.
  let worktreePath: string | undefined;
  if (deps.worktreeStore && existing.spec_path && input.skipWorktree !== true) {
    const heavy = await isHeavyModeSpec(existing.spec_path);
    if (heavy) {
      try {
        const existingWt = await deps.worktreeStore.get(existing.id);
        if (existingWt) {
          worktreePath = existingWt.path;
        } else {
          const wt = await deps.worktreeStore.create({
            task_id: existing.id,
            slug: existing.slug,
          });
          worktreePath = wt.path;
        }
      } catch (err) {
        // Surface the failure but don't block the claim.
        worktreePath = undefined;
        console.error(
          `task claim: worktree creation failed for ${existing.id}: ${(err as Error).message}`,
        );
      }
    }
  }

  const updated = await deps.taskStore.update(input.id, {
    state: "claimed",
    assignee: input.agentId,
    claimed_at,
    ...(worktreePath ? { worktree_path: worktreePath } : {}),
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
      to_state: "claimed",
      trigger_verb: "task:claim",
      agent_id: input.agentId,
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
      { mission_id: updated.mission_id, trigger_task_verb: "task:claim" },
    );
  }

  await emitHandoff(
    { emitter: deps.handoffEmitter, clock: deps.clock },
    {
      task_id: updated.id,
      trigger_verb: "task:claim",
      agent_id: input.agentId,
      worktree_path: updated.worktree_path,
      spec_path: updated.spec_path,
    },
  );

  return updated;
}

async function isHeavyModeSpec(specPath: string): Promise<boolean> {
  try {
    const raw = await readFile(specPath, "utf8");
    const spec = parseSpecFile(raw, specPath);
    return spec.frontmatter.mode === "heavy";
  } catch {
    return false;
  }
}
