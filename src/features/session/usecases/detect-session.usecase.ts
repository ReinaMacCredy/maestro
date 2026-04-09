import type { SessionDetectPort } from "../ports/session-detect.port.js";
import type { AgentSession } from "@/domain/types.js";

/**
 * Phase 1 strip: the conductor only answers "what session is this?"
 * from the current environment. The explicit `--session <id>` prefix
 * resolve flow, the cwd-fallback search, and the staleness warning
 * all came from the handoff workflow and are gone. `DetectSessionOpts`
 * still carries `noSession` because callers that want to opt out of
 * any session attribution (e.g. scripts) need to say so.
 */

export interface DetectSessionOpts {
  readonly cwd: string;
  readonly noSession?: boolean;
}

export interface DetectSessionResult {
  readonly session: AgentSession;
}

export async function detectSession(
  sessionDetect: SessionDetectPort,
  opts: DetectSessionOpts,
): Promise<DetectSessionResult | undefined> {
  if (opts.noSession) return undefined;

  const session = await sessionDetect.detect(opts.cwd);
  if (!session) return undefined;

  return { session };
}
