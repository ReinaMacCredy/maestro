import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsRuntimeStoreAdapter } from "../../../src/adapters/runtime-store.adapter.js";
import type { WorkerRuntime } from "../../../src/domain/runtime-types.js";

describe("FsRuntimeStoreAdapter", () => {
  let tmpDir: string;
  let store: FsRuntimeStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "runtime-store-"));
    store = new FsRuntimeStoreAdapter(tmpDir);
  });

  it("saves and reads a runtime record", async () => {
    const runtime: WorkerRuntime = {
      featureId: "f1",
      attemptId: "attempt-1",
      attempt: 1,
      agent: "unknown",
      runtimeState: "starting",
      startedAt: "2026-04-01T00:00:00.000Z",
      lastSeenAt: "2026-04-01T00:00:00.000Z",
      leaseExpiresAt: "2026-04-01T00:02:00.000Z",
      recoveryMetadata: {
        retryCount: 0,
        history: [],
      },
    };

    await store.save("2026-04-01-001", "f1", runtime);

    await expect(store.get("2026-04-01-001", "f1")).resolves.toEqual(runtime);
  });

  it("lists runtimes for a mission", async () => {
    await store.save("2026-04-01-001", "f2", {
      featureId: "f2",
      attemptId: "attempt-2",
      attempt: 1,
      agent: "unknown",
      runtimeState: "starting",
      startedAt: "2026-04-01T00:00:00.000Z",
      lastSeenAt: "2026-04-01T00:00:00.000Z",
      leaseExpiresAt: "2026-04-01T00:02:00.000Z",
      recoveryMetadata: {
        retryCount: 0,
        history: [],
      },
    });
    await store.save("2026-04-01-001", "f1", {
      featureId: "f1",
      attemptId: "attempt-1",
      attempt: 1,
      agent: "unknown",
      runtimeState: "starting",
      startedAt: "2026-04-01T00:00:00.000Z",
      lastSeenAt: "2026-04-01T00:00:00.000Z",
      leaseExpiresAt: "2026-04-01T00:02:00.000Z",
      recoveryMetadata: {
        retryCount: 0,
        history: [],
      },
    });

    await expect(store.list("2026-04-01-001")).resolves.toHaveLength(2);
    await expect(store.list("2026-04-01-001")).resolves.toMatchObject([
      { featureId: "f1" },
      { featureId: "f2" },
    ]);
  });
});
