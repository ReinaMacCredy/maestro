import { describe, expect, it } from "bun:test";
import {
  missionCancel,
  MissionCancelTerminalError,
} from "@/service/mission-cancel.usecase.js";
import { MissionNotFoundError } from "@/repo/mission-store.port.js";
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

const FROZEN = new Date("2026-05-15T11:00:00.000Z");

function makeStores(): {
  missionStore: MissionStorePort;
  taskStore: TaskStorePort;
  evidenceStore: EvidenceStorePort;
  missions: Map<MissionId, Mission>;
  tasks: Map<TaskId, Task>;
  evidence: EvidenceRow[];
} {
  const missions = new Map<MissionId, Mission>();
  const tasks = new Map<TaskId, Task>();
  const evidence: EvidenceRow[] = [];
  let mN = 0;
  let tN = 0;
  const missionStore: MissionStorePort = {
    async create(input: CreateMissionInput) {
      mN += 1;
      const m: Mission = {
        id: `pln-${mN}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      };
      missions.set(m.id, m);
      return m;
    },
    async get(id) {
      return missions.get(id);
    },
    async update(id, patch: MissionPatch) {
      const existing = missions.get(id);
      if (!existing) throw new Error("not found");
      const next: Mission = { ...existing, ...patch, updated_at: FROZEN.toISOString() };
      missions.set(id, next);
      return next;
    },
    async list() {
      return [...missions.values()];
    },
    async listByState(state: MissionState) {
      return [...missions.values()].filter((m) => m.state === state);
    },
  };
  const taskStore: TaskStorePort = {
    async create(input: CreateTaskInput) {
      tN += 1;
      const t: Task = {
        id: `tsk-${tN}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        mission_id: input.mission_id,
        blocked_by: input.blocked_by ?? [],
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      };
      tasks.set(t.id, t);
      return t;
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
  return { missionStore, taskStore, evidenceStore, missions, tasks, evidence };
}

async function seed(
  ms: MissionStorePort,
  ts: TaskStorePort,
  state: MissionState,
  taskStates: readonly TaskState[],
): Promise<Mission> {
  const m = await ms.create({ slug: "demo", title: "Demo", state });
  for (let i = 0; i < taskStates.length; i += 1) {
    await ts.create({
      slug: `child-${i + 1}`,
      title: `Child ${i + 1}`,
      state: taskStates[i]!,
      mission_id: m.id,
    });
  }
  return m;
}

describe("missionCancel", () => {
  it("throws when mission id does not exist", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    await expect(
      missionCancel(
        { missionStore, taskStore, evidenceStore },
        { mission_id: "pln-missing" },
      ),
    ).rejects.toBeInstanceOf(MissionNotFoundError);
  });

  it("is idempotent on an already-cancelled mission", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const m = await seed(missionStore, taskStore, "cancelled", []);
    const result = await missionCancel(
      { missionStore, taskStore, evidenceStore },
      { mission_id: m.id },
    );
    expect(result.alreadyCancelled).toBe(true);
    expect(result.cancelledTaskIds).toEqual([]);
    expect(evidence.length).toBe(0);
  });

  it("errors on a completed mission (different outcome shouldn't be re-stamped)", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const m = await seed(missionStore, taskStore, "completed", ["shipped"]);
    await expect(
      missionCancel({ missionStore, taskStore, evidenceStore }, { mission_id: m.id }),
    ).rejects.toBeInstanceOf(MissionCancelTerminalError);
  });

  it("errors on a failed mission", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const m = await seed(missionStore, taskStore, "failed", ["abandoned"]);
    await expect(
      missionCancel({ missionStore, taskStore, evidenceStore }, { mission_id: m.id }),
    ).rejects.toBeInstanceOf(MissionCancelTerminalError);
  });

  it("cascades to active tasks and transitions mission to cancelled", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const m = await seed(missionStore, taskStore, "in-progress", [
      "claimed",
      "doing",
      "blocked",
    ]);
    const result = await missionCancel(
      { missionStore, taskStore, evidenceStore },
      { mission_id: m.id, reason: "out of scope" },
    );
    expect(result.alreadyCancelled).toBe(false);
    expect(result.mission.state).toBe("cancelled");
    expect(result.cancelledTaskIds.length).toBe(3);
    expect(result.cascadeErrors).toEqual([]);

    const taskEvidence = evidence.filter((e) => "task_id" in e && e.task_id !== undefined);
    expect(taskEvidence.length).toBe(3);
    for (const row of taskEvidence) {
      expect(row).toMatchObject({
        to_state: "abandoned",
        trigger_verb: "mission:cancel",
        reason: "out of scope",
      });
    }

    const cascadedTasks = await taskStore.listByMissionId(m.id);
    for (const t of cascadedTasks) {
      expect(t.abandon_reason).toBe("out of scope");
    }

    const missionEvidence = evidence.find(
      (e) => "mission_id" in e && e.mission_id === m.id && "to_state" in e && e.to_state === "cancelled",
    );
    expect(missionEvidence).toBeDefined();
    expect(missionEvidence).toMatchObject({
      trigger: "verb",
      cancelled_by: "user",
      reason: "out of scope",
    });
  });

  it("falls back to abandon_reason='mission cancelled' when no reason is given", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const m = await seed(missionStore, taskStore, "in-progress", ["claimed"]);
    await missionCancel(
      { missionStore, taskStore, evidenceStore },
      { mission_id: m.id },
    );
    const cascadedTasks = await taskStore.listByMissionId(m.id);
    expect(cascadedTasks[0]?.abandon_reason).toBe("mission cancelled");
  });

  it("skips already-terminal tasks during cascade", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const m = await seed(missionStore, taskStore, "in-progress", [
      "shipped",
      "abandoned",
      "doing",
      "claimed",
    ]);
    const result = await missionCancel(
      { missionStore, taskStore, evidenceStore },
      { mission_id: m.id },
    );
    expect(result.cancelledTaskIds.length).toBe(2);

    const taskEvidence = evidence.filter((e) => "task_id" in e && e.task_id !== undefined);
    expect(taskEvidence.length).toBe(2);
  });

  it("records cascade errors but still cancels the mission (best-effort semantics)", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const m = await seed(missionStore, taskStore, "in-progress", ["doing"]);
    const taskList = await taskStore.listByMissionId(m.id);
    const taskId = taskList[0]!.id;
    // Replace update on the task store with one that throws for this specific id.
    const originalUpdate = taskStore.update.bind(taskStore);
    taskStore.update = async (id, patch) => {
      if (id === taskId && (patch as { state?: string }).state === "abandoned") {
        throw new Error("simulated storage error");
      }
      return originalUpdate(id, patch);
    };

    const result = await missionCancel(
      { missionStore, taskStore, evidenceStore },
      { mission_id: m.id },
    );

    expect(result.mission.state).toBe("cancelled");
    expect(result.cancelledTaskIds).toEqual([]);
    expect(result.cascadeErrors.length).toBe(1);
    expect(result.cascadeErrors[0]).toMatchObject({ taskId, message: "simulated storage error" });
  });

  it("can cancel a planned mission (no active tasks yet)", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const m = await seed(missionStore, taskStore, "planned", ["draft", "draft"]);
    const result = await missionCancel(
      { missionStore, taskStore, evidenceStore },
      { mission_id: m.id },
    );
    expect(result.mission.state).toBe("cancelled");
    // Drafts are non-terminal and get abandoned in the cascade.
    expect(result.cancelledTaskIds.length).toBe(2);
  });

  it("can cancel an intake mission with zero tasks", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const m = await seed(missionStore, taskStore, "intake", []);
    const result = await missionCancel(
      { missionStore, taskStore, evidenceStore },
      { mission_id: m.id },
    );
    expect(result.mission.state).toBe("cancelled");
    expect(result.cancelledTaskIds).toEqual([]);
  });
});
