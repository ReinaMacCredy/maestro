import type { SessionDetectPort } from "../ports/session-detect.port.js";
import type { DetectionMethod, MaestroConfig, HandoffSession } from "../domain/types.js";
import { MaestroError } from "../domain/errors.js";

const DEFAULT_STALE_MINUTES = 60;

export interface DetectSessionOpts {
  readonly cwd: string;
  readonly sessionId?: string;
  readonly noSession?: boolean;
}

export interface DetectSessionResult {
  readonly session: HandoffSession;
  readonly method: DetectionMethod;
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
        "Or skip with --skip-session",
      ]);
    }
    return { session, method: session.detectionMethod ?? "explicit", stale: false };
  }

  const session = await sessionDetect.detect(opts.cwd);
  if (!session) return undefined;

  const method = session.detectionMethod ?? "cwd-fallback";

  let stale = false;
  if (method === "cwd-fallback" && session.startedAt) {
    const staleMinutes = config.sessionDetection?.staleMinutes ?? DEFAULT_STALE_MINUTES;
    const ageMinutes = (Date.now() - session.startedAt) / 60_000;
    if (ageMinutes > staleMinutes) {
      stale = true;
    }
  }

  return { session, method, stale };
}
