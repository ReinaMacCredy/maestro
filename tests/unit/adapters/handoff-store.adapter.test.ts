/**
 * Filesystem handoff store (v2, UKI v5.3) adapter tests.
 */
import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { access, mkdir, mkdtemp, rm, utimes, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { MaestroError } from "../../../src/domain/errors.js";
import { FsHandoffStoreAdapter } from "../../../src/adapters/handoff-store.adapter.js";
import { parseUki, type UkiSlots } from "../../../src/lib/uki-format.js";
import type { CreateUkiHandoffInput } from "../../../src/domain/uki-types.js";

const SAMPLE_SLOTS: UkiSlots = {
  sessionCore: "adapter_test_sample",
  causalDrivers: ["unit_test_ran"],
  divergences: [],
  keyDecisions: ["proceed_with_persistence"],
  decisionBasis: ["keep_store_roundtrip_safe"],
  signalDelta: ["handoffs_0~1"],
  validationState: ["unit_green"],
  executionState: "tmpdir_isolated",
  boundaryState: [],
  nextAction: "verify_roundtrip",
  artifacts: ["branch_feat_missionControl"],
  stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
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
      expect(created.version).toBe("5.3");
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

    it("normalizes persisted v5.2 slot records to the v5.3 slot shape on read", async () => {
      const id = "2026-04-09-123";
      const handoffDir = join(dir, ".maestro", "handoffs");
      await mkdir(handoffDir, { recursive: true });
      await writeFile(join(handoffDir, `${id}.json`), JSON.stringify({
        id,
        version: "5.2",
        timestamp: "2026-04-09T00:00:00.000Z",
        status: "pending",
        agent: "codex",
        sessionId: "legacy-v52",
        slots: {
          sessionCore: "legacy_record",
          causalDrivers: ["upgrade_path"],
          divergences: [],
          keyDecisions: ["keep_pickup_safe"],
          signalDelta: ["handoffs_1~2"],
          artifacts: ["branch_main"],
          executionState: "legacy_tmpdir",
          boundaryState: [],
          stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
          nextAction: "review_upgrade",
          cs: { work: 0.8 },
          summary: "Legacy_record-normalized-low_risk",
        },
        uki: "SESSION_CORE-legacy_record|CAUSAL_DRIVERS-upgrade_path|DIVERGENCES-NONE|KEY_DECISIONS-keep_pickup_safe|SIGNAL_DELTA-handoffs_1~2|ARTIFACTS-branch_main|EXECUTION_STATE-legacy_tmpdir|BOUNDARY_STATE-NONE|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION|NEXT_ACTION-review_upgrade|CS-work_0.8|SUMMARY-Legacy_record-normalized-low_risk",
      }, null, 2));

      const retrieved = await store.get(id);

      expect(retrieved?.version).toBe("5.2");
      expect(retrieved?.slots.decisionBasis).toEqual([]);
      expect(retrieved?.slots.validationState).toEqual([]);
      expect(retrieved?.slots.nextAction).toBe("review_upgrade");
      expect(retrieved?.slots.artifacts).toEqual(["branch_main"]);
    });

  it("returns undefined for missing id", async () => {
    const result = await store.get("9999-12-31-999");
    expect(result).toBeUndefined();
  });

  it("rejects handoff ids with path traversal segments", async () => {
    await expect(store.get("../package")).rejects.toThrow(MaestroError);
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

  it("skips malformed persisted records during list and latest-pending scans", async () => {
    const created = await store.create(SAMPLE_INPUT);
    const handoffDir = join(dir, ".maestro", "handoffs");
    await mkdir(handoffDir, { recursive: true });
    await writeFile(join(handoffDir, "2026-04-09-999.json"), '{"id":"2026-04-09-999","broken":true}');

    const listed = await store.list();
    expect(listed.map((handoff) => handoff.id)).toEqual([created.id]);

    const latest = await store.getLatestPending();
    expect(latest?.id).toBe(created.id);
  });

  it("lists legacy handoff directories alongside UKI records", async () => {
    const created = await store.create(SAMPLE_INPUT);
    const legacyDir = join(dir, ".maestro", "handoffs", "2026-04-08-001");
    await mkdir(legacyDir, { recursive: true });
    await writeFile(join(legacyDir, "handoff.json"), JSON.stringify({
      id: "2026-04-08-001",
      timestamp: "2026-04-08T00:00:00.000Z",
      message: "Legacy handoff",
      session: {
        agent: "claude-code",
        sessionId: "legacy-session",
        sourcePath: "/tmp/session.jsonl",
      },
      sitrep: "Legacy sitrep",
      quickstart: "Review the legacy work",
      git: {
        branch: "main",
        recentCommits: [],
        changedFiles: [],
        workingTreeClean: true,
        diffStat: "+0 -0",
      },
    }, null, 2));
    await writeFile(join(legacyDir, "envelope.json"), JSON.stringify({
      handoff: {
        id: "2026-04-08-001",
        timestamp: "2026-04-08T00:00:00.000Z",
        message: "Legacy handoff",
        session: {
          agent: "claude-code",
          sessionId: "legacy-session",
          sourcePath: "/tmp/session.jsonl",
        },
        sitrep: "Legacy sitrep",
        quickstart: "Review the legacy work",
        git: {
          branch: "main",
          recentCommits: [],
          changedFiles: [],
          workingTreeClean: true,
          diffStat: "+0 -0",
        },
      },
      status: "pending",
    }, null, 2));

    const listed = await store.list();

    expect(listed).toHaveLength(2);
    expect(listed.map((handoff) => handoff.id)).toEqual([created.id, "2026-04-08-001"]);
  });

  it("supports concurrent creates without reusing handoff ids", async () => {
    const created = await Promise.all(
      Array.from({ length: 12 }, () => store.create(SAMPLE_INPUT)),
    );

    expect(new Set(created.map((handoff) => handoff.id)).size).toBe(created.length);
    expect((await store.list()).length).toBe(created.length);
  });

  it("allows only one concurrent claim to transition a pending handoff", async () => {
    const created = await store.create(SAMPLE_INPUT);

    const [first, second] = await Promise.all([
      store.claimPending(created.id, "alpha"),
      store.claimPending(created.id, "beta"),
    ]);

    const claimed = [first, second].filter((handoff) => handoff !== undefined);
    expect(claimed).toHaveLength(1);
    const winner = claimed[0]?.pickedUpBy ?? "";
    expect(winner).not.toBe("");
    expect(["alpha", "beta"]).toContain(winner);

    const reread = await store.get(created.id);
    expect(reread?.status).toBe("picked-up");
    expect(reread?.pickedUpBy).toBe(winner);
  });

  it("recovers from a stale create lock file", async () => {
    const handoffDir = join(dir, ".maestro", "handoffs");
    const lockPath = join(handoffDir, ".create.lock");
    await mkdir(handoffDir, { recursive: true });
    await writeFile(lockPath, "");
    const staleTime = new Date(Date.now() - 60_000);
    await utimes(lockPath, staleTime, staleTime);

    const created = await store.create(SAMPLE_INPUT);

    expect(created.status).toBe("pending");
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
