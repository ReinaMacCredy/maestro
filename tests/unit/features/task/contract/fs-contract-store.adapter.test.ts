import { beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { MaestroError } from "@/shared/errors.js";

function createInput(overrides: Partial<Contract> = {}) {
  return {
    taskId: "tsk-a1b2c3",
    repoRoot: "/repo",
    intent: "Implement task contracts",
    scope: {
      filesExpected: ["src/features/task/**"],
      filesForbidden: [],
    },
    doneWhen: [
      {
        id: "dw-a1b2c3",
        text: "contract store exists",
        kind: "manual" as const,
      },
    ],
    createdAt: "2026-04-21T00:00:00.000Z",
    createdBy: "user",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail" as const,
      rebaseFallback: "best-effort" as const,
      staleReclaimContractPolicy: "inherit" as const,
    },
    ...overrides,
  };
}

describe("FsContractStoreAdapter", () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "contract-store-"));
  });

  it("creates draft contracts, persists them, and appends the index", async () => {
    const store = new FsContractStoreAdapter(tmpDir);

    const created = await store.create(createInput());

    expect(created.id).toMatch(/^c-[0-9a-f]{6}$/);
    expect(created.status).toBe("draft");
    expect((await store.get(created.id))?.taskId).toBe("tsk-a1b2c3");
    expect((await store.getByTaskId("tsk-a1b2c3"))?.id).toBe(created.id);

    const index = await readFile(join(tmpDir, ".maestro", "tasks", "contracts", "index.jsonl"), "utf8");
    expect(index).toContain(created.id);
    expect(index).toContain("\"status\":\"draft\"");
  });

  it("persists saved contract updates across store instances", async () => {
    const store = new FsContractStoreAdapter(tmpDir);
    const created = await store.create(createInput());
    const saved = await store.save({
      ...created,
      status: "locked",
      lockedAt: "2026-04-21T01:00:00.000Z",
      lockedBy: "session:codex:1",
    });

    expect(saved.status).toBe("locked");

    const fresh = new FsContractStoreAdapter(tmpDir);
    expect((await fresh.get(created.id))?.lockedBy).toBe("session:codex:1");
  });

  it("resolves a task contract from the index without loading unrelated broken files", async () => {
    const store = new FsContractStoreAdapter(tmpDir);
    const created = await store.create(createInput());
    const contractsDir = join(tmpDir, ".maestro", "tasks", "contracts");
    await Bun.write(join(contractsDir, "c-badbad.json"), "{not json");
    await Bun.write(
      join(contractsDir, "index.jsonl"),
      [
        JSON.stringify({
          id: created.id,
          taskId: created.taskId,
          status: "draft",
          at: created.createdAt,
        }),
        JSON.stringify({
          id: "c-badbad",
          taskId: "tsk-badbad",
          status: "draft",
          at: "2026-04-21T00:30:00.000Z",
        }),
        "",
      ].join("\n"),
    );

    const resolved = await store.getByTaskId(created.taskId);
    expect(resolved?.id).toBe(created.id);
  });

  it("rejects saving a second active contract in the same repo when overlap policy is fail", async () => {
    const store = new FsContractStoreAdapter(tmpDir);
    const first = await store.create(createInput({ taskId: "tsk-a1b2c3" }));
    const second = await store.create(createInput({ taskId: "tsk-b2c3d4" }));

    await store.save({
      ...first,
      status: "locked",
      lockedAt: "2026-04-21T01:00:00.000Z",
      lockedBy: "session:codex:1",
    });

    await expect(store.save({
      ...second,
      status: "locked",
      lockedAt: "2026-04-21T01:05:00.000Z",
      lockedBy: "session:codex:2",
    })).rejects.toThrow("overlaps an active contract in the same repo");
  });

  it("retries one generated id collision before failing", async () => {
    const store = new FsContractStoreAdapter(tmpDir, {
      generateId: (() => {
        let count = 0;
        return () => {
          count += 1;
          return count === 1 ? "c-deadbe" : "c-feed01";
        };
      })(),
    });

    await store.create(createInput({ id: "c-deadbe", taskId: "tsk-deadbe" }));
    const created = await store.create(createInput({ taskId: "tsk-feed01" }));

    expect(created.id).toBe("c-feed01");
  });

  it("removes a contract file and appends a discard index entry", async () => {
    const store = new FsContractStoreAdapter(tmpDir);
    const created = await store.create(createInput());

    const removed = await store.delete(created.id, {
      taskId: created.taskId,
      status: "discarded",
      at: "2026-04-21T03:00:00.000Z",
      reason: "task_deleted",
    });

    expect(removed).toBe(true);
    expect(await store.get(created.id)).toBeUndefined();

    const index = await readFile(join(tmpDir, ".maestro", "tasks", "contracts", "index.jsonl"), "utf8");
    expect(index).toContain("\"status\":\"discarded\"");
    expect(index).toContain("\"reason\":\"task_deleted\"");
  });

  it("rejects invalid hand-edited contract JSON", async () => {
    const contractsDir = join(tmpDir, ".maestro", "tasks", "contracts");
    await mkdir(contractsDir, { recursive: true });
    await Bun.write(
      join(contractsDir, "c-badbad.json"),
      JSON.stringify({
        id: "c-badbad",
        taskId: "tsk-a1b2c3",
      }),
    );

    const store = new FsContractStoreAdapter(tmpDir);
    await expect(store.get("c-badbad")).rejects.toThrow(MaestroError);
  });

  it("rejects traversal segments in the contract id on delete", async () => {
    const store = new FsContractStoreAdapter(tmpDir);
    await expect(
      store.delete("../../etc/passwd", {
        taskId: "tsk-a1b2c3",
        at: "2026-04-21T00:00:00.000Z",
      }),
    ).rejects.toThrow(/Invalid contract id/);
  });

  it("rejects traversal segments in the contract id on get", async () => {
    const store = new FsContractStoreAdapter(tmpDir);
    // Malformed ids resolve to undefined without touching disk.
    expect(await store.get("../../etc/passwd")).toBeUndefined();
  });
});
