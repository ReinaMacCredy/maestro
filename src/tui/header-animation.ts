import cliSpinners from "cli-spinners";

import type { MissionControlSnapshot } from "./state/types.js";

const HEADER_DOT_FRAMES = ["●••", "•●•", "••●", "•●•"] as const;

export const HEADER_DOT_INTERVAL_MS = cliSpinners.orangePulse.interval;

export function isHeaderAnimationActive(snapshot: MissionControlSnapshot): boolean {
  return snapshot.mode === "mission"
    && (snapshot.effectiveStatus === "executing" || snapshot.effectiveStatus === "validating");
}

export function getHeaderDotsFrame(snapshot: MissionControlSnapshot, frameIndex: number): string {
  if (!isHeaderAnimationActive(snapshot)) {
    return HEADER_DOT_FRAMES[0];
  }

  return HEADER_DOT_FRAMES[Math.abs(frameIndex) % HEADER_DOT_FRAMES.length] ?? HEADER_DOT_FRAMES[0];
}
