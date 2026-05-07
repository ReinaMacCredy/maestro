import os from "node:os";

export function detectMcpSessionId(env: NodeJS.ProcessEnv = process.env): string {
  return (
    env.MAESTRO_SESSION_ID ||
    env.CLAUDECODE_SESSION_ID ||
    env.CODEX_THREAD_ID ||
    `${os.userInfo().username}@${os.hostname()}`
  );
}
