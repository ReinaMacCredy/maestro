import { describe, expect, it } from "bun:test";
import { pickupHandoff, listHandoffs } from "../../../src/usecases/pickup-handoff.usecase.js";
import { MaestroError } from "../../../src/domain/errors.js";
import { mockHandoffStore } from "../../helpers/mocks.js";
import type { Handoff, HandoffEnvelope } from "../../../src/domain/types.js";

const makeEnvelope = (
  id: string,
  status: "pending" | "picked-up" | "completed" = "pending",
): HandoffEnvelope => ({
  handoff: {
    id,
    timestamp: "2026-03-28T12:00:00Z",
    message: "Test",
    session: {
      agent: "claude-code",
      sessionId: "sess-1",
      sourcePath: "/tmp/sessions/sess-1",
    },
    sitrep: "Done",
    quickstart: "Run tests",
    git: {
      branch: "main",
      recentCommits: [],
      changedFiles: [],
      workingTreeClean: true,
      diffStat: "+0 -0",
    },
  },
  status,
});

describe("pickupHandoff", () => {
  it("picks up the latest pending handoff", async () => {
    const store = mockHandoffStore([makeEnvelope("2026-03-28-001")]);
    const result = await pickupHandoff(store, { agent: "codex" });
    expect(result.handoff.id).toBe("2026-03-28-001");
    expect(result.status).toBe("picked-up");
    expect(result.pickedUpBy).toBe("codex");
  });

  it("picks up a specific handoff by ID", async () => {
    const store = mockHandoffStore([
      makeEnvelope("2026-03-28-001"),
      makeEnvelope("2026-03-28-002"),
    ]);
    const result = await pickupHandoff(store, {
      id: "2026-03-28-001",
      agent: "gemini",
    });
    expect(result.handoff.id).toBe("2026-03-28-001");
  });

  it("throws when no pending handoffs exist", async () => {
    const store = mockHandoffStore();
    try {
      await pickupHandoff(store, { agent: "codex" });
      expect(true).toBe(false);
    } catch (err) {
      expect(err).toBeInstanceOf(MaestroError);
      expect((err as MaestroError).message).toContain("No pending");
    }
  });

  it("throws when specified ID not found", async () => {
    const store = mockHandoffStore();
    try {
      await pickupHandoff(store, { id: "nonexistent", agent: "codex" });
      expect(true).toBe(false);
    } catch (err) {
      expect(err).toBeInstanceOf(MaestroError);
    }
  });
});

describe("listHandoffs", () => {
  it("returns all handoffs", async () => {
    const store = mockHandoffStore([
      makeEnvelope("2026-03-28-001"),
      makeEnvelope("2026-03-28-002", "picked-up"),
    ]);
    const result = await listHandoffs(store);
    expect(result).toHaveLength(2);
  });

  it("returns empty array when none exist", async () => {
    const store = mockHandoffStore();
    const result = await listHandoffs(store);
    expect(result).toHaveLength(0);
  });
});
