import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { MaestroError } from "@/shared/errors.js";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createContract } from "@/features/task/usecases/contract/create-contract.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { findSimilarTasks } from "@/features/task/usecases/find-similar-tasks.usecase.js";

describe("findSimilarTasks", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;
  let contractStore: FsContractStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-similar-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
    contractStore = new FsContractStoreAdapter(tmpDir);
  });

  it("returns tasks with title overlap, ranked by overlap count", async () => {
    const target = await createTask(store, { title: "rotate jwt session tokens" });
    const single = await createTask(store, { title: "jwt rotation in auth pipeline" });
    const double = await createTask(store, { title: "audit session tokens pipeline" });
    await createTask(store, { title: "unrelated topic entirely" });

    const matches = await findSimilarTasks(store, target.id);

    expect(matches.length).toBeGreaterThan(0);
    const ids = matches.map((m) => m.task.id);
    expect(ids).toContain(single.id);
    expect(ids).toContain(double.id);
    expect(ids[0]).toBe(double.id);
  });

  it("excludes the target task itself", async () => {
    const target = await createTask(store, { title: "lonely keyword occurrence" });
    const matches = await findSimilarTasks(store, target.id);
    expect(matches.map((m) => m.task.id)).not.toContain(target.id);
  });

  it("includes receipt summary and surprise in the similarity pool", async () => {
    const completed = await createTask(store, { title: "unrelated title" });
    await updateTask(store, completed.id, {
      status: "completed",
      summary: "migration engine was broken",
      surprise: "snapshot restore skipped indexes",
    });

    const target = await createTask(store, { title: "run migration engine cleanup" });
    const matches = await findSimilarTasks(store, target.id);

    expect(matches.map((m) => m.task.id)).toContain(completed.id);
  });

  it("breaks overlap ties by recency (newer updatedAt first)", async () => {
    const older = await createTask(store, { title: "indexing improvements" });
    const newer = await createTask(store, { title: "indexing improvements" });

    await updateTask(store, newer.id, { description: "refresh" });

    const target = await createTask(store, { title: "indexing improvements plan" });
    const matches = await findSimilarTasks(store, target.id);
    const ids = matches.map((m) => m.task.id);
    expect(ids.indexOf(newer.id)).toBeLessThan(ids.indexOf(older.id));
  });

  it("returns empty array when the target has no extractable keywords", async () => {
    const target = await createTask(store, { title: "a an" });
    const matches = await findSimilarTasks(store, target.id);
    expect(matches).toEqual([]);
  });

  it("caps results at the provided limit", async () => {
    for (let i = 0; i < 8; i++) {
      await createTask(store, { title: `demo keyword occurrence ${i}` });
    }
    const target = await createTask(store, { title: "demo keyword target" });
    const matches = await findSimilarTasks(store, target.id, 3);
    expect(matches.length).toBe(3);
  });

  it("rejects negative limits", async () => {
    const target = await createTask(store, { title: "demo keyword target" });

    await expect(findSimilarTasks(store, target.id, -1)).rejects.toThrow(MaestroError);
  });

  it("includes contract intent and criteria text in the similarity pool", async () => {
    const contracted = await createTask(store, { title: "unrelated backlog item" });
    await createContract(store, contractStore, {
      taskId: contracted.id,
      repoRoot: tmpDir,
      intent: "stabilize websocket backpressure flow",
      scope: {
        filesExpected: ["src/features/task/**"],
        filesForbidden: [],
      },
      doneWhen: [
        {
          text: "backpressure metrics are covered",
          kind: "manual",
        },
      ],
      createdBy: "user",
      configSnapshot: {
        strict: false,
        overlapPolicy: "fail",
        rebaseFallback: "best-effort",
        staleReclaimContractPolicy: "inherit",
      },
    });

    const target = await createTask(store, { title: "plan websocket metrics cleanup" });
    const matches = await findSimilarTasks(store, target.id, 5, contractStore);

    expect(matches.map((match) => match.task.id)).toContain(contracted.id);
  });
});
