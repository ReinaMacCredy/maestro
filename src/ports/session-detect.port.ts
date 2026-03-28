import type { HandoffSession } from "../domain/types.js";

export interface SessionDetectPort {
  detect(cwd: string): Promise<HandoffSession | undefined>;
}
