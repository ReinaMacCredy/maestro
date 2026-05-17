import { describe, expect, it } from "bun:test";
import type {
  CreateMissionInput,
  MissionPatch,
  MissionStorePort,
} from "@/repo/mission-store.port.js";
import { MissionNotFoundError } from "@/repo/mission-store.port.js";
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
import { DuplicateSlugError } from "@/repo/task-store.port.js";
import type { Mission, MissionId } from "@/types/mission.js";
import type { MissionState } from "@/types/mission-state.js";
import { MissionTransitionError } from "@/types/mission-state.js";
import type { Task, TaskId } from "@/types/task.js";
import type { TaskState } from "@/types/task-state.js";
import {
  parseMissionDecomposeBatch,
  missionDecompose,
  MissionDecomposeBatchEmptyError,
  MissionDecomposeBatchInvalidError,
  MissionDecomposeDuplicateSlugInBatchError,
} from "@/service/mission-decompose.usecase.js";

const FROZEN = new Date("2026-05-15T11:00:00.000Z");

function makeStores(): {
  missionStore: MissionStorePort;
  plans: Map<MissionId, Mission>;
  taskStore: TaskStorePort;
  tasks: Map<TaskId, Task>;
  evidenceStore: EvidenceStorePort;
  evidence: EvidenceRow[];
} {
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
      if ([...tasks.values()].some((t) => t.slug === input.slug)) {
        throw new DuplicateSlugError(input.slug);
      }
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
  return { missionStore, plans, taskStore, tasks, evidenceStore, evidence };
}

async function seedSpecifiedPlan(missionStore: MissionStorePort): Promise<Mission> {
  return missionStore.create({
    slug: "demo-plan",
    title: "Demo plan",
    state: "approved",
  });
}

describe("parseMissionDecomposeBatch", () => {
  it("accepts a JSON array of task objects", () => {
    const out = parseMissionDecomposeBatch([
      { title: "A", slug: "a" },
      { title: "B", slug: "b", spec_path: "specs/b.md" },
    ]);
    expect(out.length).toBe(2);
    expect(out[0]).toEqual({ title: "A", slug: "a", spec_path: undefined });
    expect(out[1]).toEqual({ title: "B", slug: "b", spec_path: "specs/b.md" });
  });

  it("accepts a wrapper object with a 'tasks' array", () => {
    const out = parseMissionDecomposeBatch({ tasks: [{ title: "A", slug: "a" }] });
    expect(out.length).toBe(1);
  });

  it("rejects an empty batch", () => {
    expect(() => parseMissionDecomposeBatch([])).toThrow(MissionDecomposeBatchEmptyError);
  });

  it("rejects a non-array, non-object root", () => {
    expect(() => parseMissionDecomposeBatch("nope")).toThrow(MissionDecomposeBatchInvalidError);
  });

  it("rejects a task missing a title", () => {
    expect(() => parseMissionDecomposeBatch([{ slug: "a" }])).toThrow(MissionDecomposeBatchInvalidError);
  });

  it("rejects a task missing a slug", () => {
    expect(() => parseMissionDecomposeBatch([{ title: "A" }])).toThrow(
      MissionDecomposeBatchInvalidError,
    );
  });

  it("rejects spec_path that is not a non-empty string", () => {
    expect(() => parseMissionDecomposeBatch([{ title: "A", slug: "a", spec_path: 42 }])).toThrow(
      MissionDecomposeBatchInvalidError,
    );
  });
});

describe("missionDecompose intake-state input (relaxed source)", () => {
  it("accepts intake as input state and advances intake -> planned", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const mission = await missionStore.create({
      slug: "bare-mission",
      title: "Bare",
      state: "intake",
    });
    const result = await missionDecompose(
      { missionStore, taskStore, evidenceStore },
      {
        mission_id: mission.id,
        tasks: [{ title: "A", slug: "a" }],
      },
    );
    expect(result.mission.state).toBe("planned");
  });
});

describe("missionDecompose rejects missions with existing tasks", () => {
  it("errors when mission already owns tasks", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const mission = await missionStore.create({
      slug: "with-tasks",
      title: "With tasks",
      state: "intake",
    });
    await taskStore.create({
      slug: "pre-existing",
      title: "Pre-existing",
      state: "draft",
      mission_id: mission.id,
    });
    await expect(
      missionDecompose(
        { missionStore, taskStore, evidenceStore },
        {
          mission_id: mission.id,
          tasks: [{ title: "New", slug: "new" }],
        },
      ),
    ).rejects.toThrow(/already has 1 task/);
  });
});

