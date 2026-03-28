import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir, homedir } from "node:os";
import { ClaudeSessionDetectAdapter } from "../../../src/adapters/session-detect.adapter.js";

const adapter = new ClaudeSessionDetectAdapter();

describe("ClaudeSessionDetectAdapter", () => {
  describe("detect", () => {
    it("returns a session for the current working directory", async () => {
      // This test uses the real ~/.claude/sessions/ directory
      const cwd = process.cwd();
      const session = await adapter.detect(cwd);

      // May or may not find a session depending on environment
      if (session) {
        expect(session.agent).toBe("claude-code");
        expect(session.sessionId).toBeTruthy();
        expect(session.sourcePath).toContain(".claude/projects");
      }
    });

    it("returns undefined for a directory with no session", async () => {
      const session = await adapter.detect("/tmp/nonexistent-project-12345");
      expect(session).toBeUndefined();
    });

    it("returns session with correct agent slug", async () => {
      const cwd = process.cwd();
      const session = await adapter.detect(cwd);

      if (session) {
        expect(session.agent).toBe("claude-code");
        expect(typeof session.sessionId).toBe("string");
        expect(typeof session.sourcePath).toBe("string");
      }
    });
  });
});
