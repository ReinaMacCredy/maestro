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

export class ClaudeSessionDetectAdapter implements SessionDetectPort {
  async detect(cwd: string): Promise<HandoffSession | undefined> {
    const sessions = await readSessionFiles();

    // Filter sessions matching this workspace, sort by most recent
    const matching = sessions
      .filter((s) => normalizePath(s.cwd) === normalizePath(cwd))
      .sort((a, b) => b.startedAt - a.startedAt);

    const best = matching[0];
    if (!best) return undefined;

    // Construct the session source path for CASS
    const encodedCwd = encodeProjectPath(cwd);
    const sourcePath = join(
      homedir(),
      ".claude",
      "projects",
      encodedCwd,
      best.sessionId,
    );

    return {
      agent: "claude-code",
      sessionId: best.sessionId,
      sourcePath,
      cassIndexed: false,
    };
  }
}

async function readSessionFiles(): Promise<ClaudeSessionFile[]> {
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

function normalizePath(p: string): string {
  return p.replace(/\/+$/, "");
}

/**
 * Claude Code encodes project paths by replacing path separators.
 * e.g. /Users/foo/Code/bar -> -Users-foo-Code-bar
 */
function encodeProjectPath(cwd: string): string {
  return cwd.replace(/\//g, "-").replace(/^-/, "");
}
