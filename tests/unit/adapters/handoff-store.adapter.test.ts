/**
 * Filesystem handoff store (v2, UKI v5.2) adapter tests.
 */
import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { access, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter } from "../../../src/adapters/handoff-store.adapter.js";
import { parseUki, type UkiSlots } from "../../../src/lib/uki-format.js";
import type { CreateUkiHandoffInput } from "../../../src/domain/uki-types.js";

const SAMPLE_SLOTS: UkiSlots = {
  sessionCore: "adapter_test_sample",
  causalDrivers: ["unit_test_ran"],
  divergences: [],
  keyDecisions: ["proceed_with_persistence"],
  signalDelta: ["handoffs_0~1"],
  artifacts: ["branch_feat_missionControl"],
  executionState: "tmpdir_isolated",
  boundaryState: [],
  stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
  nextAction: "verify_roundtrip",
  cs: { work: 0.9 },
  summary: "Adapter_test-roundtrip_verified-low_risk",
};

const SAMPLE_INPUT: CreateUkiHandoffInput = {
  slots: SAMPLE_SLOTS,
  agent: "claude-code",
  sessionId: "test-session-abc",
};

describe("FsHandoffStoreAdapter", () => {
  let dir: string;
  let store: FsHandoffStoreAdapter;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-handoff-"));
    store = new FsHandoffStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("creates a handoff with generated id and status pending", async () => {
    const created = await store.create(SAMPLE_INPUT);
    expect(created.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
    expect(created.status).toBe("pending");
    expect(created.agent).toBe("claude-code");
    expect(created.sessionId).toBe("test-session-abc");
    expect(created.version).toBe("5.2");
    expect(typeof created.timestamp).toBe("string");
    expect(created.slots).toEqual(SAMPLE_SLOTS);
  });

  it("caches the compressed UKI string at create time", async () => {
    const created = await store.create(SAMPLE_INPUT);
    expect(typeof created.uki).toBe("string");
    expect(created.uki.length).toBeGreaterThan(0);
    // Cached string must round-trip back to the original slots.
    const parsed = parseUki(created.uki);
    expect(parsed).toEqual(SAMPLE_SLOTS);
  });

  it("persists to .maestro/handoffs/<id>.json", async () => {
    const created = await store.create(SAMPLE_INPUT);
    const filePath = join(dir, ".maestro", "handoffs", `${created.id}.json`);
    // Should exist.
    await access(filePath);
  });

  it("retrieves a created handoff by id", async () => {
    const created = await store.create(SAMPLE_INPUT);
    const retrieved = await store.get(created.id);
    expect(retrieved).toEqual(created);
  });

  it("returns undefined for missing id", async () => {
    const result = await store.get("9999-12-31-999");
    expect(result).toBeUndefined();
  });

  it("lists all handoffs sorted newest first", async () => {
    const a = await store.create(SAMPLE_INPUT);
    const b = await store.create(SAMPLE_INPUT);
    const c = await store.create(SAMPLE_INPUT);
    const all = await store.list();
    expect(all.length).toBe(3);
    const ids = all.map((h) => h.id);
    expect(ids).toEqual([c.id, b.id, a.id]);
  });

  it("filters list by status", async () => {
    const a = await store.create(SAMPLE_INPUT);
    const b = await store.create(SAMPLE_INPUT);
    await store.updateStatus(a.id, "picked-up", { pickedUpBy: "codex" });

    const pending = await store.list({ status: "pending" });
    expect(pending.length).toBe(1);
    expect(pending[0]!.id).toBe(b.id);

    const pickedUp = await store.list({ status: "picked-up" });
    expect(pickedUp.length).toBe(1);
    expect(pickedUp[0]!.id).toBe(a.id);
  });

  it("getLatestPending returns newest pending handoff", async () => {
    const a = await store.create(SAMPLE_INPUT);
    const b = await store.create(SAMPLE_INPUT);
    const c = await store.create(SAMPLE_INPUT);

    // c is latest
    const latest1 = await store.getLatestPending();
    expect(latest1?.id).toBe(c.id);

    // Mark c as picked-up -- b should then be latest pending.
    await store.updateStatus(c.id, "picked-up");
    const latest2 = await store.getLatestPending();
    expect(latest2?.id).toBe(b.id);
  });

  it("getLatestPending returns undefined when nothing pending", async () => {
    const a = await store.create(SAMPLE_INPUT);
    await store.updateStatus(a.id, "picked-up");
    const latest = await store.getLatestPending();
    expect(latest).toBeUndefined();
  });

  it("updateStatus transitions pending -> picked-up and records pickedUpBy + pickedUpAt", async () => {
    const a = await store.create(SAMPLE_INPUT);
    const updated = await store.updateStatus(a.id, "picked-up", {
      pickedUpBy: "codex-implementer",
    });
    expect(updated?.status).toBe("picked-up");
    expect(updated?.pickedUpBy).toBe("codex-implementer");
    expect(typeof updated?.pickedUpAt).toBe("string");

    const retrieved = await store.get(a.id);
    expect(retrieved?.status).toBe("picked-up");
    expect(retrieved?.pickedUpBy).toBe("codex-implementer");
  });

  it("updateStatus transitions picked-up -> completed and records report", async () => {
    const a = await store.create(SAMPLE_INPUT);
    await store.updateStatus(a.id, "picked-up", { pickedUpBy: "codex" });
    const completed = await store.updateStatus(a.id, "completed", {
      report: "all tests green",
    });
    expect(completed?.status).toBe("completed");
    expect(completed?.report).toBe("all tests green");
    expect(typeof completed?.completedAt).toBe("string");
    // pickedUpBy should still be present from the prior transition
    expect(completed?.pickedUpBy).toBe("codex");
  });

  it("updateStatus returns undefined for missing id", async () => {
    const result = await store.updateStatus("9999-12-31-999", "completed");
    expect(result).toBeUndefined();
  });

  it("delete removes the record and returns true", async () => {
    const a = await store.create(SAMPLE_INPUT);
    const removed = await store.delete(a.id);
    expect(removed).toBe(true);
    const retrieved = await store.get(a.id);
    expect(retrieved).toBeUndefined();
  });

  it("delete returns false for missing id", async () => {
    const result = await store.delete("9999-12-31-999");
    expect(result).toBe(false);
  });

  it("list on empty store returns empty array", async () => {
    const all = await store.list();
    expect(all).toEqual([]);
  });
});
