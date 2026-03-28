import { homedir } from "node:os";
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import type { HandoffSession } from "../domain/types.js";
import type { SessionDetectPort } from "../ports/session-detect.port.js";

interface ClaudeSessionFile {
  readonly pid: number;
  readonly sessionId: string;
  readonly cwd: string;
  readonly startedAt: number;
  readonly kind?: string;
  readonly entrypoint?: string;
}

const CLAUDE_SESSIONS_DIR = join(homedir(), ".claude", "sessions");
const CODEX_SESSIONS_DIR = join(homedir(), ".codex", "sessions");

export class ClaudeSessionDetectAdapter implements SessionDetectPort {
  async detect(cwd: string): Promise<HandoffSession | undefined> {
    // 1. Claude Code: exact PID match
    if (process.env.CLAUDECODE === "1") {
      const ppid = process.ppid;
      const session = await readClaudeSessionByPid(ppid);
      if (session) {
        return buildClaudeSession(cwd, session);
      }
    }

    // 2. Codex: exact env var match
    const codexThreadId = process.env.CODEX_THREAD_ID;
    if (codexThreadId) {
      return resolveCodexSession(codexThreadId);
    }

    // 3. Fallback: cwd-based match (may be stale)
    return this.detectByCwd(cwd);
  }

  async resolve(cwd: string, sessionId: string): Promise<HandoffSession | undefined> {
    // Search Claude sessions by prefix match
    const sessions = await readClaudeSessionFiles();
    const match = sessions.find((s) =>
      s.sessionId.startsWith(sessionId),
    );
    if (match) {
      return buildClaudeSession(cwd, match);
    }

    // Search Codex sessions by prefix match
    return resolveCodexSession(sessionId);
  }

  private async detectByCwd(cwd: string): Promise<HandoffSession | undefined> {
    const sessions = await readClaudeSessionFiles();

    const matching = sessions
      .filter((s) => normalizePath(s.cwd) === normalizePath(cwd))
      .sort((a, b) => b.startedAt - a.startedAt);

    const best = matching[0];
    if (!best) return undefined;

    return buildClaudeSession(cwd, best);
  }
}

function buildClaudeSession(cwd: string, session: ClaudeSessionFile): HandoffSession {
  const encodedCwd = encodeProjectPath(cwd);
  const sourcePath = join(
    homedir(),
    ".claude",
    "projects",
    encodedCwd,
    session.sessionId,
  );

  return {
    agent: "claude-code",
    sessionId: session.sessionId,
    sourcePath,
    cassIndexed: false,
    startedAt: session.startedAt,
  };
}

async function readClaudeSessionByPid(pid: number): Promise<ClaudeSessionFile | undefined> {
  try {
    const file = Bun.file(join(CLAUDE_SESSIONS_DIR, `${pid}.json`));
    if (!(await file.exists())) return undefined;
    const data = (await file.json()) as ClaudeSessionFile;
    if (data.sessionId && data.cwd && data.startedAt) return data;
    return undefined;
  } catch {
    return undefined;
  }
}

async function readClaudeSessionFiles(): Promise<ClaudeSessionFile[]> {
  try {
    const entries = await readdir(CLAUDE_SESSIONS_DIR);
    const sessions: ClaudeSessionFile[] = [];

    for (const entry of entries) {
      if (!entry.endsWith(".json")) continue;
      try {
        const file = Bun.file(join(CLAUDE_SESSIONS_DIR, entry));
        const data = (await file.json()) as ClaudeSessionFile;
        if (data.sessionId && data.cwd && data.startedAt) {
          sessions.push(data);
        }
      } catch {
        // Skip malformed session files
      }
    }

    return sessions;
  } catch {
    return [];
  }
}

async function resolveCodexSession(threadId: string): Promise<HandoffSession | undefined> {
  // Codex stores sessions in date-sharded dirs: ~/.codex/sessions/YYYY/MM/DD/rollout-*-{threadId}.jsonl
  // Glob across all date dirs to find the matching file
  try {
    const glob = new Bun.Glob(`**/*-${threadId}.jsonl`);
    for await (const path of glob.scan({ cwd: CODEX_SESSIONS_DIR, absolute: true })) {
      return {
        agent: "codex",
        sessionId: threadId,
        sourcePath: path,
        cassIndexed: false,
        startedAt: Date.now(),
      };
    }

    // Prefix match: try shorter ID
    if (threadId.length < 36) {
      const prefixGlob = new Bun.Glob(`**/*-${threadId}*.jsonl`);
      for await (const path of prefixGlob.scan({ cwd: CODEX_SESSIONS_DIR, absolute: true })) {
        // Extract full thread ID from filename
        const filename = path.split("/").pop()!;
        const match = filename.match(/rollout-[^-]+-(.+)\.jsonl$/);
        const fullId = match?.[1] ?? threadId;
        return {
          agent: "codex",
          sessionId: fullId,
          sourcePath: path,
          cassIndexed: false,
          startedAt: Date.now(),
        };
      }
    }

    return undefined;
  } catch {
    return undefined;
  }
}

function normalizePath(p: string): string {
  return p.replace(/\/+$/, "");
}

export function encodeProjectPath(cwd: string): string {
  return cwd.replace(/\//g, "-").replace(/^-/, "");
}
