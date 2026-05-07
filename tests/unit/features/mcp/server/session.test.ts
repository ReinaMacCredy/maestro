import { describe, expect, it } from "bun:test";
import { detectMcpSessionId } from "@/features/mcp/server/session.js";

describe("detectMcpSessionId", () => {
  it("prefers MAESTRO_SESSION_ID when set", () => {
    const id = detectMcpSessionId({
      MAESTRO_SESSION_ID: "ms-1",
      CLAUDECODE_SESSION_ID: "cc-2",
      CODEX_THREAD_ID: "cx-3",
    } as NodeJS.ProcessEnv);
    expect(id).toBe("ms-1");
  });

  it("falls back to CLAUDECODE_SESSION_ID when MAESTRO_SESSION_ID is absent", () => {
    const id = detectMcpSessionId({
      CLAUDECODE_SESSION_ID: "cc-2",
      CODEX_THREAD_ID: "cx-3",
    } as NodeJS.ProcessEnv);
    expect(id).toBe("cc-2");
  });

  it("falls back to CODEX_THREAD_ID when only it is set", () => {
    const id = detectMcpSessionId({
      CODEX_THREAD_ID: "cx-3",
    } as NodeJS.ProcessEnv);
    expect(id).toBe("cx-3");
  });

  it("falls back to user@host when no env hint is set", () => {
    const id = detectMcpSessionId({} as NodeJS.ProcessEnv);
    expect(id).toMatch(/.+@.+/);
  });

  it("treats empty strings as absent (falls through)", () => {
    const id = detectMcpSessionId({
      MAESTRO_SESSION_ID: "",
      CLAUDECODE_SESSION_ID: "cc-2",
    } as NodeJS.ProcessEnv);
    expect(id).toBe("cc-2");
  });
});
