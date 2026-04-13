import type { AgentSession } from "../domain/types.js";

/**
 * Phase 1 strip: the conductor now owns only identity detection
 * for memory and notes scoping, so the port is reduced to a single
 * env-only `detect` method. Prefix-based `resolve()` is gone along
 * with the cwd-fallback lookup and the staleness warning flow.
 */
export interface SessionDetectPort {
  detect(cwd: string): Promise<AgentSession | undefined>;
}
