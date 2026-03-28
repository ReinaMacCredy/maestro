import { describe, expect, it } from "bun:test";
import { reportHandoff } from "../../../src/usecases/report-handoff.usecase.js";
import { MaestroError } from "../../../src/domain/errors.js";
import { mockHandoffStore } from "../../helpers/mocks.js";
import type { HandoffEnvelope } from "../../../src/domain/types.js";

const makeEnvelope = (
  id: string,
  status: "pending" | "picked-up" | "completed" = "picked-up",
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
  ...(status === "picked-up" && {
    pickedUpBy: "codex",
    pickedUpAt: "2026-03-28T12:01:00Z",
  }),
});

describe("reportHandoff", () => {
  it("marks latest picked-up handoff as completed with report", async () => {
    const store = mockHandoffStore([makeEnvelope("2026-03-28-001")]);
    const result = await reportHandoff(store, { content: "All done" });
    expect(result.status).toBe("completed");
    expect(result.report).toBe("All done");
    expect(result.completedAt).toBeDefined();
  });

  it("reports on a specific handoff by ID", async () => {
    const store = mockHandoffStore([
      makeEnvelope("2026-03-28-001"),
      makeEnvelope("2026-03-28-002"),
    ]);
    const result = await reportHandoff(store, {
      id: "2026-03-28-002",
      content: "Fixed the bug",
    });
    expect(result.handoff.id).toBe("2026-03-28-002");
    expect(result.status).toBe("completed");
    expect(result.report).toBe("Fixed the bug");
  });

  it("throws when no picked-up handoffs exist", async () => {
    const store = mockHandoffStore([makeEnvelope("2026-03-28-001", "pending")]);
    try {
      await reportHandoff(store, { content: "done" });
      expect(true).toBe(false);
    } catch (err) {
      expect(err).toBeInstanceOf(MaestroError);
      expect((err as MaestroError).message).toContain("No picked-up");
    }
  });

  it("throws when specified ID not found", async () => {
    const store = mockHandoffStore();
    try {
      await reportHandoff(store, { id: "nonexistent", content: "done" });
      expect(true).toBe(false);
    } catch (err) {
      expect(err).toBeInstanceOf(MaestroError);
      expect((err as MaestroError).message).toContain("not found");
    }
  });
});
