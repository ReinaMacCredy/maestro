import type { SessionDetectPort } from "./ports/session-detect.port.js";
import { ClaudeSessionDetectAdapter } from "./adapters/session-detect.adapter.js";

export interface SessionServices {
  readonly sessionDetect: SessionDetectPort;
}

export function buildSessionServices(): SessionServices {
  return {
    sessionDetect: new ClaudeSessionDetectAdapter(),
  };
}
