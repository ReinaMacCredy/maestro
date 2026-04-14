import { describe, expect, it } from "bun:test";
import { detectSession } from "@/features/session";

describe("detectSession", () => {
  it("returns undefined when noSession is requested", async () => {
    let detectCalls = 0;

    const result = await detectSession(
      {
        detect: async () => {
          detectCalls += 1;
          return undefined;
        },
      },
      {
        cwd: "/tmp/project",
        noSession: true,
      },
    );

    expect(result).toBeUndefined();
    expect(detectCalls).toBe(0);
  });

  it("returns undefined when the adapter finds no session", async () => {
    const result = await detectSession(
      {
        detect: async (cwd) => {
          expect(cwd).toBe("/tmp/project");
          return undefined;
        },
      },
      { cwd: "/tmp/project" },
    );

    expect(result).toBeUndefined();
  });

  it("wraps the detected session in a result object", async () => {
    const session = {
      agent: "codex" as const,
      sessionId: "thread-123",
      sourcePath: "/tmp/source.jsonl",
      startedAt: 1234,
    };

    const result = await detectSession(
      {
        detect: async () => session,
      },
      { cwd: "/tmp/project" },
    );

    expect(result).toEqual({ session });
  });
});
