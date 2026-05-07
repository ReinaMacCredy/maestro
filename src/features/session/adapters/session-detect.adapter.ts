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

  async detect(cwd: string): Promise<AgentSession | undefined> {
    const maestroAgent = process.env.MAESTRO_AGENT?.trim();
    const maestroSessionId = process.env.MAESTRO_SESSION_ID?.trim();
    if (maestroAgent && maestroSessionId) {
      return {
        agent: maestroAgent,
        sessionId: maestroSessionId,
        sourcePath: `env:MAESTRO_SESSION_ID:${maestroSessionId}`,
        startedAt: undefined,
      };
    }

    if (process.env.CLAUDECODE === "1") {
      const session = await readJson<ClaudeSessionFile>(
        join(this.resolveClaudeSessionsDir(), `${process.ppid}.json`),
      );
      if (session?.sessionId && session.cwd && session.startedAt) {
        return this.buildClaudeSession(session);
      }

      const fallback = await this.findClaudeSessionByCwd(cwd);
      if (fallback) {
        return this.buildClaudeSession(fallback);
      }
    }

    const codexThreadId = process.env.CODEX_THREAD_ID;
    if (codexThreadId) {
      return this.resolveCodexSession(codexThreadId);
    }

    return undefined;
  }

  private async findClaudeSessionByCwd(cwd: string): Promise<ClaudeSessionFile | undefined> {
    const sessionsDir = this.resolveClaudeSessionsDir();
    if (!await pathExists(sessionsDir)) {
      return undefined;
    }
    const glob = new Bun.Glob("*.json");
    let best: ClaudeSessionFile | undefined;
    for await (const path of glob.scan({ cwd: sessionsDir, absolute: true })) {
      const session = await readJson<ClaudeSessionFile>(path).catch(() => undefined);
      if (!session?.sessionId || !session.cwd || !session.startedAt) continue;
      if (!isCwdAncestorOrSame(session.cwd, cwd)) continue;
      if (!best || session.startedAt > best.startedAt) {
        best = session;
      }
    }
    return best;
  }

  async lookup(agent: AgentSession["agent"], sessionId: string): Promise<AgentSession | undefined> {
    switch (agent) {
      case "codex":
        return this.findCodexSession(sessionId, "exact");
      case "claude-code":
        return this.findClaudeProjectSession(sessionId);
      default:
        return undefined;
    }
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
      if (cached && cached.sessionId === threadId && await pathExists(cached.sourcePath)) {
        return cached;
      }
      this.codexSessionCache.delete(cacheKey);

      const session = await this.findCodexSession(threadId, "prefix");
      if (session?.sessionId === threadId) {
        this.codexSessionCache.set(cacheKey, session);
      }
      return session;
    } catch {
      return undefined;
    }
  }

  private async findCodexSession(
    threadId: string,
    mode: "exact" | "prefix",
  ): Promise<AgentSession | undefined> {
    const sessionsDir = this.resolveCodexSessionsDir();
    if (!await pathExists(sessionsDir)) {
      return undefined;
    }
    const glob = new Bun.Glob(mode === "exact" ? `**/*-${threadId}.jsonl` : `**/*-${threadId}*.jsonl`);
    let best: AgentSession | undefined;

    for await (const path of glob.scan({ cwd: sessionsDir, absolute: true })) {
      const filename = basename(path);
      const idMatch = filename.match(/rollout-\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}-(.+)\.jsonl$/);
      const fullId = idMatch?.[1] ?? threadId;
      const matches = mode === "exact" ? fullId === threadId : fullId === threadId || fullId.startsWith(threadId);
      if (!matches) {
        continue;
      }
      const candidate: AgentSession = {
        agent: "codex",
        sessionId: fullId,
        sourcePath: path,
        startedAt: parseCodexTimestamp(filename),
      };
      if (!best || compareSessionCandidates(threadId, candidate, best) < 0) {
        best = candidate;
      }
    }

    return best;
  }

  private async findClaudeProjectSession(sessionId: string): Promise<AgentSession | undefined> {
    try {
      const projectsDir = this.resolveClaudeProjectsDir();
      if (!await pathExists(projectsDir)) {
        return undefined;
      }
      const glob = new Bun.Glob(`**/${sessionId}.jsonl`);
      let best: AgentSession | undefined;

      for await (const path of glob.scan({ cwd: projectsDir, absolute: true })) {
        const candidate: AgentSession = {
          agent: "claude-code",
          sessionId,
          sourcePath: path,
          startedAt: undefined,
        };
        if (!best || candidate.sourcePath > best.sourcePath) {
          best = candidate;
        }
      }

      return best;
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
  return p.replace(/[\\/]+$/, "");
}

function isCwdAncestorOrSame(ancestor: string, descendant: string): boolean {
  const a = normalizePath(ancestor);
  const d = normalizePath(descendant);
  if (a === d) return true;
  return d.startsWith(`${a}/`) || d.startsWith(`${a}\\`);
}

function encodeProjectPath(cwd: string): string {
  return cwd.replace(/[\\/]/g, "-");
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

function compareSessionCandidates(
  requestedId: string,
  left: AgentSession,
  right: AgentSession,
): number {
  const leftExact = left.sessionId === requestedId ? 0 : 1;
  const rightExact = right.sessionId === requestedId ? 0 : 1;
  if (leftExact !== rightExact) {
    return leftExact - rightExact;
  }

  const leftStarted = left.startedAt ?? Number.NEGATIVE_INFINITY;
  const rightStarted = right.startedAt ?? Number.NEGATIVE_INFINITY;
  if (leftStarted !== rightStarted) {
    return rightStarted - leftStarted;
  }

  return right.sourcePath.localeCompare(left.sourcePath);
}
