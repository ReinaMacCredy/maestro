import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsCandidateStoreAdapter } from "@/features/task/adapters/fs-candidate-store.adapter.js";
import type { Task } from "@/features/task/domain/task-types.js";
import { captureTaskCandidate } from "@/features/task/usecases/capture-task-candidate.usecase.js";

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-abc123",
    title: "Sample",
    type: "task",
    priority: 2,
    status: "completed",
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-04-10T00:00:00.000Z",
    updatedAt: "2026-04-10T00:00:00.000Z",
    ...overrides,
  };
}

describe("captureTaskCandidate", () => {
  let tmpDir: string;
  let store: FsCandidateStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-capture-"));
    store = new FsCandidateStoreAdapter(tmpDir);
  });

  it("captures a candidate when a completion reason is present", async () => {
    const result = await captureTaskCandidate(store, task({
      id: "tsk-argon",
      title: "Implement argon2 password hashing",
      closeReason: "argon2 compare was backwards",
    }));

    expect(result?.id).toBe("tsk-argon");
    expect(result?.reason).toBe("argon2 compare was backwards");
    expect(result?.keywords).toContain("argon2");
    expect(result?.keywords).toContain("password");
  });

  it("returns undefined when the reason is missing or blank", async () => {
    expect(await captureTaskCandidate(store, task({ closeReason: undefined }))).toBeUndefined();
    expect(await captureTaskCandidate(store, task({ closeReason: "   " }))).toBeUndefined();
  });

  it("persists candidates to disk", async () => {
    await captureTaskCandidate(store, task({
      id: "tsk-a1b2c3",
      title: "Persistence",
      closeReason: "some reason",
    }));

    const all = await store.all();
    expect(all).toHaveLength(1);
    expect(all[0]?.id).toBe("tsk-a1b2c3");
  });
});
