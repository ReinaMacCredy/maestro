import { homedir } from "node:os";
import { basename, join } from "node:path";
import { access } from "node:fs/promises";
import type { AgentSession } from "../domain/types.js";
import type { SessionDetectPort } from "../ports/session-detect.port.js";
import { readJson } from "@/shared/lib/fs.js";

/**
 * Phase 1 strip: this adapter used to resolve sessions by cwd fallback,
 * session-id prefix, and on-disk pid files. The conductor model only
 * needs to answer "what agent is currently running?" for memory and
 * notes scoping, so the implementation collapses to a pair of env-var
 * reads (CLAUDECODE, CODEX_THREAD_ID). The `resolve()` method was
 * removed from the port.
 */

interface ClaudeSessionFile {
  readonly pid: number;
  readonly sessionId: string;
  readonly cwd: string;
  readonly startedAt: number;
}

interface SessionRootOptions {
  readonly claudeSessionsDir?: string;
  readonly claudeProjectsDir?: string;
  readonly codexSessionsDir?: string;
}

export class ClaudeSessionDetectAdapter implements SessionDetectPort {
  private readonly codexSessionCache = new Map<string, AgentSession>();

  constructor(private readonly roots: SessionRootOptions = {}) {}

  async detect(_cwd: string): Promise<AgentSession | undefined> {
    if (process.env.CLAUDECODE === "1") {
      const session = await readJson<ClaudeSessionFile>(
        join(this.resolveClaudeSessionsDir(), `${process.ppid}.json`),
      );
      if (session?.sessionId && session.cwd && session.startedAt) {
        return this.buildClaudeSession(session);
      }
    }

    const codexThreadId = process.env.CODEX_THREAD_ID;
    if (codexThreadId) {
      return this.resolveCodexSession(codexThreadId);
    }

    return undefined;
  }

  private buildClaudeSession(session: ClaudeSessionFile): AgentSession {
    const sourcePath = join(
      this.resolveClaudeProjectsDir(),
      encodeProjectPath(normalizePath(session.cwd)),
      `${session.sessionId}.jsonl`,
    );

    return {
      agent: "claude-code",
      sessionId: session.sessionId,
      sourcePath,
      startedAt: session.startedAt,
    };
  }

  private async resolveCodexSession(threadId: string): Promise<AgentSession | undefined> {
    try {
      const sessionsDir = this.resolveCodexSessionsDir();
      const cacheKey = `${sessionsDir}::${threadId}`;
      const cached = this.codexSessionCache.get(cacheKey);
      if (cached && await pathExists(cached.sourcePath)) {
        return cached;
      }
      this.codexSessionCache.delete(cacheKey);

      const glob = new Bun.Glob(`**/*-${threadId}*.jsonl`);
      for await (const path of glob.scan({ cwd: sessionsDir, absolute: true })) {
        const filename = basename(path);
        const idMatch = filename.match(/rollout-\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}-(.+)\.jsonl$/);
        const fullId = idMatch?.[1] ?? threadId;

        if (fullId === threadId || fullId.startsWith(threadId)) {
          const session: AgentSession = {
            agent: "codex",
            sessionId: fullId,
            sourcePath: path,
            startedAt: parseCodexTimestamp(filename),
          };
          this.codexSessionCache.set(cacheKey, session);
          return session;
        }
      }
      return undefined;
    } catch {
      return undefined;
    }
  }

  private resolveClaudeSessionsDir(): string {
    return resolveDirOverride(
      process.env.MAESTRO_CLAUDE_SESSIONS_DIR,
      this.roots.claudeSessionsDir ?? join(homedir(), ".claude", "sessions"),
    );
  }

  private resolveClaudeProjectsDir(): string {
    return resolveDirOverride(
      process.env.MAESTRO_CLAUDE_PROJECTS_DIR,
      this.roots.claudeProjectsDir ?? join(homedir(), ".claude", "projects"),
    );
  }

  private resolveCodexSessionsDir(): string {
    return resolveDirOverride(
      process.env.MAESTRO_CODEX_SESSIONS_DIR,
      this.roots.codexSessionsDir ?? join(homedir(), ".codex", "sessions"),
    );
  }
}

function parseCodexTimestamp(filename: string): number | undefined {
  // rollout-2026-03-28T14-39-21-{threadId}.jsonl
  const match = filename.match(/rollout-(\d{4}-\d{2}-\d{2}T[\d-]+)-/);
  if (!match) return undefined;
  const iso = match[1]!.replace(/T(\d{2})-(\d{2})-(\d{2})/, "T$1:$2:$3");
  const ts = new Date(iso).getTime();
  return Number.isNaN(ts) ? undefined : ts;
}

function resolveDirOverride(
  override: string | undefined,
  fallback: string,
): string {
  if (override === undefined) {
    return fallback;
  }
  const trimmed = override.trim();
  return trimmed.length > 0 ? trimmed : fallback;
}

function normalizePath(p: string): string {
  return p.replace(/\/+$/, "");
}

function encodeProjectPath(cwd: string): string {
  return cwd.replace(/\//g, "-");
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}
