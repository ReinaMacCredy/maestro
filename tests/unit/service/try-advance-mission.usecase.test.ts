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
import { tryAdvanceMission } from "@/service/try-advance-mission.usecase.js";
import { MissionTransitionError } from "@/types/mission-state.js";

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
  return { missionStore, plans, taskStore, tasks, evidenceStore, evidence };
}

async function seedPlanWithTasks(
  ps: MissionStorePort,
  ts: TaskStorePort,
  planState: MissionState,
  taskStates: readonly TaskState[],
): Promise<Mission> {
  const plan = await ps.create({ slug: "demo", title: "Demo", state: planState });
  for (let i = 0; i < taskStates.length; i += 1) {
    await ts.create({
      slug: `child-${i + 1}`,
      title: `Child ${i + 1}`,
      state: taskStates[i]!,
      mission_id: plan.id,
    });
  }
  return plan;
}

describe("tryAdvanceMission", () => {
  it("is a no-op when mission_id is undefined", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: undefined, trigger_task_verb: "task:claim" },
    );
    expect(out).toBeUndefined();
    expect(evidence.length).toBe(0);
  });

  it("is a no-op when the plan id does not exist", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: "pln-missing", trigger_task_verb: "task:claim" },
    );
    expect(out).toBeUndefined();
    expect(evidence.length).toBe(0);
  });

  it("advances planned -> in-progress on first task:claim and emits mission:auto-start", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "planned", [
      "claimed",
      "draft",
      "draft",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      kind: "transition",
      mission_id: plan.id,
      from_state: "planned",
      to_state: "in-progress",
      trigger_verb: "mission:auto-start",
    });
  });

  it("is idempotent: a second task:claim against an in-progress plan emits nothing", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "in-progress", [
      "claimed",
      "doing",
      "draft",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(0);
  });

  it("does not auto-complete on task:ship while a sibling is still non-terminal", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "in-progress", [
      "shipped",
      "doing",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:ship" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(0);
  });

  it("advances in-progress -> completed when every child is shipped", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "in-progress", [
      "shipped",
      "shipped",
      "shipped",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:ship" },
    );

    expect(out!.state).toBe("completed");
    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      kind: "transition",
      mission_id: plan.id,
      from_state: "in-progress",
      to_state: "completed",
      trigger_verb: "mission:auto-complete",
      trigger: "rollup",
      rule: "complete-or-fail",
    });
    expect((evidence[0] as { task_summary?: { total: number } }).task_summary).toMatchObject({
      total: 3,
    });
  });

  it("advances in-progress -> failed when at least one child is abandoned (terminal mixed)", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "in-progress", [
      "shipped",
      "shipped",
      "abandoned",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:ship" },
    );

    expect(out!.state).toBe("failed");
    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      from_state: "in-progress",
      to_state: "failed",
      rule: "complete-or-fail",
      trigger: "rollup",
    });
  });

  it("steps through in-progress when planned plan has all children terminal (abandoned-before-claim path)", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "planned", [
      "abandoned",
      "abandoned",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:abandon" },
    );

    expect(out!.state).toBe("failed");
    expect(evidence.length).toBe(2);
    expect(evidence[0]).toMatchObject({
      from_state: "planned",
      to_state: "in-progress",
      trigger_verb: "mission:auto-start",
    });
    expect(evidence[1]).toMatchObject({
      from_state: "in-progress",
      to_state: "failed",
      trigger_verb: "mission:auto-fail",
    });
  });

  it("auto-pauses in-progress when every active task is blocked", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "in-progress", [
      "blocked",
      "blocked",
      "shipped",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("paused");
    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      from_state: "in-progress",
      to_state: "paused",
      rule: "auto-pause",
      trigger: "rollup",
    });
  });

  it("auto-pauses when trigger is task:block (last active task is now blocked)", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "in-progress", [
      "blocked",
      "shipped",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:block" },
    );

    expect(out!.state).toBe("paused");
    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      from_state: "in-progress",
      to_state: "paused",
      rule: "auto-pause",
      trigger: "rollup",
    });
  });

  it("auto-resumes paused when at least one active task is unblocked", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "paused", [
      "blocked",
      "doing",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      from_state: "paused",
      to_state: "in-progress",
      rule: "auto-resume",
    });
  });

  it("never produces a planned -> paused transition", async () => {
    // Structural invariant from plan: TASK_TRANSITIONS forbids draft -> blocked,
    // so the only way into "blocked" is via "claimed" which fires planned ->
    // in-progress first. Combinations that include blocked + non-claimed tasks
    // can't legitimately occur, but the rollup must not synthesize paused from
    // planned even if presented with one.
    const { missionStore, taskStore, evidenceStore } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "planned", [
      "blocked",
      "draft",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).not.toBe("paused");
  });

  it("fixed-point loop handles single-tick multi-hop (planned -> in-progress -> completed)", async () => {
    // Edge case 2: a one-task mission whose only task is already shipped at
    // the time the rollup fires must reach the completed terminal state in
    // a single call, not stop at in-progress.
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "planned", ["shipped"]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:ship" },
    );

    expect(out!.state).toBe("completed");
    expect(evidence.length).toBe(2);
    expect(evidence.map((e) => (e as { to_state: string }).to_state)).toEqual([
      "in-progress",
      "completed",
    ]);
  });

  it("is a no-op for plans already at a terminal state", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "completed", [
      "shipped",
      "shipped",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:ship" },
    );

    expect(out!.state).toBe("completed");
    expect(evidence.length).toBe(0);
  });

  it("is idempotent when a concurrent invocation already advanced the mission to next.to", async () => {
    // Race window simulation: between our snapshot read (mission=planned) and
    // the in-loop advanceRollup write, another process moved the mission to
    // exactly the same target (in-progress). The freshness check must accept
    // it as success — no second write, no duplicate evidence row.
    const { missionStore, taskStore, evidenceStore, evidence, plans } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "planned", [
      "claimed",
      "draft",
    ]);
    let getCalls = 0;
    const racingStore = {
      ...missionStore,
      get: async (id: MissionId) => {
        getCalls += 1;
        // First call: tryAdvanceMission's snapshot read. Returns the planned
        // mission, so we enter the loop.
        // Second call onward: advanceRollup's freshness check sees the
        // concurrent writer's update.
        if (getCalls >= 2) {
          const existing = plans.get(id);
          return existing ? { ...existing, state: "in-progress" as const } : undefined;
        }
        return plans.get(id);
      },
    };

    const out = await tryAdvanceMission(
      { missionStore: racingStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("in-progress");
    // No fresh evidence — the concurrent writer already emitted theirs.
    expect(evidence.length).toBe(0);
  });

  it("throws MissionTransitionError if a concurrent writer moved the mission to an incompatible state", async () => {
    // Race window: we computed next.to = in-progress from a planned snapshot,
    // but a concurrent writer moved the mission to cancelled (terminal) before
    // we wrote. Writing in-progress would silently regress a legitimate cancel,
    // so the freshness check must throw instead of overwriting.
    const { missionStore, taskStore, evidenceStore, evidence, plans } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "planned", [
      "claimed",
      "draft",
    ]);
    let getCalls = 0;
    const racingStore = {
      ...missionStore,
      get: async (id: MissionId) => {
        getCalls += 1;
        if (getCalls >= 2) {
          const existing = plans.get(id);
          return existing ? { ...existing, state: "cancelled" as const } : undefined;
        }
        return plans.get(id);
      },
    };

    await expect(
      tryAdvanceMission(
        { missionStore: racingStore, taskStore, evidenceStore },
        { mission_id: plan.id, trigger_task_verb: "task:claim" },
      ),
    ).rejects.toBeInstanceOf(MissionTransitionError);
    expect(evidence.length).toBe(0);
  });

  it("is a no-op when called with task:claim against an in-progress plan (claim only matters for planned)", async () => {
    const { missionStore, taskStore, evidenceStore, evidence } = makeStores();
    const plan = await seedPlanWithTasks(missionStore, taskStore, "in-progress", [
      "claimed",
      "draft",
    ]);

    const out = await tryAdvanceMission(
      { missionStore, taskStore, evidenceStore },
      { mission_id: plan.id, trigger_task_verb: "task:claim" },
    );

    expect(out!.state).toBe("in-progress");
    expect(evidence.length).toBe(0);
  });

  it("throws MissionRollupCapExceededError when rollup never reaches steady state", async () => {
    // Simulate a rule-set bug: listByMissionId returns task snapshots that
    // alternate between "every active task blocked" and "some active task
    // unblocked" on each call. That would drive a pause/resume bounce past
    // the fixed-point cap. The cap is the safety net against silent drift.
    const { missionStore, plans, evidenceStore } = makeStores();
    const plan = await missionStore.create({
      slug: "thrash",
      title: "Thrash",
      state: "in-progress",
    });
    // Mock task store whose listByMissionId toggles every call. Only the
    // listByMissionId path matters for tryAdvanceMission's hot loop.
    let call = 0;
    const baseTask = {
      id: "tsk-a" as TaskId,
      slug: "a",
      title: "A",
      mission_id: plan.id,
      blocked_by: [] as string[],
      created_at: FROZEN.toISOString(),
      updated_at: FROZEN.toISOString(),
    };
    const ponyTaskStore: TaskStorePort = {
      async create() {
        throw new Error("unused");
      },
      async createMany() {
        throw new Error("unused");
      },
      async get() {
        return undefined;
      },
      async update() {
        throw new Error("unused");
      },
      async list() {
        return [];
      },
      async listByState() {
        return [];
      },
      async listByMissionId() {
        call += 1;
        const state: TaskState = call % 2 === 1 ? "blocked" : "doing";
        return [{ ...baseTask, state }];
      },
    };
    await expect(
      tryAdvanceMission(
        { missionStore, taskStore: ponyTaskStore, evidenceStore },
        { mission_id: plan.id, trigger_task_verb: "task:block" },
      ),
    ).rejects.toMatchObject({
      name: "MissionRollupCapExceededError",
      missionId: plan.id,
    });
    // The mission record was advanced through several states but never
    // reached the cap-clearing rule, so it sits at one of the bouncing states.
    const final = plans.get(plan.id);
    expect(["paused", "in-progress"]).toContain(final!.state);
  });
});
