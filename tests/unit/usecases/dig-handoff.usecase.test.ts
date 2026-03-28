import { describe, expect, it } from "bun:test";
import { digHandoff } from "../../../src/usecases/dig-handoff.usecase.js";
import { MaestroError } from "../../../src/domain/errors.js";
import { mockHandoffStore, mockCass } from "../../helpers/mocks.js";
import type { HandoffEnvelope } from "../../../src/domain/types.js";

const makeEnvelope = (): HandoffEnvelope => ({
  handoff: {
    id: "2026-03-28-001",
    timestamp: "2026-03-28T12:00:00Z",
    message: "Test",
    session: {
      agent: "claude-code",
      sessionId: "sess-1",
      sourcePath: "/tmp/sessions/sess-1",
      cassIndexed: true,
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
  status: "picked-up",
});

describe("digHandoff", () => {
  it("searches CASS with handoff context", async () => {
    let searchQuery = "";
    let searchOpts: Record<string, unknown> = {};
    const cass = mockCass({
      search: async (q, opts) => {
        searchQuery = q;
        searchOpts = opts;
        return { query: q, count: 0, totalMatches: 0, hits: [] };
      },
    });
    const store = mockHandoffStore([makeEnvelope()]);

    await digHandoff(store, cass, "token refresh", { id: "2026-03-28-001" });
    expect(searchQuery).toBe("token refresh");
    expect(searchOpts.agent).toBe("claude_code");
  });

  it("throws when CASS unavailable", async () => {
    const cass = mockCass({ isAvailable: async () => false });
    const store = mockHandoffStore([makeEnvelope()]);

    try {
      await digHandoff(store, cass, "test", {});
      expect(true).toBe(false);
    } catch (err) {
      expect(err).toBeInstanceOf(MaestroError);
      expect((err as MaestroError).message).toContain("not available");
    }
  });

  it("throws when no handoffs exist", async () => {
    const cass = mockCass();
    const store = mockHandoffStore();

    try {
      await digHandoff(store, cass, "test", {});
      expect(true).toBe(false);
    } catch (err) {
      expect(err).toBeInstanceOf(MaestroError);
    }
  });
});
