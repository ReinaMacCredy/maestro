import os from "node:os";

export function detectMcpSessionId(env: NodeJS.ProcessEnv = process.env): string {
  const explicit =
    env.MAESTRO_SESSION_ID || env.CLAUDECODE_SESSION_ID || env.CODEX_THREAD_ID;
  if (explicit) return explicit;
  // os.userInfo() throws on systems where the effective user has no passwd
  // entry (common in minimal Docker images). Fall back to env vars instead
  // of crashing the MCP session-id detection path.
  let username: string;
  try {
    username = os.userInfo().username;
  } catch {
    username = env.USER || env.USERNAME || env.LOGNAME || "unknown";
  }
  return `${username}@${os.hostname()}`;
}
