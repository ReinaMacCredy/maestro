import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { access, mkdir, mkdtemp, rm, utimes, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter } from "../../../src/adapters/handoff-store.adapter.js";
import { MaestroError } from "../../../src/domain/errors.js";
import type { CreateUkiHandoffInput, ExecuteUkiHandoffContent } from "../../../src/domain/uki-types.js";
import { compressUki, parseUki } from "../../../src/lib/uki-format.js";
import { LEGACY_V53_UKI } from "../../helpers/uki-fixtures.js";

const SAMPLE_CONTENT: ExecuteUkiHandoffContent = {
  mode: "execute",
  currentState: "execute_in_progress",
  sessionCore: "adapter_test_sample",
  decisions: ["proceed_with_persistence"],
  artifacts: ["branch_feat_handoff_rebuild", "file_src_lib_uki_format_ts"],
  readMore: ["file_src_lib_uki_format_ts"],
  nextAction: "verify_roundtrip",
  summary: "Adapter_test-roundtrip_verified-low_risk",
  maestroRefs: {
    missionId: "2026_04_09_001",
  },
  cs: { work: 0.9 },
  signalDelta: ["handoffs_0_1"],
  boundaryState: [],
  risks: [],
  causalDrivers: ["unit_test_ran"],
  divergences: [],
  touchedFiles: ["file_src_lib_uki_format_ts"],
  completedWork: ["structured_payload_persisted"],
  validation: ["unit_green"],
};

