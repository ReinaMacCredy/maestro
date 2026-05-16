import { describe, expect, it } from "bun:test";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
} from "@/v2/repo/evidence-store.port.js";
import type {
  ObservabilityEvent,
  ObservabilityPort,
} from "@/v2/repo/observability.port.js";
import {
  TaskNotFoundError,
  type CreateTaskInput,
  type TaskPatch,
  type TaskStorePort,
} from "@/v2/repo/task-store.port.js";
import { TaskTransitionError, type TaskState } from "@/v2/types/task-state.js";
import type { Task, TaskId } from "@/v2/types/task.js";
import { taskShip } from "@/v2/service/task-ship.usecase.js";

function makeEvidence(): { store: EvidenceStorePort; rows: EvidenceRow[] } {
  const rows: EvidenceRow[] = [];
  return {
    rows,
    store: {
      async append(row) {
        rows.push(row);
      },
      async list(_filter?: EvidenceFilter) {
        return rows;
      },
    },
  };
}

function makeTaskStore(seed: readonly Task[] = []): TaskStorePort {
  const tasks = new Map<TaskId, Task>(seed.map((t) => [t.id, t]));
  return {
    async create(input: CreateTaskInput) {
      const now = new Date().toISOString();
      const task: Task = {
        id: `tsk-${tasks.size + 1}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        blocked_by: input.blocked_by ?? [],
        created_at: now,
        updated_at: now,
      };
      tasks.set(task.id, task);
      return task;
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
    async listByPlanId(plan_id: string) {
      return [...tasks.values()].filter((t) => t.plan_id === plan_id);
    },
  };
}

function seedTask(state: TaskState, id = "tsk-target"): Task {
  const now = new Date().toISOString();
  return {
    id,
    slug: "demo",
    title: "demo",
    state,
    blocked_by: [],
    created_at: now,
    updated_at: now,
  };
}

describe("taskShip", () => {
  it("transitions ready -> shipped, records merged_at and pr_url, emits transition with verdict PASS", async () => {
    const taskStore = makeTaskStore([seedTask("ready")]);
    const { store: evidenceStore, rows } = makeEvidence();
    const FROZEN = new Date("2026-05-15T10:00:00.000Z");

    const result = await taskShip(
      { taskStore, evidenceStore, clock: () => FROZEN },
      { id: "tsk-target", pr_url: "https://example.test/pr/1" },
    );

    expect(result.state).toBe("shipped");
    expect(result.pr_url).toBe("https://example.test/pr/1");
    expect(result.merged_at).toBe(FROZEN.toISOString());
    expect(rows.length).toBe(1);
    expect(rows[0]).toMatchObject({
      kind: "transition",
      from_state: "ready",
      to_state: "shipped",
      trigger_verb: "task:ship",
      verdict: "PASS",
    });
  });

  it("allows shipping without a pr_url", async () => {
    const taskStore = makeTaskStore([seedTask("ready")]);
    const { store: evidenceStore } = makeEvidence();
    const result = await taskShip({ taskStore, evidenceStore }, { id: "tsk-target" });
    expect(result.state).toBe("shipped");
    expect(result.pr_url).toBeUndefined();
  });

  it("throws TaskNotFoundError for unknown id", async () => {
    const taskStore = makeTaskStore();
    const { store: evidenceStore } = makeEvidence();
    await expect(taskShip({ taskStore, evidenceStore }, { id: "tsk-missing" })).rejects.toBeInstanceOf(
      TaskNotFoundError,
    );
  });

  it("emits an observability event to the task-scoped runs stream when observabilityStore is supplied", async () => {
    const taskStore = makeTaskStore([seedTask("ready")]);
    const { store: evidenceStore } = makeEvidence();
    const events: ObservabilityEvent[] = [];
    const observabilityStore: ObservabilityPort = {
      async emit(event) {
        events.push(event);
      },
    };
    await taskShip(
      { taskStore, evidenceStore, observabilityStore },
      { id: "tsk-target" },
    );
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({
      task_id: "tsk-target",
      kind: "transition",
      payload: {
        from_state: "ready",
        to_state: "shipped",
        trigger_verb: "task:ship",
        verdict: "PASS",
      },
    });
  });

  it("throws TaskTransitionError when the source state is not ready", async () => {
    const taskStore = makeTaskStore([seedTask("verifying")]);
    const { store: evidenceStore } = makeEvidence();
    await expect(taskShip({ taskStore, evidenceStore }, { id: "tsk-target" })).rejects.toBeInstanceOf(
      TaskTransitionError,
    );
  });
});
