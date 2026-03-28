import { describe, expect, it } from "bun:test";
import { createHandoff } from "../../../src/usecases/create-handoff.usecase.js";
import { MaestroError } from "../../../src/domain/errors.js";
import {
  mockGit,
  mockCass,
  mockSessionDetect,
  mockHandoffStore,
} from "../../helpers/mocks.js";

describe("createHandoff", () => {
  it("creates a handoff with git state and session", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(
      mockGit(),
      mockCass(),
      mockSessionDetect(),
      store,
      {
        plan: false,
        sitrep: "Auth done",
        quickstart: "Run tests",
        dir: process.cwd(),
      },
    );

    expect(handoff.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
    expect(handoff.sitrep).toBe("Auth done");
    expect(handoff.quickstart).toBe("Run tests");
    expect(handoff.git.branch).toBe("main");
    expect(handoff.session.agent).toBe("claude-code");
    expect(handoff.session.cassIndexed).toBe(true);
  });

  it("throws when not in a git repo", async () => {
    const git = mockGit({ isRepo: async () => false });
    const store = mockHandoffStore();

    try {
      await createHandoff(git, mockCass(), mockSessionDetect(), store, {
        plan: false,
        sitrep: "test",
        quickstart: "test",
        dir: "/tmp",
      });
      expect(true).toBe(false); // Should not reach
    } catch (err) {
      expect(err).toBeInstanceOf(MaestroError);
    }
  });

  it("continues when CASS is unavailable", async () => {
    const cass = mockCass({ isAvailable: async () => false });
    const store = mockHandoffStore();

    const handoff = await createHandoff(
      mockGit(),
      cass,
      mockSessionDetect(),
      store,
      {
        plan: false,
        sitrep: "test",
        quickstart: "test",
        dir: process.cwd(),
      },
    );

    expect(handoff.session.cassIndexed).toBe(false);
  });

  it("continues when session detection fails", async () => {
    const sessionDetect = { detect: async () => undefined };
    const store = mockHandoffStore();

    const handoff = await createHandoff(
      mockGit(),
      mockCass(),
      sessionDetect,
      store,
      {
        plan: false,
        sitrep: "test",
        quickstart: "test",
        dir: process.cwd(),
      },
    );

    expect(handoff.session.agent).toBe("unknown");
  });

  it("uses message when provided", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(
      mockGit(),
      mockCass(),
      mockSessionDetect(),
      store,
      {
        plan: false,
        sitrep: "Full sitrep",
        quickstart: "Steps",
        message: "Short msg",
        dir: process.cwd(),
      },
    );

    expect(handoff.message).toBe("Short msg");
  });

  it("truncates sitrep for auto-message", async () => {
    const store = mockHandoffStore();
    const longSitrep = "A".repeat(200);
    const handoff = await createHandoff(
      mockGit(),
      mockCass(),
      mockSessionDetect(),
      store,
      {
        plan: false,
        sitrep: longSitrep,
        quickstart: "Steps",
        dir: process.cwd(),
      },
    );

    expect(handoff.message.length).toBeLessThanOrEqual(80);
  });
});
