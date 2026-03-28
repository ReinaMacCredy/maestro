import { homedir } from "node:os";
import { basename, join } from "node:path";
import { readdir, realpath } from "node:fs/promises";
import type { DetectionMethod, HandoffSession } from "../domain/types.js";
import type { SessionDetectPort } from "../ports/session-detect.port.js";
import { readJson } from "../lib/fs.js";

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
    if (process.env.CLAUDECODE === "1") {
      const session = await readJson<ClaudeSessionFile>(
        join(CLAUDE_SESSIONS_DIR, `${process.ppid}.json`),
      );
      if (session?.sessionId && session.cwd && session.startedAt) {
        return buildClaudeSession(cwd, session, "pid");
      }
    }

    const codexThreadId = process.env.CODEX_THREAD_ID;
    if (codexThreadId) {
      return resolveCodexSession(codexThreadId, "env");
    }

    return this.detectByCwd(cwd);
  }

  async resolve(cwd: string, sessionId: string): Promise<HandoffSession | undefined> {
    const sessions = await readClaudeSessionFiles();
    const match = sessions.find((s) => s.sessionId.startsWith(sessionId));
    if (match) {
      return buildClaudeSession(cwd, match, "explicit");
    }
    return resolveCodexSession(sessionId, "explicit");
  }

  private async detectByCwd(cwd: string): Promise<HandoffSession | undefined> {
    const sessions = await readClaudeSessionFiles();
    const matching = sessions
      .filter((s) => normalizePath(s.cwd) === normalizePath(cwd))
      .sort((a, b) => b.startedAt - a.startedAt);

    const best = matching[0];
    if (!best) return undefined;
    return buildClaudeSession(cwd, best, "cwd-fallback");
  }
}

async function buildClaudeSession(
  cwd: string,
  session: ClaudeSessionFile,
  method: DetectionMethod,
): Promise<HandoffSession> {
  let resolvedCwd: string;
  try {
    resolvedCwd = await realpath(cwd);
  } catch {
    resolvedCwd = cwd;
  }

  const sourcePath = join(
    homedir(),
    ".claude",
    "projects",
    encodeProjectPath(normalizePath(resolvedCwd)),
    session.sessionId + ".jsonl",
  );

  return {
    agent: "claude-code",
    sessionId: session.sessionId,
    sourcePath,
    startedAt: session.startedAt,
    detectionMethod: method,
  };
}

async function readClaudeSessionFiles(): Promise<ClaudeSessionFile[]> {
  try {
    const entries = await readdir(CLAUDE_SESSIONS_DIR);
    const settled = await Promise.allSettled(
      entries
        .filter((e) => e.endsWith(".json"))
        .map((e) => readJson<ClaudeSessionFile>(join(CLAUDE_SESSIONS_DIR, e))),
    );
    return settled
      .filter((r): r is PromiseFulfilledResult<ClaudeSessionFile | undefined> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter(
        (d): d is ClaudeSessionFile => d !== undefined && !!d.sessionId && !!d.cwd && !!d.startedAt,
      );
  } catch {
    return [];
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

async function resolveCodexSession(
  threadId: string,
  method: DetectionMethod,
): Promise<HandoffSession | undefined> {
  try {
    const glob = new Bun.Glob(`**/*-${threadId}*.jsonl`);
    for await (const path of glob.scan({ cwd: CODEX_SESSIONS_DIR, absolute: true })) {
      const filename = basename(path);
      const idMatch = filename.match(/rollout-[^-]+-(.+)\.jsonl$/);
      const fullId = idMatch?.[1] ?? threadId;

      if (fullId === threadId || fullId.startsWith(threadId)) {
        return {
          agent: "codex",
          sessionId: fullId,
          sourcePath: path,
          startedAt: parseCodexTimestamp(filename),
          detectionMethod: method,
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

function encodeProjectPath(cwd: string): string {
  return cwd.replace(/\//g, "-");
}