const SAMPLE_INPUT: CreateUkiHandoffInput = {
  content: SAMPLE_CONTENT,
  agent: "codex",
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

  it("creates a handoff with generated id and cached UKI", async () => {
    const created = await store.create(SAMPLE_INPUT);
    expect(created.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
    expect(created.status).toBe("pending");
    expect(created.version).toBe("5.4");
    expect(created.content).toEqual(SAMPLE_CONTENT);
    expect(parseUki(created.uki)).toEqual(SAMPLE_CONTENT);
  });

  it("persists to .maestro/handoffs/<id>.json", async () => {
    const created = await store.create(SAMPLE_INPUT);
    await access(join(dir, ".maestro", "handoffs", `${created.id}.json`));
  });

  it("lists newest-first and filters by status", async () => {
    const a = await store.create(SAMPLE_INPUT);
    const b = await store.create(SAMPLE_INPUT);

    expect((await store.list()).map((handoff) => handoff.id)).toEqual([b.id, a.id]);

    await store.updateStatus(a.id, "picked-up", { pickedUpBy: "codex" });
    const pending = await store.list({ status: "pending" });
    const pickedUp = await store.list({ status: "picked-up" });

    expect(pending).toHaveLength(1);
    expect(pending[0]?.id).toBe(b.id);
    expect(pickedUp).toHaveLength(1);
    expect(pickedUp[0]?.id).toBe(a.id);
  });

  it("claims and completes handoffs while preserving metadata", async () => {
    const created = await store.create(SAMPLE_INPUT);

    const claimed = await store.claimPending(created.id, "codex-reviewer");
    expect(claimed?.status).toBe("picked-up");
    expect(claimed?.pickedUpBy).toBe("codex-reviewer");

    const completed = await store.updateStatus(created.id, "completed", {
      report: "verified",
    });
    expect(completed?.status).toBe("completed");
    expect(completed?.report).toBe("verified");
    expect(completed?.pickedUpBy).toBe("codex-reviewer");
  });

  it("normalizes persisted v5.2/v5.3 slot records into the new content shape", async () => {
    const id = "2026-04-09-123";
    const handoffDir = join(dir, ".maestro", "handoffs");
    await mkdir(handoffDir, { recursive: true });
    await writeFile(join(handoffDir, `${id}.json`), JSON.stringify({
      id,
      version: "5.3",
      timestamp: "2026-04-09T00:00:00.000Z",
      status: "pending",
      agent: "codex",
      sessionId: "legacy-v53",
      slots: {
        sessionCore: "legacy_record",
        causalDrivers: ["upgrade_path"],
        divergences: [],
        keyDecisions: ["keep_pickup_safe"],
        decisionBasis: ["safe_upgrade_path"],
        signalDelta: ["handoffs_1_2"],
        validationState: ["unit_green"],
        artifacts: ["branch_main", "file_src_lib_uki_format_ts"],
        executionState: "legacy_tmpdir",
        boundaryState: [],
        stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
        nextAction: "review_upgrade",
        cs: { work: 0.8 },
        summary: "Legacy_record-normalized-low_risk",
      },
      uki: "SESSION_CORE-legacy_record|CAUSAL_DRIVERS-upgrade_path|DIVERGENCES-NONE|KEY_DECISIONS-keep_pickup_safe|DECISION_BASIS-safe_upgrade_path|SIGNAL_DELTA-handoffs_1_2|VALIDATION_STATE-unit_green|EXECUTION_STATE-legacy_tmpdir|BOUNDARY_STATE-NONE|NEXT_ACTION-review_upgrade|ARTIFACTS-branch_main-file_src_lib_uki_format_ts|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION|CS-work_0.8|SUMMARY-Legacy_record-normalized-low_risk",
    }, null, 2));

    const retrieved = await store.get(id);

    expect(retrieved?.version).toBe("5.3");
    expect(retrieved?.content.mode).toBe("execute");
    expect(retrieved?.content.readMore).toEqual(["file_src_lib_uki_format_ts"]);
    expect(retrieved?.content.validation).toEqual(["unit_green"]);
    expect(retrieved?.content.decisions).toEqual(["keep_pickup_safe", "safe_upgrade_path"]);
  });

  it("canonicalizes persisted legacy uki strings from normalized content", async () => {
    const id = "2026-04-09-124";
    const handoffDir = join(dir, ".maestro", "handoffs");
    await mkdir(handoffDir, { recursive: true });
    await writeFile(join(handoffDir, `${id}.json`), JSON.stringify({
      id,
      version: "5.3",
      timestamp: "2026-04-09T00:00:00.000Z",
      status: "pending",
      agent: "codex",
      sessionId: "legacy-v53",
      uki: LEGACY_V53_UKI,
    }, null, 2));

    const retrieved = await store.get(id);

    expect(retrieved).toBeDefined();
    expect(retrieved?.uki).toBe(compressUki(retrieved!.content));
    expect(retrieved?.uki.startsWith("MODE-execute|")).toBe(true);
  });

    it("canonicalizes tampered uki strings from structured content on read", async () => {
      const id = "2026-04-09-125";
      const handoffDir = join(dir, ".maestro", "handoffs");
      await mkdir(handoffDir, { recursive: true });
      await writeFile(join(handoffDir, `${id}.json`), JSON.stringify({
        id,
        version: "5.4",
        timestamp: "2026-04-09T00:00:00.000Z",
        status: "pending",
        agent: "codex",
        sessionId: "tampered",
        content: SAMPLE_CONTENT,
        uki: "\u001b[31mspoofed\u001b[0m",
      }, null, 2));

      const retrieved = await store.get(id);

      expect(retrieved).toBeDefined();
      expect(retrieved?.content).toEqual(SAMPLE_CONTENT);
      expect(retrieved?.uki).toBe(compressUki(SAMPLE_CONTENT));
    });

    it("rejects malformed canonical content records instead of coercing them", async () => {
      const id = "2026-04-09-126";
      const handoffDir = join(dir, ".maestro", "handoffs");
      await mkdir(handoffDir, { recursive: true });
      await writeFile(join(handoffDir, `${id}.json`), JSON.stringify({
        id,
        version: "5.4",
        timestamp: "2026-04-09T00:00:00.000Z",
        status: "pending",
        agent: "codex",
        sessionId: "broken-content",
        content: {
          ...SAMPLE_CONTENT,
          mode: "broken",
        },
        uki: compressUki(SAMPLE_CONTENT),
      }, null, 2));

      await expect(store.get(id)).rejects.toThrow(MaestroError);
    });

  it("keeps reading legacy envelope directories", async () => {
    const legacyDir = join(dir, ".maestro", "handoffs", "2026-04-08-001");
    await mkdir(legacyDir, { recursive: true });
    await writeFile(join(legacyDir, "envelope.json"), JSON.stringify({
        handoff: {
          id: "2026-04-08-001",
          timestamp: "2026-04-08T00:00:00.000Z",
        message: "Legacy handoff",
        sitrep: "Legacy sitrep",
        quickstart: "Review the legacy work",
        session: {
          agent: "claude-code",
          sessionId: "legacy-session",
        },
        git: {
          branch: "main",
          diffStat: "+0 -0",
        },
        },
        status: "pending",
    }, null, 2));

    const listed = await store.list();
    expect(listed).toHaveLength(1);
    expect(listed[0]?.content.mode).toBe("execute");
    expect(listed[0]?.agent).toBe("claude-code");
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
});