describe("missionDecompose", () => {
  it("creates child tasks linked by mission_id and advances the plan to 'planned'", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedSpecifiedPlan(missionStore);

    const result = await missionDecompose(
      { missionStore, taskStore, evidenceStore },
      {
        mission_id: plan.id,
        tasks: [
          { title: "First", slug: "first" },
          { title: "Second", slug: "second", spec_path: "specs/second.md" },
          { title: "Third", slug: "third" },
        ],
      },
    );

    expect(result.mission.id).toBe(plan.id);
    expect(result.mission.state).toBe("planned");
    expect(result.tasks.length).toBe(3);
    for (const t of result.tasks) {
      expect(t.state).toBe("draft");
      expect(t.mission_id).toBe(plan.id);
    }
    expect(result.tasks[1]!.spec_path).toBe("specs/second.md");

    const transitions = evidence.filter((e) => e.kind === "transition");
    expect(transitions.length).toBe(4);
    const taskTransitions = transitions.filter((e) => e.task_id !== undefined);
    const planOnlyTransitions = transitions.filter((e) => e.mission_id !== undefined && e.task_id === undefined);
    expect(taskTransitions.length).toBe(3);
    for (const ev of taskTransitions) {
      expect(ev).toMatchObject({
        kind: "transition",
        mission_id: plan.id,
        from_state: null,
        to_state: "draft",
        trigger_verb: "task:from-spec",
      });
    }
    expect(planOnlyTransitions.length).toBe(1);
    expect(planOnlyTransitions[0]).toMatchObject({
      kind: "transition",
      mission_id: plan.id,
      from_state: "approved",
      to_state: "planned",
      trigger_verb: "mission:decompose",
    });
  });

  it("throws MissionNotFoundError when the plan id does not exist", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    await expect(
      missionDecompose(
        { missionStore, taskStore, evidenceStore },
        { mission_id: "pln-missing", tasks: [{ title: "A", slug: "a" }] },
      ),
    ).rejects.toBeInstanceOf(MissionNotFoundError);
    expect(evidence.length).toBe(0);
  });

  it("rejects when the plan is not in 'approved' (e.g. already planned)", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const plan = await seedSpecifiedPlan(missionStore);
    await missionStore.update(plan.id, { state: "planned" });

    await expect(
      missionDecompose(
        { missionStore, taskStore, evidenceStore },
        { mission_id: plan.id, tasks: [{ title: "A", slug: "a" }] },
      ),
    ).rejects.toBeInstanceOf(MissionTransitionError);
  });

  it("rejects an empty batch", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const plan = await seedSpecifiedPlan(missionStore);

    await expect(
      missionDecompose(
        { missionStore, taskStore, evidenceStore },
        { mission_id: plan.id, tasks: [] },
      ),
    ).rejects.toBeInstanceOf(MissionDecomposeBatchEmptyError);
  });

  it("rejects duplicate slugs within the batch", async () => {
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const plan = await seedSpecifiedPlan(missionStore);

    await expect(
      missionDecompose(
        { missionStore, taskStore, evidenceStore },
        {
          mission_id: plan.id,
          tasks: [
            { title: "A", slug: "dup" },
            { title: "B", slug: "dup" },
          ],
        },
      ),
    ).rejects.toBeInstanceOf(MissionDecomposeDuplicateSlugInBatchError);
  });

  it("rejects when a batch slug collides with an existing task slug", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedSpecifiedPlan(missionStore);
    await taskStore.create({ slug: "taken", title: "Existing", state: "draft" });

    await expect(
      missionDecompose(
        { missionStore, taskStore, evidenceStore },
        { mission_id: plan.id, tasks: [{ title: "A", slug: "taken" }] },
      ),
    ).rejects.toBeInstanceOf(DuplicateSlugError);
    // pre-validation should have prevented any task or plan transition writes
    expect(evidence.length).toBe(0);
    expect((await missionStore.get(plan.id))!.state).toBe("approved");
  });
});
