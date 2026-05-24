import { describe, expect, it } from "bun:test";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
} from "@/repo/evidence-store.port.js";
import type {
  CreateMissionInput,
  MissionPatch,
  MissionStorePort,
} from "@/repo/mission-store.port.js";
import {
  DuplicateSlugError,
  DuplicateTaskIdError,
  InvalidTaskIdError,
  TaskNotFoundError,
  type CreateTaskInput,
  type TaskPatch,
  type TaskStorePort,
} from "@/repo/task-store.port.js";
import { MissionTerminalGuardError } from "@/service/assert-mission-active.js";
import {
  EmptyChildInputsError,
  taskSplit,
  TaskSplitInvalidStateError,
  TaskSplitNotClaimantError,
} from "@/service/task-split.usecase.js";
import type { Mission, MissionId } from "@/types/mission.js";
import type { MissionState } from "@/types/mission-state.js";
import { TASK_ID_PATTERN, type Task, type TaskId } from "@/types/task.js";
import type { TaskState } from "@/types/task-state.js";

const FROZEN = new Date("2026-05-15T11:00:00.000Z");

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
      async read(id) {
        return rows.find((r) => r.id === id);
      },
    },
  };
}

// Map-backed fake that mirrors the JsonlTaskStore.splitTask semantics: parent
// patch + child create commit together, with dup-slug detection across batch
// and existing rows. Just enough to exercise the usecase end-to-end without
// touching the filesystem.
function makeTaskStore(seed: readonly Task[]): {
  store: TaskStorePort;
  tasks: Map<TaskId, Task>;
} {
  const tasks = new Map<TaskId, Task>(seed.map((t) => [t.id, t]));
  let autoIdN = 0;
  const nextId = (): TaskId => {
    autoIdN += 1;
    return `tsk-auto-${autoIdN}` as TaskId;
  };
  const store: TaskStorePort = {
    async create(input: CreateTaskInput) {
      const t: Task = {
        id: input.id ?? nextId(),
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        mission_id: input.mission_id,
        blocked_by: input.blocked_by ?? [],
        ...(input.parent_id !== undefined ? { parent_id: input.parent_id } : {}),
        ...(input.worktree_path !== undefined ? { worktree_path: input.worktree_path } : {}),
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      };
      tasks.set(t.id, t);
      return t;
    },
    async createMany(inputs: readonly CreateTaskInput[]) {
      const out: Task[] = [];
      for (const i of inputs) out.push(await this.create(i));
      return out;
    },
    async splitTask(input) {
      if (input.childInputs.length === 0) {
        throw new Error("splitTask requires at least one child");
      }
      const parent = tasks.get(input.parentId);
      if (!parent) throw new TaskNotFoundError(input.parentId);
      const inBatchSlugs = new Set<string>();
      const inBatchIds = new Set<TaskId>();
      const existingSlugs = new Set([...tasks.values()].map((t) => t.slug));
      const existingIds = new Set([...tasks.values()].map((t) => t.id));
      for (const ci of input.childInputs) {
        if (ci.id !== undefined) {
          if (!TASK_ID_PATTERN.test(ci.id)) {
            throw new InvalidTaskIdError(ci.id);
          }
          if (existingIds.has(ci.id) || inBatchIds.has(ci.id)) {
            throw new DuplicateTaskIdError(ci.id);
          }
          inBatchIds.add(ci.id);
        }
        if (inBatchSlugs.has(ci.slug) || existingSlugs.has(ci.slug)) {
          throw new DuplicateSlugError(ci.slug);
        }
        inBatchSlugs.add(ci.slug);
      }
      const updatedParent: Task = {
        ...parent,
        ...input.parentPatch,
        updated_at: FROZEN.toISOString(),
      };
      tasks.set(parent.id, updatedParent);
      const children: Task[] = input.childInputs.map((ci) => ({
        id: ci.id ?? nextId(),
        slug: ci.slug,
        title: ci.title,
        state: ci.state,
        ...(ci.spec_path !== undefined ? { spec_path: ci.spec_path } : {}),
        ...(ci.mission_id !== undefined ? { mission_id: ci.mission_id } : {}),
        ...(ci.worktree_path !== undefined ? { worktree_path: ci.worktree_path } : {}),
        blocked_by: ci.blocked_by ?? [],
        parent_id: input.parentId,
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      }));
      for (const c of children) tasks.set(c.id, c);
      return { parent: updatedParent, children };
    },
    async get(id) {
      return tasks.get(id);
    },
    async update(id, patch: TaskPatch) {
      const existing = tasks.get(id);
      if (!existing) throw new TaskNotFoundError(id);
      const next: Task = {
        ...existing,
        ...patch,
        id: existing.id,
        slug: existing.slug,
        created_at: existing.created_at,
        updated_at: FROZEN.toISOString(),
      };
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
  return { store, tasks };
}

interface MissionStoreFake {
  store: MissionStorePort;
  missions: Map<MissionId, Mission>;
  getCalls: MissionId[];
}

function makeMissionStore(seed: readonly Mission[] = []): MissionStoreFake {
  const missions = new Map<MissionId, Mission>(seed.map((m) => [m.id, m]));
  const getCalls: MissionId[] = [];
  const store: MissionStorePort = {
    async create(input: CreateMissionInput) {
      const m: Mission = {
        id: `pln-${missions.size + 1}`,
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
      getCalls.push(id);
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
  return { store, missions, getCalls };
}

function seedParent(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-parent" as TaskId,
    slug: "parent",
    title: "Parent task",
    state: "claimed",
    blocked_by: [],
    created_at: FROZEN.toISOString(),
    updated_at: FROZEN.toISOString(),
    ...overrides,
  };
}

describe("taskSplit", () => {
  it("happy path: sequential 3-child split chains blocked_by and appends to parent", async () => {
    const parent = seedParent();
    const { store: taskStore, tasks } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    let n = 0;
    const idFactory = () => `tsk-c-${++n}`;
    const children = await taskSplit(
      { taskStore, evidenceStore, idFactory },
      { id: parent.id, titles: ["a", "b", "c"] },
    );
    expect(children.length).toBe(3);
    expect(children[0]!.blocked_by).toEqual([]);
    expect(children[1]!.blocked_by).toEqual([children[0]!.id]);
    expect(children[2]!.blocked_by).toEqual([children[1]!.id]);
    const updatedParent = tasks.get(parent.id)!;
    expect(updatedParent.blocked_by).toEqual([
      children[0]!.id,
      children[1]!.id,
      children[2]!.id,
    ]);
  });

  it("parallel=true sets all children blocked_by to empty arrays", async () => {
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    const children = await taskSplit(
      { taskStore, evidenceStore },
      { id: parent.id, titles: ["a", "b", "c"], parallel: true },
    );
    for (const c of children) expect(c.blocked_by).toEqual([]);
  });

  const NON_SPLITTABLE: readonly TaskState[] = [
    "draft",
    "verifying",
    "blocked",
    "ready",
    "shipped",
    "abandoned",
  ];
  for (const state of NON_SPLITTABLE) {
    it(`throws TaskSplitInvalidStateError when parent state is ${state}`, async () => {
      const parent = seedParent({ state });
      const { store: taskStore } = makeTaskStore([parent]);
      const { store: evidenceStore } = makeEvidence();
      await expect(
        taskSplit({ taskStore, evidenceStore }, { id: parent.id, titles: ["a"] }),
      ).rejects.toBeInstanceOf(TaskSplitInvalidStateError);
    });
  }

  it("throws TaskSplitNotClaimantError when agentId does not match assignee", async () => {
    const parent = seedParent({ assignee: "alice" });
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    await expect(
      taskSplit(
        { taskStore, evidenceStore },
        { id: parent.id, titles: ["a"], agentId: "bob" },
      ),
    ).rejects.toBeInstanceOf(TaskSplitNotClaimantError);
  });

  it("children inherit mission_id, spec_path, worktree_path; never inherit assignee/slug/id/blocked_by", async () => {
    const parent = seedParent({
      mission_id: "pln-1",
      spec_path: ".maestro/specs/parent.md",
      worktree_path: "/tmp/wt/parent",
      assignee: "alice",
      blocked_by: ["tsk-other"],
    });
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    let n = 0;
    const idFactory = () => `tsk-c-${++n}`;
    const children = await taskSplit(
      { taskStore, evidenceStore, idFactory },
      { id: parent.id, titles: ["one", "two"] },
    );
    for (const c of children) {
      expect(c.mission_id).toBe("pln-1");
      expect(c.spec_path).toBe(".maestro/specs/parent.md");
      expect(c.worktree_path).toBe("/tmp/wt/parent");
      expect(c.assignee).toBeUndefined();
      expect(c.slug).not.toBe(parent.slug);
      expect(c.id).not.toBe(parent.id);
    }
    // child[0] is the first link so blocked_by must be empty (i.e. NOT
    // inherited from parent.blocked_by which had ["tsk-other"]).
    expect(children[0]!.blocked_by).toEqual([]);
    // child[1] depends only on child[0], not on parent's blocked_by.
    expect(children[1]!.blocked_by).toEqual([children[0]!.id]);
  });

  it("titles=[] throws EmptyChildInputsError", async () => {
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    await expect(
      taskSplit({ taskStore, evidenceStore }, { id: parent.id, titles: [] }),
    ).rejects.toBeInstanceOf(EmptyChildInputsError);
  });

  it("titles with a whitespace-only entry throws EmptyChildInputsError", async () => {
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    await expect(
      taskSplit(
        { taskStore, evidenceStore },
        { id: parent.id, titles: ["valid", "   ", "valid2"] },
      ),
    ).rejects.toBeInstanceOf(EmptyChildInputsError);
  });

  it("custom idFactory is honored", async () => {
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    let n = 0;
    const idFactory = () => `tsk-counted-${++n}`;
    const children = await taskSplit(
      { taskStore, evidenceStore, idFactory },
      { id: parent.id, titles: ["a", "b"] },
    );
    expect(children[0]!.id).toBe("tsk-counted-1");
    expect(children[1]!.id).toBe("tsk-counted-2");
  });

  it("child ids are unique within the batch", async () => {
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    const children = await taskSplit(
      { taskStore, evidenceStore },
      { id: parent.id, titles: ["a", "b", "c", "d"] },
    );
    const ids = new Set(children.map((c) => c.id));
    expect(ids.size).toBe(children.length);
  });

  it("every child carries parent_id pointing at the parent", async () => {
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    const children = await taskSplit(
      { taskStore, evidenceStore },
      { id: parent.id, titles: ["a", "b", "c"] },
    );
    for (const c of children) expect(c.parent_id).toBe(parent.id);
  });

  it("calls tryAdvanceMission with trigger_task_verb='task:split' when parent.mission_id is set", async () => {
    const parent = seedParent({ mission_id: "pln-1" });
    const seedMission: Mission = {
      id: "pln-1",
      slug: "m",
      title: "Mission",
      state: "planned",
      created_at: FROZEN.toISOString(),
      updated_at: FROZEN.toISOString(),
    };
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    const { store: missionStore, getCalls } = makeMissionStore([seedMission]);
    await taskSplit(
      { taskStore, evidenceStore, missionStore },
      { id: parent.id, titles: ["a"] },
    );
    // tryAdvanceMission performs a missionStore.get at entry — recorded call
    // is our proxy for "the rollup was invoked for this mission".
    expect(getCalls).toContain("pln-1");
  });

  it("does NOT call tryAdvanceMission when parent.mission_id is undefined", async () => {
    const parent = seedParent(); // no mission_id
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    const { store: missionStore, getCalls } = makeMissionStore([]);
    await taskSplit(
      { taskStore, evidenceStore, missionStore },
      { id: parent.id, titles: ["a"] },
    );
    expect(getCalls).toEqual([]);
  });

  it("propagates mission-rollup failure AFTER children persist", async () => {
    const parent = seedParent({ mission_id: "pln-1" });
    const { store: taskStore, tasks } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    // First get() (assertMissionActive) returns an active mission so the
    // pre-mutation guard passes. Subsequent get() (tryAdvanceMission, after
    // splitTask committed) throws to simulate a rollup failure that must
    // NOT roll back the persisted children.
    const activeMission: Mission = {
      id: "pln-1",
      slug: "m",
      title: "Mission",
      state: "in-progress",
      created_at: FROZEN.toISOString(),
      updated_at: FROZEN.toISOString(),
    };
    let getCount = 0;
    const missionStore: MissionStorePort = {
      async create() {
        throw new Error("not used");
      },
      async get() {
        getCount += 1;
        if (getCount === 1) return activeMission;
        throw new Error("mission lookup blew up");
      },
      async update() {
        throw new Error("not used");
      },
      async list() {
        return [];
      },
      async listByState() {
        return [];
      },
    };
    let n = 0;
    const idFactory = () => `tsk-c-${++n}`;
    await expect(
      taskSplit(
        { taskStore, evidenceStore, missionStore, idFactory },
        { id: parent.id, titles: ["a", "b"] },
      ),
    ).rejects.toThrow("mission lookup blew up");
    // Despite the rollup error, the children must already be persisted —
    // splitTask committed before tryAdvanceMission ran.
    expect(tasks.get("tsk-c-1" as TaskId)).toBeDefined();
    expect(tasks.get("tsk-c-2" as TaskId)).toBeDefined();
  });

  it("re-split of an already-split parent surfaces adapter DuplicateSlugError verbatim", async () => {
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    await taskSplit(
      { taskStore, evidenceStore },
      { id: parent.id, titles: ["a", "b"] },
    );
    // Parent stays in "claimed" — the usecase doesn't transition the parent.
    // Splitting again will collide on slugs "parent-1" / "parent-2".
    await expect(
      taskSplit({ taskStore, evidenceStore }, { id: parent.id, titles: ["a", "b"] }),
    ).rejects.toBeInstanceOf(DuplicateSlugError);
  });

  it("refuses to split when parent mission is in a terminal state", async () => {
    // Mirror taskClaim/taskShip: orphaned task under a cancelled mission
    // cannot be split, so the guard short-circuits before SPLITTABLE_STATES.
    const parent = seedParent({ mission_id: "pln-dead" });
    const cancelled: Mission = {
      id: "pln-dead",
      slug: "m",
      title: "Mission",
      state: "cancelled",
      created_at: FROZEN.toISOString(),
      updated_at: FROZEN.toISOString(),
    };
    const { store: taskStore, tasks } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    const { store: missionStore } = makeMissionStore([cancelled]);
    await expect(
      taskSplit(
        { taskStore, evidenceStore, missionStore },
        { id: parent.id, titles: ["a"] },
      ),
    ).rejects.toBeInstanceOf(MissionTerminalGuardError);
    // Parent is untouched: guard fires before any mutation.
    expect(tasks.get(parent.id)).toEqual(parent);
  });

  it("constant idFactory throws DuplicateTaskIdError on the 2nd child id", async () => {
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    const { store: evidenceStore } = makeEvidence();
    const idFactory = () => "tsk-fixed-id";
    await expect(
      taskSplit(
        { taskStore, evidenceStore, idFactory },
        { id: parent.id, titles: ["a", "b"] },
      ),
    ).rejects.toBeInstanceOf(DuplicateTaskIdError);
  });

  it("test-fake mirrors adapter: malformed child id throws InvalidTaskIdError", async () => {
    // Direct adapter-level call — taskSplit() never lets callers inject
    // ci.id, but the fake must mirror the real adapter so adapter-only
    // regressions show up at this layer too.
    const parent = seedParent();
    const { store: taskStore } = makeTaskStore([parent]);
    await expect(
      taskStore.splitTask({
        parentId: parent.id,
        parentPatch: {},
        childInputs: [
          { id: "not-a-task-id", slug: "x", title: "X", state: "draft" },
        ],
      }),
    ).rejects.toBeInstanceOf(InvalidTaskIdError);
  });
});
