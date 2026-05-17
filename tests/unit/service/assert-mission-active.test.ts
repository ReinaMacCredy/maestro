import { describe, expect, it } from "bun:test";
import type {
  CreateMissionInput,
  MissionPatch,
  MissionStorePort,
} from "@/repo/mission-store.port.js";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
} from "@/repo/evidence-store.port.js";
import type {
  CreateTaskInput,
  TaskPatch,
  TaskStorePort,
} from "@/repo/task-store.port.js";
import type { Mission, MissionId } from "@/types/mission.js";
import type { MissionState } from "@/types/mission-state.js";
import type { Task, TaskId } from "@/types/task.js";
import type { TaskState } from "@/types/task-state.js";
import {
  assertMissionActive,
  MissionTerminalGuardError,
} from "@/service/assert-mission-active.js";
import { taskClaim } from "@/service/task-claim.usecase.js";
import { taskShip } from "@/service/task-ship.usecase.js";

const FROZEN = new Date("2026-05-15T11:00:00.000Z");

function makeStores() {
  const plans = new Map<MissionId, Mission>();
  const tasks = new Map<TaskId, Task>();
  const evidence: EvidenceRow[] = [];
  let planN = 0;
  let taskN = 0;
  const missionStore: MissionStorePort = {
    async create(input: CreateMissionInput) {
      planN += 1;
      const plan: Mission = {
        id: `pln-${planN}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      };
      plans.set(plan.id, plan);
      return plan;
    },
    async get(id) {
      return plans.get(id);
    },
    async update(id, patch: MissionPatch) {
      const existing = plans.get(id);
      if (!existing) throw new Error("not found");
      const next: Mission = { ...existing, ...patch, updated_at: FROZEN.toISOString() };
      plans.set(id, next);
      return next;
    },
    async list() {
      return [...plans.values()];
    },
    async listByState(state: MissionState) {
      return [...plans.values()].filter((p) => p.state === state);
    },
  };
  const taskStore: TaskStorePort = {
    async create(input: CreateTaskInput) {
      taskN += 1;
      const task: Task = {
        id: `tsk-${taskN}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        mission_id: input.mission_id,
        blocked_by: input.blocked_by ?? [],
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      };
      tasks.set(task.id, task);
      return task;
    },
    async createMany(inputs: readonly CreateTaskInput[]) {
      const out: Task[] = [];
      for (const i of inputs) out.push(await this.create(i));
      return out;
    },
    async get(id) {
      return tasks.get(id);
    },
    async update(id, patch: TaskPatch) {
      const existing = tasks.get(id);
      if (!existing) throw new Error("not found");
      const next: Task = { ...existing, ...patch, updated_at: FROZEN.toISOString() };
      tasks.set(id, next);
      return next;
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
  const evidenceStore: EvidenceStorePort = {
    async append(row) {
      evidence.push(row);
    },
    async list(_filter?: EvidenceFilter) {
      return evidence;
    },
    async read(id) {
      return evidence.find((r) => r.id === id);
    },
  };
  return { missionStore, taskStore, evidenceStore, plans, tasks, evidence };
}

describe("assertMissionActive", () => {
  it("is a no-op when the mission store is undefined", async () => {
    await assertMissionActive(undefined, "pln-x", "task:claim");
  });

  it("is a no-op when missionId is undefined", async () => {
    const { missionStore } = makeStores();
    await assertMissionActive(missionStore, undefined, "task:claim");
  });

  it("is a no-op when the mission_id points at a missing mission (lenient on orphans)", async () => {
    const { missionStore } = makeStores();
    await assertMissionActive(missionStore, "pln-missing", "task:claim");
  });

  it("throws MissionTerminalGuardError when the parent mission is cancelled", async () => {
    const { missionStore } = makeStores();
    const m = await missionStore.create({ slug: "demo", title: "Demo", state: "cancelled" });
    await expect(assertMissionActive(missionStore, m.id, "task:claim")).rejects.toBeInstanceOf(
      MissionTerminalGuardError,
    );
  });

  it("throws for completed and failed missions too (all terminal states are guarded)", async () => {
    const { missionStore } = makeStores();
    const c = await missionStore.create({ slug: "c", title: "C", state: "completed" });
    const f = await missionStore.create({ slug: "f", title: "F", state: "failed" });
    await expect(assertMissionActive(missionStore, c.id, "task:ship")).rejects.toBeInstanceOf(
      MissionTerminalGuardError,
    );
    await expect(assertMissionActive(missionStore, f.id, "task:ship")).rejects.toBeInstanceOf(
      MissionTerminalGuardError,
    );
  });

  it("passes for active missions", async () => {
    const { missionStore } = makeStores();
    const m = await missionStore.create({ slug: "demo", title: "Demo", state: "in-progress" });
    await assertMissionActive(missionStore, m.id, "task:claim");
  });
});

describe("task verbs refuse to operate under a terminal mission", () => {
  it("taskClaim throws MissionTerminalGuardError when parent mission is cancelled", async () => {
    const { missionStore, taskStore, evidenceStore, plans } = makeStores();
    const mission = await missionStore.create({
      slug: "demo",
      title: "Demo",
      state: "in-progress",
    });
    const task = await taskStore.create({
      slug: "orphan",
      title: "Orphan",
      state: "draft",
      mission_id: mission.id,
    });
    // Mission gets cancelled out-of-band (e.g. cascade abandon failed for this
    // task during `mission cancel`). The orphan task must not be claimable.
    plans.set(mission.id, { ...plans.get(mission.id)!, state: "cancelled" });

    await expect(
      taskClaim({ taskStore, missionStore, evidenceStore }, { id: task.id }),
    ).rejects.toBeInstanceOf(MissionTerminalGuardError);

    // Task state untouched: guard short-circuits before assertTaskTransition.
    const after = await taskStore.get(task.id);
    expect(after!.state).toBe("draft");
  });

  it("taskShip throws MissionTerminalGuardError when parent mission is completed", async () => {
    const { missionStore, taskStore, evidenceStore, plans } = makeStores();
    const mission = await missionStore.create({
      slug: "demo",
      title: "Demo",
      state: "in-progress",
    });
    const task = await taskStore.create({
      slug: "child",
      title: "Child",
      state: "claimed",
      mission_id: mission.id,
    });
    plans.set(mission.id, { ...plans.get(mission.id)!, state: "completed" });

    await expect(
      taskShip({ taskStore, missionStore, evidenceStore }, { id: task.id }),
    ).rejects.toBeInstanceOf(MissionTerminalGuardError);

    const after = await taskStore.get(task.id);
    expect(after!.state).toBe("claimed");
  });

  it("taskClaim still works when no missionStore is wired (back-compat surface)", async () => {
    // taskClaim's missionStore is optional; the guard must no-op if a caller
    // (older test harness, alt composition) didn't wire it. Otherwise we'd
    // regress every taskClaim call site that doesn't pass missionStore.
    const { taskStore, evidenceStore } = makeStores();
    const task = await taskStore.create({
      slug: "lone",
      title: "Lone",
      state: "draft",
    });
    const out = await taskClaim({ taskStore, evidenceStore }, { id: task.id });
    expect(out.state).toBe("claimed");
  });
});
