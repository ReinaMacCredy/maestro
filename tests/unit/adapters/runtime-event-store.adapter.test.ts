import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsRuntimeEventStoreAdapter } from "../../../src/adapters/runtime-event-store.adapter.js";

describe("FsRuntimeEventStoreAdapter", () => {
  let baseDir: string;

  beforeEach(async () => {
    baseDir = await mkdtemp(join(tmpdir(), "maestro-runtime-events-"));
  });

  afterEach(async () => {
    await rm(baseDir, { recursive: true, force: true });
  });

  it("appends and reads feature-scoped runtime events in timestamp order", async () => {
    const store = new FsRuntimeEventStoreAdapter(baseDir);

    await store.append("mission-1", {
      id: "evt-1",
      missionId: "mission-1",
      featureId: "f1",
      attemptId: "attempt-1",
      worker: "codex",
      timestamp: "2026-04-02T12:00:00.000Z",
      kind: "status",
      text: "started",
    });
    await store.append("mission-1", {
      id: "evt-2",
      missionId: "mission-1",
      featureId: "f1",
      attemptId: "attempt-1",
      worker: "codex",
      timestamp: "2026-04-02T12:00:01.000Z",
      kind: "stdout",
      text: "Reading files",
    });

    const events = await store.listByFeature("mission-1", "f1");

    expect(events.map((event) => event.id)).toEqual(["evt-1", "evt-2"]);
    expect(events[1]?.text).toBe("Reading files");
  });

  it("tails recent feature-scoped runtime events without requiring the full file", async () => {
    const store = new FsRuntimeEventStoreAdapter(baseDir);

    for (let index = 0; index < 40; index += 1) {
      await store.append("mission-1", {
        id: `evt-${index}`,
        missionId: "mission-1",
        featureId: "f1",
        attemptId: "attempt-1",
        worker: "codex",
        timestamp: `2026-04-02T12:00:${String(index).padStart(2, "0")}.000Z`,
        kind: "stdout",
        text: `line-${index}-${"x".repeat(80)}`,
      });
    }

    const events = await store.tailByFeature("mission-1", "f1", {
      maxBytes: 2_048,
      maxLines: 5,
    });

    expect(events.map((event) => event.id)).toEqual([
      "evt-35",
      "evt-36",
      "evt-37",
      "evt-38",
      "evt-39",
    ]);
  });
});
