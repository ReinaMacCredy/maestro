/**
 * Create a new UKI handoff record.
 *
 * Accepts structured UkiSlots from the caller plus optional overrides
 * for agent identity and session id. If the caller does not supply
 * agent/sessionId, the use-case auto-fills them from the session-detect
 * port. If session-detect also returns nothing (e.g. in CI or outside
 * a tracked shell), the use-case falls back to reasonable defaults so
 * the handoff is still persistable.
 */
import type { UkiSlots } from "../lib/uki-format.js";
import type { UkiHandoff } from "../domain/uki-types.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { SessionDetectPort } from "../ports/session-detect.port.js";

export interface CreateUkiHandoffOptions {
  readonly slots: UkiSlots;
  readonly agent?: string;
  readonly sessionId?: string;
}

const DEFAULT_AGENT = "unknown";
const DEFAULT_SESSION_ID = "none";

export async function createUkiHandoff(
  handoffStore: HandoffStorePort,
  sessionDetect: SessionDetectPort,
  cwd: string,
  opts: CreateUkiHandoffOptions,
): Promise<UkiHandoff> {
  let agent = opts.agent;
  let sessionId = opts.sessionId;

  if (!agent || !sessionId) {
    try {
      const detected = await sessionDetect.detect(cwd);
      if (detected) {
        agent = agent ?? detected.agent;
        sessionId = sessionId ?? detected.sessionId;
      }
    } catch {
      // Session detect is best-effort; fall through to defaults.
    }
  }

  return handoffStore.create({
    slots: opts.slots,
    agent: agent ?? DEFAULT_AGENT,
    sessionId: sessionId ?? DEFAULT_SESSION_ID,
  });
}
