/**
 * Unit tests for the three UKI handoff usecases: create, pickup, list.
 *
 * Uses a real FsHandoffStoreAdapter against a tmpdir so the end-to-end
 * compression path is exercised, and a stub SessionDetectPort that
 * returns fixed identity data.
 */
import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter } from "../../../src/adapters/handoff-store.adapter.js";
import type { SessionDetectPort } from "../../../src/ports/session-detect.port.js";
import type { AgentSession } from "../../../src/domain/types.js";
import { createUkiHandoff } from "../../../src/usecases/create-uki-handoff.usecase.js";
import { pickupUkiHandoff } from "../../../src/usecases/pickup-uki-handoff.usecase.js";
import { listUkiHandoffs } from "../../../src/usecases/list-uki-handoffs.usecase.js";
import { MaestroError } from "../../../src/domain/errors.js";
import type { UkiSlots } from "../../../src/lib/uki-format.js";

const SAMPLE_SLOTS: UkiSlots = {
  sessionCore: "usecase_test_sample",
  causalDrivers: ["unit_test_ran"],
  divergences: [],
  keyDecisions: ["proceed_with_stub_adapter"],
  signalDelta: ["usecase_tests_0~3"],
  artifacts: ["branch_feat_missionControl"],
  executionState: "isolated_tmpdir_store",
  boundaryState: [],
  stanceCollapse: "NONE_DETECTED_LOW_FRICTION",
  nextAction: "run_full_suite",
  cs: { work: 0.95, summary: 0.9 },
  summary: "Usecase_tests-roundtripped-low_risk",
};

class StubSessionDetect implements SessionDetectPort {
  constructor(private readonly session: AgentSession | undefined) {}
  async detect(_cwd: string): Promise<AgentSession | undefined> {
    return this.session;
  }
}

const CLAUDE_SESSION: AgentSession = {
  agent: "claude-code",
  sessionId: "abc-123",
  sourcePath: "/tmp/fake.jsonl",
};

describe("createUkiHandoff usecase", () => {
  let dir: string;
  let store: FsHandoffStoreAdapter;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-uki-usecase-"));
    store = new FsHandoffStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("auto-fills agent and sessionId from session-detect", async () => {
    const sessionDetect = new StubSessionDetect(CLAUDE_SESSION);
    const handoff = await createUkiHandoff(store, sessionDetect, dir, {
      slots: SAMPLE_SLOTS,
    });
    expect(handoff.agent).toBe("claude-code");
    expect(handoff.sessionId).toBe("abc-123");
    expect(handoff.status).toBe("pending");
  });

  it("uses explicit agent/sessionId overrides over detection", async () => {
    const sessionDetect = new StubSessionDetect(CLAUDE_SESSION);
    const handoff = await createUkiHandoff(store, sessionDetect, dir, {
      slots: SAMPLE_SLOTS,
      agent: "codex",
      sessionId: "override-xyz",
    });
    expect(handoff.agent).toBe("codex");
    expect(handoff.sessionId).toBe("override-xyz");
  });

  it("falls back to defaults when session-detect returns nothing", async () => {
    const sessionDetect = new StubSessionDetect(undefined);
    const handoff = await createUkiHandoff(store, sessionDetect, dir, {
      slots: SAMPLE_SLOTS,
    });
    expect(handoff.agent).toBe("unknown");
    expect(handoff.sessionId).toBe("none");
  });

  it("persists the cached UKI string on the record", async () => {
    const sessionDetect = new StubSessionDetect(CLAUDE_SESSION);
    const handoff = await createUkiHandoff(store, sessionDetect, dir, {
      slots: SAMPLE_SLOTS,
    });
    expect(handoff.uki.length).toBeGreaterThan(0);
    expect(handoff.uki.startsWith("SESSION_CORE-usecase_test_sample")).toBe(true);
  });
});

describe("pickupUkiHandoff usecase", () => {
  let dir: string;
  let store: FsHandoffStoreAdapter;
  const sessionDetect = new StubSessionDetect(CLAUDE_SESSION);

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-uki-pickup-"));
    store = new FsHandoffStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("returns latest pending when no id supplied", async () => {
    const a = await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    const b = await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    const picked = await pickupUkiHandoff(store);
    expect(picked.id).toBe(b.id);
    expect(a.id).not.toBe(b.id);
  });

  it("returns the specific handoff when id supplied", async () => {
    const a = await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    const picked = await pickupUkiHandoff(store, { id: a.id });
    expect(picked.id).toBe(a.id);
  });

  it("claim transitions pending -> picked-up", async () => {
    const a = await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    const picked = await pickupUkiHandoff(store, {
      id: a.id,
      claim: true,
      pickedUpBy: "codex",
    });
    expect(picked.status).toBe("picked-up");
    expect(picked.pickedUpBy).toBe("codex");

    const reread = await store.get(a.id);
    expect(reread?.status).toBe("picked-up");
  });

  it("claim is a no-op on an already-picked-up handoff", async () => {
    const a = await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    await store.updateStatus(a.id, "picked-up", { pickedUpBy: "first" });
    const picked = await pickupUkiHandoff(store, {
      id: a.id,
      claim: true,
      pickedUpBy: "second",
    });
    // Status is still picked-up, but pickedUpBy is preserved from the
    // original claim (claim is a no-op here).
    expect(picked.status).toBe("picked-up");
    expect(picked.pickedUpBy).toBe("first");
  });

  it("claim without an id still returns a claimed handoff under contention", async () => {
    await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });

    const claimed = await Promise.all([
      pickupUkiHandoff(store, { claim: true, pickedUpBy: "alpha" }),
      pickupUkiHandoff(store, { claim: true, pickedUpBy: "beta" }),
    ]);

    expect(claimed).toHaveLength(2);
    expect(claimed.every((handoff) => handoff.status === "picked-up")).toBe(true);
  });

  it("throws MaestroError with hints when nothing pending", async () => {
    await expect(pickupUkiHandoff(store)).rejects.toThrow(MaestroError);
  });

  it("throws MaestroError with hints when id not found", async () => {
    await expect(pickupUkiHandoff(store, { id: "9999-12-31-999" })).rejects.toThrow(
      MaestroError,
    );
  });
});

describe("listUkiHandoffs usecase", () => {
  let dir: string;
  let store: FsHandoffStoreAdapter;
  const sessionDetect = new StubSessionDetect(CLAUDE_SESSION);

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-uki-list-"));
    store = new FsHandoffStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("returns all handoffs when no filter", async () => {
    await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    const all = await listUkiHandoffs(store);
    expect(all.length).toBe(3);
  });

  it("filters by status", async () => {
    const a = await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    await createUkiHandoff(store, sessionDetect, dir, { slots: SAMPLE_SLOTS });
    await store.updateStatus(a.id, "picked-up");
    const pending = await listUkiHandoffs(store, { status: "pending" });
    expect(pending.length).toBe(1);
    const pickedUp = await listUkiHandoffs(store, { status: "picked-up" });
    expect(pickedUp.length).toBe(1);
    expect(pickedUp[0]!.id).toBe(a.id);
  });

  it("returns empty array when store is empty", async () => {
    const all = await listUkiHandoffs(store);
    expect(all).toEqual([]);
  });
});
