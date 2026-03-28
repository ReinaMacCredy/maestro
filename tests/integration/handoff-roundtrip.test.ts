import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsHandoffStoreAdapter } from "../../src/adapters/handoff-store.adapter.js";
import { createHandoff } from "../../src/usecases/create-handoff.usecase.js";
import { pickupHandoff } from "../../src/usecases/pickup-handoff.usecase.js";
import { mockGit, mockConfig, mockSessionDetect } from "../helpers/mocks.js";

let tmpDir: string;
let store: FsHandoffStoreAdapter;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-roundtrip-"));
  await mkdir(join(tmpDir, ".maestro", "handoffs"), { recursive: true });
  store = new FsHandoffStoreAdapter(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("Handoff roundtrip", () => {
  it("create -> list -> pickup -> verify state transition", async () => {
    // Create
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "Auth module complete. JWT chosen over sessions.",
      quickstart: "Run: bun test test/auth",
      message: "Auth handoff",
      session: "test-session-123",
      dir: tmpDir,
    });

    expect(handoff.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
    expect(handoff.sitrep).toContain("Auth module");

    // List -- should have one pending
    const pending = await store.list({ status: "pending" });
    expect(pending).toHaveLength(1);
    expect(pending[0]!.handoff.id).toBe(handoff.id);

    // Pickup
    const envelope = await pickupHandoff(store, { agent: "codex" });
    expect(envelope.status).toBe("picked-up");
    expect(envelope.pickedUpBy).toBe("codex");

    // Verify state transition
    const afterPickup = await store.list({ status: "pending" });
    expect(afterPickup).toHaveLength(0);

    const pickedUp = await store.list({ status: "picked-up" });
    expect(pickedUp).toHaveLength(1);
  });

  it("creates multiple handoffs with sequential IDs", async () => {
    const git = mockGit();
    const session = mockSessionDetect();

    const h1 = await createHandoff(git, session, { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "First",
      quickstart: "Step 1",
      session: "test-session-123",
      dir: tmpDir,
    });

    const h2 = await createHandoff(git, session, { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "Second",
      quickstart: "Step 2",
      session: "test-session-123",
      dir: tmpDir,
    });

    // Same date, sequential numbers
    const date1 = h1.id.slice(0, 10);
    const date2 = h2.id.slice(0, 10);
    expect(date1).toBe(date2);

    const seq1 = parseInt(h1.id.slice(11), 10);
    const seq2 = parseInt(h2.id.slice(11), 10);
    expect(seq2).toBe(seq1 + 1);
  });

  it("instructions survive create -> pickup roundtrip", async () => {
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "Auth done",
      quickstart: "Run tests",
      instructions: "Deploy to staging before PR review",
      session: "test-session-123",
      dir: tmpDir,
    });

    expect(handoff.instructions).toBe("Deploy to staging before PR review");

    const envelope = await pickupHandoff(store, { agent: "codex" });
    expect(envelope.handoff.instructions).toBe("Deploy to staging before PR review");
  });

  it("handoff without instructions has undefined field", async () => {
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "Auth done",
      quickstart: "Run tests",
      session: "test-session-123",
      dir: tmpDir,
    });

    expect(handoff.instructions).toBeUndefined();

    const envelope = await pickupHandoff(store, { agent: "codex" });
    expect(envelope.handoff.instructions).toBeUndefined();
  });
});
