import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlEvidenceStore } from "@/repo/jsonl-evidence-store.adapter.js";
import type { TransitionEvidenceRow, LintViolationEvidenceRow } from "@/repo/evidence-store.port.js";

const FROZEN = new Date("2026-05-15T12:34:56.000Z");

describe("JsonlEvidenceStore", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-evidence-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("returns empty list when no evidence directory exists yet", async () => {
    const store = new JsonlEvidenceStore({ repoRoot: root, clock: () => FROZEN });
    const rows = await store.list();
    expect(rows).toEqual([]);
  });

  it("appends a transition row to a date-stamped JSONL file and reads it back", async () => {
    const store = new JsonlEvidenceStore({ repoRoot: root, clock: () => FROZEN });
    const row: TransitionEvidenceRow = {
      id: "evd-x",
      kind: "transition",
      timestamp: FROZEN.toISOString(),
      task_id: "tsk-1",
      from_state: "draft",
      to_state: "claimed",
      trigger_verb: "task:claim",
    };
    await store.append(row);
    const rows = await store.list();
    expect(rows).toEqual([row]);
  });

  it("filters by task_id, mission_id, and kind", async () => {
    const store = new JsonlEvidenceStore({ repoRoot: root, clock: () => FROZEN });
    const t1: TransitionEvidenceRow = {
      id: "a",
      kind: "transition",
      timestamp: FROZEN.toISOString(),
      task_id: "tsk-1",
      from_state: null,
      to_state: "draft",
      trigger_verb: "task:from-spec",
    };
    const t2: TransitionEvidenceRow = {
      id: "b",
      kind: "transition",
      timestamp: FROZEN.toISOString(),
      task_id: "tsk-2",
      from_state: "draft",
      to_state: "claimed",
      trigger_verb: "task:claim",
    };
    const lint: LintViolationEvidenceRow = {
      id: "c",
      kind: "lint-violation",
      timestamp: FROZEN.toISOString(),
      task_id: "tsk-1",
      rule_id: "no-runner-inversion",
      severity: "error",
      file: "src/service/foo.ts",
      message: "spawn forbidden",
    };
    await store.append(t1);
    await store.append(t2);
    await store.append(lint);

    expect((await store.list({ task_id: "tsk-1" })).length).toBe(2);
    expect((await store.list({ kind: "lint-violation" })).length).toBe(1);
    expect((await store.list({ task_id: "tsk-2", kind: "transition" })).length).toBe(1);
  });

  it("read(id) returns the matching row across multiple date files", async () => {
    const earlier = new Date("2026-05-14T08:00:00.000Z");
    const later = new Date("2026-05-15T08:00:00.000Z");
    let now = earlier;
    const store = new JsonlEvidenceStore({ repoRoot: root, clock: () => now });
    await store.append({
      id: "evd-old",
      kind: "transition",
      timestamp: earlier.toISOString(),
      task_id: "tsk-1",
      from_state: null,
      to_state: "draft",
      trigger_verb: "task:from-spec",
    });
    now = later;
    await store.append({
      id: "evd-new",
      kind: "transition",
      timestamp: later.toISOString(),
      task_id: "tsk-1",
      from_state: "draft",
      to_state: "claimed",
      trigger_verb: "task:claim",
    });
    const hit = await store.read("evd-old");
    expect(hit?.id).toBe("evd-old");
    const miss = await store.read("evd-missing");
    expect(miss).toBeUndefined();
  });

  it("read() returns undefined when no evidence directory exists yet", async () => {
    const store = new JsonlEvidenceStore({ repoRoot: root, clock: () => FROZEN });
    expect(await store.read("evd-x")).toBeUndefined();
  });

  it("survives multiple appends to the same file (concurrency smoke)", async () => {
    const store = new JsonlEvidenceStore({ repoRoot: root, clock: () => FROZEN });
    const writes: Promise<void>[] = [];
    for (let i = 0; i < 10; i++) {
      writes.push(
        store.append({
          id: `r-${i}`,
          kind: "transition",
          timestamp: FROZEN.toISOString(),
          task_id: "tsk-x",
          from_state: "doing",
          to_state: "verifying",
          trigger_verb: "task:verify",
        }),
      );
    }
    await Promise.all(writes);
    const rows = await store.list();
    expect(rows.length).toBe(10);
    expect(new Set(rows.map((r) => r.id)).size).toBe(10);
  });
});
