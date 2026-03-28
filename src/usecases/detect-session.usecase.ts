import type { SessionDetectPort } from "../ports/session-detect.port.js";
import type { MaestroConfig, HandoffSession } from "../domain/types.js";
import { MaestroError } from "../domain/errors.js";
import { warn } from "../lib/output.js";

const DEFAULT_STALE_MINUTES = 60;

export interface DetectSessionOpts {
  readonly cwd: string;
  readonly sessionId?: string;
  readonly noSession?: boolean;
}

export interface DetectSessionResult {
  readonly session: HandoffSession;
  readonly method: "pid" | "env" | "cwd-fallback" | "explicit";
  readonly stale: boolean;
}

export async function detectSession(
  sessionDetect: SessionDetectPort,
  config: MaestroConfig,
  opts: DetectSessionOpts,
): Promise<DetectSessionResult | undefined> {
  if (opts.noSession) return undefined;

  if (opts.sessionId) {
    const session = await sessionDetect.resolve(opts.cwd, opts.sessionId);
    if (!session) {
      throw new MaestroError(`Session ${opts.sessionId} not found`, [
        "Check available sessions: maestro session --json",
        "Or skip with --no-session",
      ]);
    }
    return { session, method: "explicit", stale: false };
  }

  const session = await sessionDetect.detect(opts.cwd);
  if (!session) return undefined;

  // Determine detection method from env
  const method = process.env.CLAUDECODE === "1"
    ? "pid" as const
    : process.env.CODEX_THREAD_ID
      ? "env" as const
      : "cwd-fallback" as const;

  // Staleness check only for cwd fallback
  let stale = false;
  if (method === "cwd-fallback" && session.startedAt) {
    const staleMinutes = config.sessionDetection?.staleMinutes ?? DEFAULT_STALE_MINUTES;
    const ageMs = Date.now() - session.startedAt;
    const ageMinutes = ageMs / 60_000;
    if (ageMinutes > staleMinutes) {
      stale = true;
      const ageHuman = ageMinutes < 60
        ? `${Math.round(ageMinutes)}m`
        : `${Math.round(ageMinutes / 60)}h`;
      warn(
        `Session ${session.sessionId.slice(0, 8)} is ${ageHuman} old (cwd fallback). Use --session <id> or --no-session to override.`,
      );
    }
  }

  return { session, method, stale };
}
