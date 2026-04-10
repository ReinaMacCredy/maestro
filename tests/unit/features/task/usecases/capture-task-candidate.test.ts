import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { captureTaskCandidate } from "@/features/task/usecases/capture-task-candidate.usecase.js";
import { FsCandidateStoreAdapter } from "@/features/task/adapters/fs-candidate-store.adapter.js";
import type { Task } from "@/features/task/domain/task-types.js";

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-abc123",
    title: "Sample",
    type: "task",
    priority: 2,
    status: "closed",
    labels: [],
    dependsOn: [],
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

  it("captures a candidate when a close reason is present", async () => {
    const t = task({
      id: "tsk-argon",
      title: "Implement argon2 password hashing",
      closeReason: "argon2 compare was backwards",
    });

    const result = await captureTaskCandidate(store, t);
    expect(result).toBeDefined();
    expect(result?.id).toBe("tsk-argon");
    expect(result?.sourceTaskId).toBe("tsk-argon");
    expect(result?.sourceType).toBe("task-close");
    expect(result?.reason).toBe("argon2 compare was backwards");
    expect(result?.keywords).toContain("argon2");
    expect(result?.keywords).toContain("compare");
    expect(result?.keywords).toContain("password");
  });

  it("persists the candidate to disk", async () => {
    const t = task({
      id: "tsk-persist",
      title: "Persistence",
      closeReason: "some reason",
    });
    await captureTaskCandidate(store, t);

    const all = await store.all();
    expect(all.length).toBe(1);
    expect(all[0]?.id).toBe("tsk-persist");
  });

  it("returns undefined when closeReason is missing", async () => {
    const t = task({ closeReason: undefined });
    const result = await captureTaskCandidate(store, t);
    expect(result).toBeUndefined();
    expect(await store.all()).toEqual([]);
  });

  it("returns undefined when closeReason is whitespace only", async () => {
    const t = task({ closeReason: "   \n\t " });
    const result = await captureTaskCandidate(store, t);
    expect(result).toBeUndefined();
    expect(await store.all()).toEqual([]);
  });

  it("returns undefined when keyword extraction produces nothing", async () => {
    // All stop words and tokens under 3 chars.
    const t = task({
      title: "a b c",
      closeReason: "is or",
    });
    const result = await captureTaskCandidate(store, t);
    expect(result).toBeUndefined();
    expect(await store.all()).toEqual([]);
  });

  it("trims whitespace from the stored reason", async () => {
    const t = task({
      title: "Trim test",
      closeReason: "   leading and trailing whitespace   ",
    });
    const result = await captureTaskCandidate(store, t);
    expect(result?.reason).toBe("leading and trailing whitespace");
  });
});
