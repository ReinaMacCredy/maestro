// `approved` means "spec parsed via mission new --from-spec", NOT a human gate.
// v2 mission gates are task-level (policy + verdict), not mission-level.
// `intake -> planned` exists so `mission decompose` can advance bare-title
// missions without forcing them through `approved` first.

export const MISSION_STATES = [
  "intake",
  "approved",
  "planned",
  "in-progress",
  "paused",
  "completed",
  "failed",
  "cancelled",
] as const;

export type MissionState = (typeof MISSION_STATES)[number];

export const MISSION_TERMINAL_STATES = ["completed", "failed", "cancelled"] as const;
export type TerminalMissionState = (typeof MISSION_TERMINAL_STATES)[number];

export const MISSION_TRANSITIONS = {
  intake: ["approved", "planned", "cancelled"],
  approved: ["planned", "cancelled"],
  planned: ["in-progress", "cancelled"],
  "in-progress": ["paused", "completed", "failed", "cancelled"],
  paused: ["in-progress", "completed", "failed", "cancelled"],
  completed: [],
  failed: [],
  cancelled: [],
} as const satisfies Record<MissionState, readonly MissionState[]>;

export function isMissionState(value: unknown): value is MissionState {
  return typeof value === "string" && (MISSION_STATES as readonly string[]).includes(value);
}

export function isTerminalMissionState(state: MissionState): state is TerminalMissionState {
  return (MISSION_TERMINAL_STATES as readonly MissionState[]).includes(state);
}

export function canTransitionMission(from: MissionState, to: MissionState): boolean {
  return (MISSION_TRANSITIONS[from] as readonly MissionState[]).includes(to);
}

export class MissionTransitionError extends Error {
  readonly from: MissionState;
  readonly to: MissionState;
  readonly allowed: readonly MissionState[];

  constructor(from: MissionState, to: MissionState) {
    const allowed = MISSION_TRANSITIONS[from];
    const hint =
      allowed.length === 0
        ? `${from} is terminal`
        : `allowed from ${from}: ${allowed.join(", ")}`;
    super(`Invalid mission transition ${from} -> ${to} (${hint})`);
    this.name = "MissionTransitionError";
    this.from = from;
    this.to = to;
    this.allowed = allowed;
  }
}

export function assertMissionTransition(from: MissionState, to: MissionState): void {
  if (!canTransitionMission(from, to)) {
    throw new MissionTransitionError(from, to);
  }
}
