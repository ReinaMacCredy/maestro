import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter } from "@/features/handoff";
import type { AgentSession } from "@/domain/types.js";
import type { ExecuteUkiHandoffContent } from "@/features/handoff";
import { MaestroError } from "@/domain/errors.js";
import type { SessionDetectPort } from "@/features/session";
import { createUkiHandoff } from "@/features/handoff";
import { listUkiHandoffs } from "@/features/handoff";
import { pickupUkiHandoff } from "@/features/handoff";

const SAMPLE_CONTENT: ExecuteUkiHandoffContent = {
  mode: "execute",
  currentState: "execute_in_progress",
  sessionCore: "usecase_test_sample",
  decisions: ["proceed_with_stub_adapter"],
  artifacts: ["branch_feat_handoff_rebuild", "file_src_lib_uki_format_ts"],
  readMore: ["file_src_lib_uki_format_ts"],
  nextAction: "run_full_suite",
  summary: "Usecase_tests-roundtripped-low_risk",
  maestroRefs: {},
  cs: { work: 0.95, summary: 0.9 },
  signalDelta: ["usecase_tests_0_3"],
  boundaryState: [],
  risks: [],
  causalDrivers: ["unit_test_ran"],
  divergences: [],
  touchedFiles: ["file_src_lib_uki_format_ts"],
  completedWork: ["store_roundtrip_verified"],
  validation: ["unit_green"],
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

describe("UKI handoff usecases", () => {
  let dir: string;
  let store: FsHandoffStoreAdapter;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-uki-usecase-"));
    store = new FsHandoffStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("create auto-fills agent and session id from session detection", async () => {
    const handoff = await createUkiHandoff(
      store,
      new StubSessionDetect(CLAUDE_SESSION),
      dir,
      { content: SAMPLE_CONTENT },
    );

    expect(handoff.agent).toBe("claude-code");
    expect(handoff.sessionId).toBe("abc-123");
    expect(handoff.content).toEqual(SAMPLE_CONTENT);
  });

  it("create respects explicit agent and session overrides", async () => {
    const handoff = await createUkiHandoff(
      store,
      new StubSessionDetect(CLAUDE_SESSION),
      dir,
      {
        content: SAMPLE_CONTENT,
        agent: "codex",
        sessionId: "override-xyz",
      },
    );

    expect(handoff.agent).toBe("codex");
    expect(handoff.sessionId).toBe("override-xyz");
  });

  it("pickup returns newest pending handoff and supports claim", async () => {
    const a = await createUkiHandoff(store, new StubSessionDetect(CLAUDE_SESSION), dir, {
      content: SAMPLE_CONTENT,
    });
    const b = await createUkiHandoff(store, new StubSessionDetect(CLAUDE_SESSION), dir, {
      content: SAMPLE_CONTENT,
    });

    expect((await pickupUkiHandoff(store)).id).toBe(b.id);

    const claimed = await pickupUkiHandoff(store, {
      id: a.id,
      claim: true,
      pickedUpBy: "codex",
    });
    expect(claimed.status).toBe("picked-up");
    expect(claimed.pickedUpBy).toBe("codex");
  });

  it("list filters by status", async () => {
    const created = await createUkiHandoff(store, new StubSessionDetect(CLAUDE_SESSION), dir, {
      content: SAMPLE_CONTENT,
    });
    await createUkiHandoff(store, new StubSessionDetect(CLAUDE_SESSION), dir, {
      content: SAMPLE_CONTENT,
    });
    await store.updateStatus(created.id, "picked-up");

    expect((await listUkiHandoffs(store, { status: "pending" })).length).toBe(1);
    expect((await listUkiHandoffs(store, { status: "picked-up" })).length).toBe(1);
  });

  it("raises MaestroError when pickup cannot find work", async () => {
    await expect(pickupUkiHandoff(store)).rejects.toThrow(MaestroError);
    await expect(pickupUkiHandoff(store, { id: "9999-12-31-999" })).rejects.toThrow(MaestroError);
  });
});
