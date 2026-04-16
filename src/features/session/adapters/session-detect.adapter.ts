import { homedir } from "node:os";
import { basename, join } from "node:path";
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

export class ClaudeSessionDetectAdapter implements SessionDetectPort {
  async detect(_cwd: string): Promise<AgentSession | undefined> {
    if (process.env.CLAUDECODE === "1") {
      const session = await readJson<ClaudeSessionFile>(
        join(resolveClaudeSessionsDir(), `${process.ppid}.json`),
      );
      if (session?.sessionId && session.cwd && session.startedAt) {
        return buildClaudeSession(session);
      }
    }

    const codexThreadId = process.env.CODEX_THREAD_ID;
    if (codexThreadId) {
      return resolveCodexSession(codexThreadId);
    }

    return undefined;
  }
}

function buildClaudeSession(session: ClaudeSessionFile): AgentSession {
  const sourcePath = join(
    resolveClaudeProjectsDir(),
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

function parseCodexTimestamp(filename: string): number | undefined {
  // rollout-2026-03-28T14-39-21-{threadId}.jsonl
  const match = filename.match(/rollout-(\d{4}-\d{2}-\d{2}T[\d-]+)-/);
  if (!match) return undefined;
  const iso = match[1]!.replace(/T(\d{2})-(\d{2})-(\d{2})/, "T$1:$2:$3");
  const ts = new Date(iso).getTime();
  return Number.isNaN(ts) ? undefined : ts;
}

async function resolveCodexSession(threadId: string): Promise<AgentSession | undefined> {
  try {
    const glob = new Bun.Glob(`**/*-${threadId}*.jsonl`);
    for await (const path of glob.scan({ cwd: resolveCodexSessionsDir(), absolute: true })) {
      const filename = basename(path);
      const idMatch = filename.match(/rollout-\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}-(.+)\.jsonl$/);
      const fullId = idMatch?.[1] ?? threadId;

      if (fullId === threadId || fullId.startsWith(threadId)) {
        return {
          agent: "codex",
          sessionId: fullId,
          sourcePath: path,
          startedAt: parseCodexTimestamp(filename),
        };
      }
    }
    return undefined;
  } catch {
    return undefined;
  }
}

function resolveClaudeSessionsDir(): string {
  return process.env.MAESTRO_CLAUDE_SESSIONS_DIR
    ?? join(homedir(), ".claude", "sessions");
}

function resolveClaudeProjectsDir(): string {
  return process.env.MAESTRO_CLAUDE_PROJECTS_DIR
    ?? join(homedir(), ".claude", "projects");
}

function resolveCodexSessionsDir(): string {
  return process.env.MAESTRO_CODEX_SESSIONS_DIR
    ?? join(homedir(), ".codex", "sessions");
}

function normalizePath(p: string): string {
  return p.replace(/\/+$/, "");
}

function encodeProjectPath(cwd: string): string {
  return cwd.replace(/\//g, "-");
}
