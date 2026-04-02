import { describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsExecutionStoreAdapter } from "../../../src/adapters/execution-store.adapter.js";

describe("FsExecutionStoreAdapter", () => {
  it("persists and lists execution records", async () => {
    const dir = await mkdtemp(join(tmpdir(), "exec-store-"));
    const store = new FsExecutionStoreAdapter(dir);

    await store.save("mission-1", {
      id: "attempt-1",
      missionId: "mission-1",
      featureId: "feature-1",
      worker: "codex",
      transport: "cli",
      attemptId: "attempt-1",
      startedAt: "2026-04-02T10:00:00.000Z",
      completedAt: "2026-04-02T10:00:05.000Z",
      durationMs: 5000,
      success: true,
      exitCode: 0,
      summary: "done",
      stdoutRaw: "",
      stderrRaw: "",
      filesChanged: ["src/file.ts"],
    });

    const records = await store.list("mission-1");
    const byFeature = await store.getByFeature("mission-1", "feature-1");

    expect(records).toHaveLength(1);
    expect(byFeature).toHaveLength(1);
    expect(records[0]?.id).toBe("attempt-1");
  });

  it("rejects path traversal in execution ids", async () => {
    const dir = await mkdtemp(join(tmpdir(), "exec-store-"));
    const store = new FsExecutionStoreAdapter(dir);

    await expect(store.get("mission-1", "../escape")).rejects.toThrow("execution ID");
  });
});
