import { describe, expect, it } from "bun:test";
import { ZodError } from "zod";
import { validateHandoff, validateEnvelope } from "../../../src/domain/validators.js";

const validGit = {
  branch: "main",
  recentCommits: ["abc1234 feat: add auth"],
  changedFiles: ["src/auth.ts"],
  workingTreeClean: true,
  diffStat: "+42 -17",
};

const validSession = {
  agent: "claude-code",
  sessionId: "abc-123",
  sourcePath: "/home/user/.claude/sessions/abc-123",
  cassIndexed: true,
};

const validHandoff = {
  id: "2026-03-28-001",
  timestamp: "2026-03-28T12:00:00Z",
  message: "Auth module complete",
  session: validSession,
  sitrep: "Implemented auth adapter",
  quickstart: "Run: bun test",
  git: validGit,
};

describe("validateHandoff", () => {
  it("accepts a valid handoff", () => {
    const result = validateHandoff(validHandoff);
    expect(result.id).toBe("2026-03-28-001");
    expect(result.session.agent).toBe("claude-code");
  });

  it("accepts handoff with optional plan", () => {
    const withPlan = {
      ...validHandoff,
      plan: {
        tasks: [{ id: "1", description: "Do X", status: "done", dependsOn: [] }],
        completed: 1,
        remaining: 0,
      },
    };
    const result = validateHandoff(withPlan);
    expect(result.plan?.tasks).toHaveLength(1);
  });

  it("rejects invalid ID format", () => {
    expect(() =>
      validateHandoff({ ...validHandoff, id: "bad-id" }),
    ).toThrow(ZodError);
  });

  it("rejects missing required fields", () => {
    const { sitrep: _, ...noSitrep } = validHandoff;
    expect(() => validateHandoff(noSitrep)).toThrow(ZodError);
  });

  it("rejects empty message", () => {
    expect(() =>
      validateHandoff({ ...validHandoff, message: "" }),
    ).toThrow(ZodError);
  });

  it("rejects invalid timestamp", () => {
    expect(() =>
      validateHandoff({ ...validHandoff, timestamp: "not-a-date" }),
    ).toThrow(ZodError);
  });

  it("rejects invalid plan task status", () => {
    const badPlan = {
      ...validHandoff,
      plan: {
        tasks: [{ id: "1", description: "X", status: "invalid", dependsOn: [] }],
        completed: 0,
        remaining: 1,
      },
    };
    expect(() => validateHandoff(badPlan)).toThrow(ZodError);
  });
});

describe("validateEnvelope", () => {
  it("accepts a valid envelope", () => {
    const envelope = { handoff: validHandoff, status: "pending" };
    const result = validateEnvelope(envelope);
    expect(result.status).toBe("pending");
  });

  it("accepts envelope with pickup metadata", () => {
    const envelope = {
      handoff: validHandoff,
      status: "picked-up",
      pickedUpAt: "2026-03-28T13:00:00Z",
      pickedUpBy: "codex",
    };
    const result = validateEnvelope(envelope);
    expect(result.pickedUpBy).toBe("codex");
  });

  it("rejects invalid status", () => {
    expect(() =>
      validateEnvelope({ handoff: validHandoff, status: "invalid" }),
    ).toThrow(ZodError);
  });
});
