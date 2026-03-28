import { describe, expect, it } from "bun:test";
import { createHandoff } from "../../../src/usecases/create-handoff.usecase.js";
import { MaestroError } from "../../../src/domain/errors.js";
import {
  mockGit,
  mockSessionDetect,
  mockHandoffStore,
} from "../../helpers/mocks.js";

describe("createHandoff", () => {
  it("creates a handoff with git state and session", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "Auth done",
      quickstart: "Run tests",
      dir: process.cwd(),
    });

    expect(handoff.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
    expect(handoff.sitrep).toBe("Auth done");
    expect(handoff.quickstart).toBe("Run tests");
    expect(handoff.git.branch).toBe("main");
    expect(handoff.session.agent).toBe("claude-code");
  });

  it("throws when not in a git repo", async () => {
    const git = mockGit({ isRepo: async () => false });
    const store = mockHandoffStore();

    try {
      await createHandoff(git, mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
        plan: false,
        sitrep: "test",
        quickstart: "test",
        dir: "/tmp",
      });
      expect(true).toBe(false);
    } catch (err) {
      expect(err).toBeInstanceOf(MaestroError);
    }
  });

  it("continues when session detection fails", async () => {
    const sessionDetect = { detect: async () => undefined, resolve: async () => undefined };
    const store = mockHandoffStore();

    const handoff = await createHandoff(mockGit(), sessionDetect, { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "test",
      quickstart: "test",
      dir: process.cwd(),
    });

    expect(handoff.session.agent).toBe("unknown");
  });

  it("uses message when provided", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "Full sitrep",
      quickstart: "Steps",
      message: "Short msg",
      dir: process.cwd(),
    });

    expect(handoff.message).toBe("Short msg");
  });

  it("uses task for message when no message provided", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      task: "implement note command",
      dir: process.cwd(),
    });

    expect(handoff.message).toBe("implement note command");
  });

  it("auto-generates sitrep from git state when not provided", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      dir: process.cwd(),
    });

    expect(handoff.sitrep).toContain("Branch: main");
    expect(handoff.sitrep).toContain("abc1234 feat: test");
    expect(handoff.quickstart).toBe("See handoff briefing for orientation.");
  });

  it("auto-generates message from branch when nothing provided", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      dir: process.cwd(),
    });

    expect(handoff.message).toContain("main");
  });

  it("passes instructions to handoff when provided", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "Auth done",
      quickstart: "Run tests",
      instructions: "Deploy to staging first",
      dir: process.cwd(),
    });

    expect(handoff.instructions).toBe("Deploy to staging first");
  });

  it("omits instructions when not provided", async () => {
    const store = mockHandoffStore();
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: "Auth done",
      quickstart: "Run tests",
      dir: process.cwd(),
    });

    expect(handoff.instructions).toBeUndefined();
  });

  it("truncates sitrep for auto-message", async () => {
    const store = mockHandoffStore();
    const longSitrep = "A".repeat(200);
    const handoff = await createHandoff(mockGit(), mockSessionDetect(), { sessionDetection: { enabled: true, agents: ["claude-code"] } }, store, {
      plan: false,
      sitrep: longSitrep,
      quickstart: "Steps",
      dir: process.cwd(),
    });

    expect(handoff.message.length).toBeLessThanOrEqual(80);
  });
});
