import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { EvidenceRow, EvidenceStorePort } from "@/repo/evidence-store.port.js";
import {
  TaskNotFoundError,
  type CreateTaskInput,
  type TaskPatch,
  type TaskStorePort,
} from "@/repo/task-store.port.js";
import type {
  CreateWorktreeInput,
  WorktreeRecord,
  WorktreeStorePort,
} from "@/repo/worktree-store.port.js";
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
        spec_path: input.spec_path,
        blocked_by: input.blocked_by ?? [],
        created_at: now,
        updated_at: now,
      };
      tasks.set(t.id, t);
      return t;
    },
    async createMany(inputs: readonly CreateTaskInput[]) {
      const out: Task[] = [];
      for (const i of inputs) out.push(await this.create(i));
      return out;
    },
    async splitTask(_input) {
      throw new Error("splitTask not stubbed in this test");
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

function makeWorktreeStore(): {
  store: WorktreeStorePort;
  creates: CreateWorktreeInput[];
} {
  const creates: CreateWorktreeInput[] = [];
  const records = new Map<string, WorktreeRecord>();
  return {
    creates,
    store: {
      async create(input) {
        creates.push(input);
        const record: WorktreeRecord = {
          task_id: input.task_id,
          slug: input.slug,
          path: `/tmp/wt/${input.task_id}`,
          branch: `feat/${input.slug}`,
          base_branch: input.base_branch ?? "main",
          created_at: new Date().toISOString(),
        };
        records.set(input.task_id, record);
        return record;
      },
      async get(id) {
        return records.get(id);
      },
      async list() {
        return [...records.values()];
      },
    },
  };
}

function seedTask(spec_path: string | undefined): Task {
  const now = new Date().toISOString();
  return {
    id: "tsk-claim",
    slug: "demo",
    title: "Demo task",
    state: "draft",
    spec_path,
    blocked_by: [],
    created_at: now,
    updated_at: now,
  };
}

async function writeSpec(dir: string, mode: "heavy" | "light"): Promise<string> {
  const specPath = join(dir, "spec.md");
  await mkdir(dir, { recursive: true });
  const front = `---
slug: demo
acceptance_criteria:
  - x
non_goals: []
risk_class: low
mode: ${mode}
work_type: spec-slice
---

# Demo spec
`;
  await writeFile(specPath, front, "utf8");
  return specPath;
}

describe("taskClaim worktree integration (PR 34)", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-claim-wt-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("creates a worktree when the spec is heavy-mode", async () => {
    const specPath = await writeSpec(root, "heavy");
    const taskStore = makeTaskStore([seedTask(specPath)]);
    const evidenceStore = makeEvidence();
    const { store: worktreeStore, creates } = makeWorktreeStore();
    const claimed = await taskClaim(
      { taskStore, evidenceStore, worktreeStore },
      { id: "tsk-claim" },
    );
    expect(creates.length).toBe(1);
    expect(creates[0]!.task_id).toBe("tsk-claim");
    expect(claimed.worktree_path).toBe("/tmp/wt/tsk-claim");
  });

  it("does not create a worktree when the spec is light-mode", async () => {
    const specPath = await writeSpec(root, "light");
    const taskStore = makeTaskStore([seedTask(specPath)]);
    const evidenceStore = makeEvidence();
    const { store: worktreeStore, creates } = makeWorktreeStore();
    const claimed = await taskClaim(
      { taskStore, evidenceStore, worktreeStore },
      { id: "tsk-claim" },
    );
    expect(creates.length).toBe(0);
    expect(claimed.worktree_path).toBeUndefined();
  });

  it("does not create a worktree when the task has no spec_path", async () => {
    const taskStore = makeTaskStore([seedTask(undefined)]);
    const evidenceStore = makeEvidence();
    const { store: worktreeStore, creates } = makeWorktreeStore();
    const claimed = await taskClaim(
      { taskStore, evidenceStore, worktreeStore },
      { id: "tsk-claim" },
    );
    expect(creates.length).toBe(0);
    expect(claimed.worktree_path).toBeUndefined();
  });

  it("skipWorktree=true bypasses worktree creation even for heavy specs", async () => {
    const specPath = await writeSpec(root, "heavy");
    const taskStore = makeTaskStore([seedTask(specPath)]);
    const evidenceStore = makeEvidence();
    const { store: worktreeStore, creates } = makeWorktreeStore();
    const claimed = await taskClaim(
      { taskStore, evidenceStore, worktreeStore },
      { id: "tsk-claim", skipWorktree: true },
    );
    expect(creates.length).toBe(0);
    expect(claimed.worktree_path).toBeUndefined();
  });

  it("reuses an existing worktree record instead of creating a duplicate", async () => {
    const specPath = await writeSpec(root, "heavy");
    const taskStore = makeTaskStore([seedTask(specPath)]);
    const evidenceStore = makeEvidence();
    const { store: worktreeStore, creates } = makeWorktreeStore();
    await worktreeStore.create({ task_id: "tsk-claim", slug: "demo" });
    creates.length = 0;
    const claimed = await taskClaim(
      { taskStore, evidenceStore, worktreeStore },
      { id: "tsk-claim" },
    );
    expect(creates.length).toBe(0);
    expect(claimed.worktree_path).toBe("/tmp/wt/tsk-claim");
  });

  it("still claims the task when worktree creation throws", async () => {
    const specPath = await writeSpec(root, "heavy");
    const taskStore = makeTaskStore([seedTask(specPath)]);
    const evidenceStore = makeEvidence();
    const worktreeStore: WorktreeStorePort = {
      async create() {
        throw new Error("git not available");
      },
      async get() {
        return undefined;
      },
      async list() {
        return [];
      },
    };
    const claimed = await taskClaim(
      { taskStore, evidenceStore, worktreeStore },
      { id: "tsk-claim" },
    );
    expect(claimed.state).toBe("claimed");
    expect(claimed.worktree_path).toBeUndefined();
  });

  it("works when no worktreeStore is wired in (backward compat)", async () => {
    const specPath = await writeSpec(root, "heavy");
    const taskStore = makeTaskStore([seedTask(specPath)]);
    const evidenceStore = makeEvidence();
    const claimed = await taskClaim({ taskStore, evidenceStore }, { id: "tsk-claim" });
    expect(claimed.state).toBe("claimed");
    expect(claimed.worktree_path).toBeUndefined();
  });
});
