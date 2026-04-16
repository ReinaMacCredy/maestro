import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, unlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { ClaudeSessionDetectAdapter } from "@/features/session";

const adapter = new ClaudeSessionDetectAdapter();

describe("ClaudeSessionDetectAdapter", () => {
  let tempRoot: string;
  let originalCodexThreadId: string | undefined;
  let originalClaudeCode: string | undefined;
  let originalCodexSessionsDir: string | undefined;
  let originalClaudeSessionsDir: string | undefined;
  let originalClaudeProjectsDir: string | undefined;

  beforeEach(async () => {
    tempRoot = await mkdtemp(join(tmpdir(), "maestro-session-detect-"));
    originalCodexThreadId = process.env.CODEX_THREAD_ID;
    originalClaudeCode = process.env.CLAUDECODE;
    originalCodexSessionsDir = process.env.MAESTRO_CODEX_SESSIONS_DIR;
    originalClaudeSessionsDir = process.env.MAESTRO_CLAUDE_SESSIONS_DIR;
    originalClaudeProjectsDir = process.env.MAESTRO_CLAUDE_PROJECTS_DIR;
    delete process.env.CODEX_THREAD_ID;
    delete process.env.CLAUDECODE;
    process.env.MAESTRO_CODEX_SESSIONS_DIR = join(tempRoot, "codex-sessions");
    process.env.MAESTRO_CLAUDE_SESSIONS_DIR = join(tempRoot, "claude-sessions");
    process.env.MAESTRO_CLAUDE_PROJECTS_DIR = join(tempRoot, "claude-projects");
  });

  afterEach(async () => {
    await rm(tempRoot, { recursive: true, force: true });
    if (originalCodexThreadId === undefined) {
      delete process.env.CODEX_THREAD_ID;
    } else {
      process.env.CODEX_THREAD_ID = originalCodexThreadId;
    }
    if (originalClaudeCode === undefined) {
      delete process.env.CLAUDECODE;
    } else {
      process.env.CLAUDECODE = originalClaudeCode;
    }
    if (originalCodexSessionsDir === undefined) {
      delete process.env.MAESTRO_CODEX_SESSIONS_DIR;
    } else {
      process.env.MAESTRO_CODEX_SESSIONS_DIR = originalCodexSessionsDir;
    }
    if (originalClaudeSessionsDir === undefined) {
      delete process.env.MAESTRO_CLAUDE_SESSIONS_DIR;
    } else {
      process.env.MAESTRO_CLAUDE_SESSIONS_DIR = originalClaudeSessionsDir;
    }
    if (originalClaudeProjectsDir === undefined) {
      delete process.env.MAESTRO_CLAUDE_PROJECTS_DIR;
    } else {
      process.env.MAESTRO_CLAUDE_PROJECTS_DIR = originalClaudeProjectsDir;
    }
  });

  describe("detect", () => {
    it("returns a codex session from the configured session root", async () => {
      const sessionsDir = process.env.MAESTRO_CODEX_SESSIONS_DIR!;
      const rolloutPath = join(
        sessionsDir,
        "suite",
        "rollout-2026-04-16T14-00-00-thread-123.jsonl",
      );
      await mkdir(join(sessionsDir, "suite"), { recursive: true });
      await writeFile(rolloutPath, "{}\n");
      process.env.CODEX_THREAD_ID = "thread-123";

      const session = await adapter.detect(process.cwd());

      expect(session).toEqual({
        agent: "codex",
        sessionId: "thread-123",
        sourcePath: rolloutPath,
        startedAt: new Date("2026-04-16T14:00:00").getTime(),
      });
    });

    it("returns undefined when the configured codex session root has no match", async () => {
      process.env.CODEX_THREAD_ID = "missing-thread";

      const session = await adapter.detect(process.cwd());

      expect(session).toBeUndefined();
    });

    it("invalidates cached codex sessions when the rollout file disappears", async () => {
      const sessionsDir = process.env.MAESTRO_CODEX_SESSIONS_DIR!;
      const rolloutPath = join(
        sessionsDir,
        "suite",
        "rollout-2026-04-16T14-00-00-thread-cache.jsonl",
      );
      await mkdir(join(sessionsDir, "suite"), { recursive: true });
      await writeFile(rolloutPath, "{}\n");
      process.env.CODEX_THREAD_ID = "thread-cache";

      const cachedAdapter = new ClaudeSessionDetectAdapter();
      expect((await cachedAdapter.detect(process.cwd()))?.sourcePath).toBe(rolloutPath);

      await unlink(rolloutPath);
      expect(await cachedAdapter.detect(process.cwd())).toBeUndefined();
    });

    it("prefers the newest matching codex rollout when multiple prefix matches exist", async () => {
      const sessionsDir = process.env.MAESTRO_CODEX_SESSIONS_DIR!;
      const suiteDir = join(sessionsDir, "suite");
      await mkdir(suiteDir, { recursive: true });
      await writeFile(join(suiteDir, "rollout-2026-04-16T14-00-00-thread-cacheAAAA.jsonl"), "{}\n");
      process.env.CODEX_THREAD_ID = "thread-cache";

      const cachedAdapter = new ClaudeSessionDetectAdapter();
      expect((await cachedAdapter.detect(process.cwd()))?.sessionId).toBe("thread-cacheAAAA");

      await writeFile(join(suiteDir, "rollout-2026-04-16T14-05-00-thread-cacheBBBB.jsonl"), "{}\n");
      expect((await cachedAdapter.detect(process.cwd()))?.sessionId).toBe("thread-cacheBBBB");
    });

    it("treats an empty codex root override as unset", async () => {
      const fallbackDir = join(tempRoot, "fallback-codex");
      const cwdRolloutPath = join(tempRoot, "rollout-2026-04-16T14-00-00-thread-emptydir.jsonl");
      await mkdir(fallbackDir, { recursive: true });
      await writeFile(cwdRolloutPath, "{}\n");
      process.env.MAESTRO_CODEX_SESSIONS_DIR = "";
      process.env.CODEX_THREAD_ID = "thread-emptydir";

      const hermeticAdapter = new ClaudeSessionDetectAdapter({
        codexSessionsDir: fallbackDir,
      });
      const session = await hermeticAdapter.detect(tempRoot);

      expect(session).toBeUndefined();
    });

    it("returns a claude session from the configured session roots", async () => {
      const cwd = join(tempRoot, "repo");
      const sessionId = "claude-session-1";
      const startedAt = 1_777_000_000_000;
      await mkdir(cwd, { recursive: true });
      await mkdir(process.env.MAESTRO_CLAUDE_SESSIONS_DIR!, { recursive: true });
      await writeFile(
        join(process.env.MAESTRO_CLAUDE_SESSIONS_DIR!, `${process.ppid}.json`),
        JSON.stringify({
          pid: process.pid,
          sessionId,
          cwd,
          startedAt,
        }),
      );
      process.env.CLAUDECODE = "1";

      const session = await adapter.detect(cwd);

      expect(session).toEqual({
        agent: "claude-code",
        sessionId,
        sourcePath: join(
          process.env.MAESTRO_CLAUDE_PROJECTS_DIR!,
          cwd.replace(/\/+$/, "").replace(/\//g, "-"),
          `${sessionId}.jsonl`,
        ),
        startedAt,
      });
    });
  });

  describe("lookup", () => {
    it("returns undefined when the configured codex session root is missing", async () => {
      const hermeticAdapter = new ClaudeSessionDetectAdapter({
        codexSessionsDir: join(tempRoot, "missing-codex-sessions"),
      });

      await expect(hermeticAdapter.lookup("codex", "dead-thread")).resolves.toBeUndefined();
    });
  });
});
