import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
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
import { TaskNotFoundError } from "@/repo/task-store.port.js";
import type { SpecStorePort } from "@/repo/spec-store.port.js";
import type { Task, TaskId } from "@/types/task.js";
import type { TaskState } from "@/types/task-state.js";
import { SpecFileNotFoundError, taskFromSpec } from "@/service/task-from-spec.usecase.js";

function makeTaskStore(): TaskStorePort {
  const tasks = new Map<TaskId, Task>();
  const create = async (input: CreateTaskInput): Promise<Task> => {
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
  };
  return {
    create,
    async createMany(inputs: readonly CreateTaskInput[]) {
      const out: Task[] = [];
      for (const i of inputs) out.push(await create(i));
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
      const updated: Task = { ...existing, ...patch, id: existing.id, slug: existing.slug, created_at: existing.created_at, updated_at: new Date().toISOString() };
      tasks.set(id, updated);
      return updated;
    },
    async list() { return [...tasks.values()]; },
    async listByState(state: TaskState) { return [...tasks.values()].filter((t) => t.state === state); },
    async listByMissionId(mission_id: string) { return [...tasks.values()].filter((t) => t.mission_id === mission_id); },
  };
}

function makeEvidence(): EvidenceStorePort {
  const rows: EvidenceRow[] = [];
  return {
    async append(row) { rows.push(row); },
    async list(_filter?: EvidenceFilter) { return rows; },
    async read(id) { return rows.find((r) => r.id === id); },
  };
}

const stubSpecStore: SpecStorePort = {
  async load() { throw new Error("not used"); },
};

describe("taskFromSpec", () => {
  let repoRoot: string;

  beforeEach(async () => {
    repoRoot = await mkdtemp(join(tmpdir(), "maestro-task-from-spec-"));
    await mkdir(join(repoRoot, ".maestro/specs"), { recursive: true });
  });

  afterEach(async () => {
    await rm(repoRoot, { recursive: true, force: true });
  });

  it("throws SpecFileNotFoundError when the spec path does not exist", async () => {
    const taskStore = makeTaskStore();
    const evidenceStore = makeEvidence();

    let caught: unknown;
    try {
      await taskFromSpec(
        { repoRoot, specStore: stubSpecStore, taskStore, evidenceStore },
        ".maestro/specs/nope.md",
      );
    } catch (err) {
      caught = err;
    }

    expect(caught).toBeInstanceOf(SpecFileNotFoundError);
    expect((caught as SpecFileNotFoundError).inputArg).toBe(".maestro/specs/nope.md");
    expect((caught as SpecFileNotFoundError).path).toBe(join(repoRoot, ".maestro/specs/nope.md"));
  });

  it("SpecFileNotFoundError preserves bare-slug inputArg so the command layer can suggest a path", async () => {
    const taskStore = makeTaskStore();
    const evidenceStore = makeEvidence();

    let caught: unknown;
    try {
      await taskFromSpec(
        { repoRoot, specStore: stubSpecStore, taskStore, evidenceStore },
        "my-slug",
      );
    } catch (err) {
      caught = err;
    }

    expect(caught).toBeInstanceOf(SpecFileNotFoundError);
    expect((caught as SpecFileNotFoundError).inputArg).toBe("my-slug");
  });

  it("creates a draft task when the spec exists", async () => {
    const specPath = join(repoRoot, ".maestro/specs/demo.md");
    await writeFile(
      specPath,
      `---\nslug: demo\nwork_type: change-request\nintent: demo\nrisk_class: low\nmode: light\nacceptance_criteria:\n  - "task gets created"\n---\n\n# Demo title\n`,
    );

    const taskStore = makeTaskStore();
    const evidenceStore = makeEvidence();
    const task = await taskFromSpec(
      { repoRoot, specStore: stubSpecStore, taskStore, evidenceStore },
      ".maestro/specs/demo.md",
    );

    expect(task.state).toBe("draft");
    expect(task.slug).toBe("demo");
    expect(task.title).toBe("Demo title");
  });
});
