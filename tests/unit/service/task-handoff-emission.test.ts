import { describe, expect, it } from "bun:test";
import type { EvidenceRow, EvidenceStorePort } from "@/repo/evidence-store.port.js";
import type {
  HandoffEmitterPort,
  HandoffEnvelope,
  HandoffPickup,
} from "@/repo/handoff-emitter.port.js";
import {
  TaskNotFoundError,
  type CreateTaskInput,
  type TaskPatch,
  type TaskStorePort,
} from "@/repo/task-store.port.js";
import { taskBlock } from "@/service/task-block.usecase.js";
import { taskClaim } from "@/service/task-claim.usecase.js";
import type { TaskState } from "@/types/task-state.js";
import type { Task, TaskId } from "@/types/task.js";

function makeEvidence(): EvidenceStorePort {
  const rows: EvidenceRow[] = [];
  return {
    async append(row) {
      rows.push(row);
    },
    async list() {
      return rows;
    },
    async read(id) {
      return rows.find((r) => r.id === id);
    },
  };
}

function makeTaskStore(seed: readonly Task[]): TaskStorePort {
  const tasks = new Map<TaskId, Task>(seed.map((t) => [t.id, t]));
  return {
    async create(input: CreateTaskInput) {
      const now = new Date().toISOString();
      const t: Task = {
        id: `tsk-${tasks.size + 1}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        blocked_by: input.blocked_by ?? [],
        created_at: now,
        updated_at: now,
      };
      tasks.set(t.id, t);
      return t;
    },
    async get(id) {
      return tasks.get(id);
    },
    async update(id, patch: TaskPatch) {
      const existing = tasks.get(id);
      if (!existing) throw new TaskNotFoundError(id);
      const updated: Task = {
        ...existing,
        ...patch,
        id: existing.id,
        slug: existing.slug,
        created_at: existing.created_at,
        updated_at: new Date().toISOString(),
      };
      tasks.set(id, updated);
      return updated;
    },
    async list() {
      return [...tasks.values()];
    },
    async listByState(state: TaskState) {
      return [...tasks.values()].filter((t) => t.state === state);
    },
    async listByMissionId(mission_id: string) {
      return [...tasks.values()].filter((t) => t.mission_id === mission_id);
    },
  };
}

function makeHandoffEmitter(): {
  emitter: HandoffEmitterPort;
  emitted: HandoffEnvelope[];
} {
  const emitted: HandoffEnvelope[] = [];
  const pickups = new Map<string, HandoffPickup>();
  return {
    emitted,
    emitter: {
      async emit(env) {
        emitted.push(env);
      },
      async list() {
        return emitted;
      },
      async get(id) {
        return emitted.find((e) => e.id === id);
      },
      async markPickedUp(envelopeId, pickup) {
        if (pickups.has(envelopeId)) {
          throw new Error("EEXIST");
        }
        pickups.set(envelopeId, pickup);
      },
      async getPickup(envelopeId) {
        return pickups.get(envelopeId);
      },
    },
  };
}

function seedTask(state: TaskState, extra: Partial<Task> = {}): Task {
  const now = new Date().toISOString();
  return {
    id: "tsk-handoff",
    slug: "demo",
    title: "demo",
    state,
    blocked_by: [],
    created_at: now,
    updated_at: now,
    ...extra,
  };
}

describe("task lifecycle handoff emission (PR 35)", () => {
  it("taskClaim emits a task:claim handoff with agent_id when an emitter is wired", async () => {
    const taskStore = makeTaskStore([seedTask("draft")]);
    const evidenceStore = makeEvidence();
    const { emitter, emitted } = makeHandoffEmitter();
    await taskClaim(
      { taskStore, evidenceStore, handoffEmitter: emitter },
      { id: "tsk-handoff", agentId: "agent-a" },
    );
    expect(emitted.length).toBe(1);
    expect(emitted[0]!.trigger_verb).toBe("task:claim");
    expect(emitted[0]!.task_id).toBe("tsk-handoff");
    expect(emitted[0]!.agent_id).toBe("agent-a");
  });

  it("taskClaim handoff includes worktree_path when the claim created one", async () => {
    const taskStore = makeTaskStore([
      seedTask("draft", { worktree_path: undefined }),
    ]);
    // Pre-populate worktree_path via update bypassing the claim's spec read.
    await taskStore.update("tsk-handoff", { worktree_path: "/tmp/wt/x" });
    const evidenceStore = makeEvidence();
    const { emitter, emitted } = makeHandoffEmitter();
    await taskClaim(
      { taskStore, evidenceStore, handoffEmitter: emitter },
      { id: "tsk-handoff" },
    );
    expect(emitted[0]!.worktree_path).toBe("/tmp/wt/x");
  });

  it("taskBlock emits a task:block handoff carrying the reason", async () => {
    const taskStore = makeTaskStore([seedTask("doing")]);
    const evidenceStore = makeEvidence();
    const { emitter, emitted } = makeHandoffEmitter();
    await taskBlock(
      { taskStore, evidenceStore, handoffEmitter: emitter },
      { id: "tsk-handoff", reason: "missing-credentials" },
    );
    expect(emitted.length).toBe(1);
    expect(emitted[0]!.trigger_verb).toBe("task:block");
    expect(emitted[0]!.reason).toBe("missing-credentials");
  });

  it("is a no-op when no emitter is wired", async () => {
    const taskStore = makeTaskStore([seedTask("draft")]);
    const evidenceStore = makeEvidence();
    const claimed = await taskClaim(
      { taskStore, evidenceStore },
      { id: "tsk-handoff" },
    );
    expect(claimed.state).toBe("claimed");
  });
});
