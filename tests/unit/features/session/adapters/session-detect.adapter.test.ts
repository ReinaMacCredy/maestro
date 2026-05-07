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
  let originalMaestroAgent: string | undefined;
  let originalMaestroSessionId: string | undefined;

  beforeEach(async () => {
    tempRoot = await mkdtemp(join(tmpdir(), "maestro-session-detect-"));
    originalCodexThreadId = process.env.CODEX_THREAD_ID;
    originalClaudeCode = process.env.CLAUDECODE;
    originalCodexSessionsDir = process.env.MAESTRO_CODEX_SESSIONS_DIR;
    originalClaudeSessionsDir = process.env.MAESTRO_CLAUDE_SESSIONS_DIR;
    originalClaudeProjectsDir = process.env.MAESTRO_CLAUDE_PROJECTS_DIR;
    originalMaestroAgent = process.env.MAESTRO_AGENT;
    originalMaestroSessionId = process.env.MAESTRO_SESSION_ID;
    delete process.env.CODEX_THREAD_ID;
    delete process.env.CLAUDECODE;
    delete process.env.MAESTRO_AGENT;
    delete process.env.MAESTRO_SESSION_ID;
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
    if (originalMaestroAgent === undefined) {
      delete process.env.MAESTRO_AGENT;
    } else {
      process.env.MAESTRO_AGENT = originalMaestroAgent;
    }
    if (originalMaestroSessionId === undefined) {
      delete process.env.MAESTRO_SESSION_ID;
    } else {
      process.env.MAESTRO_SESSION_ID = originalMaestroSessionId;
    }
  });

  describe("detect", () => {
    it("prefers Maestro session env over Codex and Claude-specific detection", async () => {
      process.env.MAESTRO_AGENT = "hermes";
      process.env.MAESTRO_SESSION_ID = "handoff-123";
      process.env.CLAUDECODE = "1";
      process.env.CODEX_THREAD_ID = "thread-123";

      const session = await adapter.detect(process.cwd());

      expect(session).toEqual({
        agent: "hermes",
        sessionId: "handoff-123",
        sourcePath: "env:MAESTRO_SESSION_ID:handoff-123",
        startedAt: undefined,
      });
    });

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
          // Matches encodeProjectPath in the adapter: both slash flavors
          // become dashes so the encoding stays stable across platforms.
          cwd.replace(/[\\/]+$/, "").replace(/[\\/]/g, "-"),
          `${sessionId}.jsonl`,
        ),
        startedAt,
      });
    });

    it("falls back to cwd-scan when the ppid-keyed session file is missing", async () => {
      const repoCwd = join(tempRoot, "repo");
      const otherCwd = join(tempRoot, "other");
      const sessionsDir = process.env.MAESTRO_CLAUDE_SESSIONS_DIR!;
      await mkdir(repoCwd, { recursive: true });
      await mkdir(sessionsDir, { recursive: true });
      await writeFile(
        join(sessionsDir, "99999.json"),
        JSON.stringify({
          pid: 99999,
          sessionId: "claude-match",
          cwd: repoCwd,
          startedAt: 1_777_000_000_000,
        }),
      );
      await writeFile(
        join(sessionsDir, "88888.json"),
        JSON.stringify({
          pid: 88888,
          sessionId: "claude-other",
          cwd: otherCwd,
          startedAt: 1_777_000_001_000,
        }),
      );
      process.env.CLAUDECODE = "1";

      const session = await adapter.detect(join(repoCwd, "nested", "deeper"));

      expect(session?.agent).toBe("claude-code");
      expect(session?.sessionId).toBe("claude-match");
    });

    it("picks the most recent session when multiple match the cwd ancestry", async () => {
      const repoCwd = join(tempRoot, "repo");
      const sessionsDir = process.env.MAESTRO_CLAUDE_SESSIONS_DIR!;
      await mkdir(repoCwd, { recursive: true });
      await mkdir(sessionsDir, { recursive: true });
      await writeFile(
        join(sessionsDir, "11111.json"),
        JSON.stringify({
          pid: 11111,
          sessionId: "claude-old",
          cwd: repoCwd,
          startedAt: 1_777_000_000_000,
        }),
      );
      await writeFile(
        join(sessionsDir, "22222.json"),
        JSON.stringify({
          pid: 22222,
          sessionId: "claude-new",
          cwd: repoCwd,
          startedAt: 1_777_000_005_000,
        }),
      );
      process.env.CLAUDECODE = "1";

      const session = await adapter.detect(repoCwd);

      expect(session?.sessionId).toBe("claude-new");
    });

    it("returns undefined when no claude session cwd is an ancestor of the caller", async () => {
      const callerCwd = join(tempRoot, "repo");
      const unrelatedCwd = join(tempRoot, "elsewhere");
      const sessionsDir = process.env.MAESTRO_CLAUDE_SESSIONS_DIR!;
      await mkdir(callerCwd, { recursive: true });
      await mkdir(sessionsDir, { recursive: true });
      await writeFile(
        join(sessionsDir, "77777.json"),
        JSON.stringify({
          pid: 77777,
          sessionId: "claude-unrelated",
          cwd: unrelatedCwd,
          startedAt: 1_777_000_000_000,
        }),
      );
      process.env.CLAUDECODE = "1";

      const session = await adapter.detect(callerCwd);

      expect(session).toBeUndefined();
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
