import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, open, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsRuntimeEventStoreAdapter } from "../../../src/adapters/runtime-event-store.adapter.js";

interface ReadableFileHandle {
  read(
    buffer: Buffer,
    offset: number,
    length: number,
    position: number,
  ): Promise<{ bytesRead: number; buffer: Buffer }>;
  close(): Promise<void>;
}

async function appendRuntimeEvents(
  store: FsRuntimeEventStoreAdapter,
  count: number,
  textBuilder: (index: number) => string = (index) => `line-${index}-${"x".repeat(80)}`,
): Promise<void> {
  for (let index = 0; index < count; index += 1) {
    await store.append("mission-1", {
      id: `evt-${index}`,
      missionId: "mission-1",
      featureId: "f1",
      attemptId: "attempt-1",
      worker: "codex",
      timestamp: `2026-04-02T12:00:${String(index).padStart(2, "0")}.000Z`,
      kind: "stdout",
      text: textBuilder(index),
    });
  }
}

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
    await appendRuntimeEvents(store, 40);

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

  it("honors partial reads when tailing recent runtime events", async () => {
    const store = new FsRuntimeEventStoreAdapter(baseDir);
    await appendRuntimeEvents(store, 40);

    const eventPath = join(
      baseDir,
      ".maestro",
      "missions",
      "mission-1",
      "workers",
      "f1",
      "events.jsonl",
    );
    const probeHandle = await open(eventPath, "r") as unknown as ReadableFileHandle;
    const fileHandleProto = Object.getPrototypeOf(probeHandle) as {
      read: ReadableFileHandle["read"];
    };
    await probeHandle.close();

    const originalRead = fileHandleProto.read;
    let readCalls = 0;

    fileHandleProto.read = async function patchedRead(
      buffer,
      offset,
      length,
      position,
    ) {
      readCalls += 1;
      const cappedLength = readCalls === 1 ? Math.max(1, Math.min(length - 1, 128)) : length;
      return originalRead.call(this, buffer, offset, cappedLength, position);
    };

    try {
      const events = await store.tailByFeature("mission-1", "f1", {
        maxBytes: 2_048,
        maxLines: 5,
      });

      expect(readCalls).toBeGreaterThanOrEqual(2);
      expect(events.map((event) => event.id)).toEqual([
        "evt-35",
        "evt-36",
        "evt-37",
        "evt-38",
        "evt-39",
      ]);
    } finally {
      fileHandleProto.read = originalRead;
    }
  });

  it("returns an empty tail when the byte window lands inside a single oversized line", async () => {
    const store = new FsRuntimeEventStoreAdapter(baseDir);
    await appendRuntimeEvents(store, 1, () => "x".repeat(6_000));

    const events = await store.tailByFeature("mission-1", "f1", {
      maxBytes: 128,
      maxLines: 5,
    });

    expect(events).toEqual([]);
  });

  it("parses the last complete runtime event even when the file has no trailing newline", async () => {
    const eventDir = join(baseDir, ".maestro", "missions", "mission-1", "workers", "f1");
    const eventPath = join(eventDir, "events.jsonl");
    await mkdir(eventDir, { recursive: true });
    await writeFile(
      eventPath,
      [
        JSON.stringify({
          id: "evt-1",
          missionId: "mission-1",
          featureId: "f1",
          attemptId: "attempt-1",
          worker: "codex",
          timestamp: "2026-04-02T12:00:00.000Z",
          kind: "stdout",
          text: "first line",
        }),
        JSON.stringify({
          id: "evt-2",
          missionId: "mission-1",
          featureId: "f1",
          attemptId: "attempt-1",
          worker: "codex",
          timestamp: "2026-04-02T12:00:01.000Z",
          kind: "stdout",
          text: "second line",
        }),
      ].join("\n"),
    );

    const store = new FsRuntimeEventStoreAdapter(baseDir);
    const events = await store.tailByFeature("mission-1", "f1", {
      maxBytes: 4_096,
      maxLines: 2,
    });

    expect(events.map((event) => event.id)).toEqual(["evt-1", "evt-2"]);
  });
});
