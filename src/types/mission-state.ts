// v2 mission lifecycle (ADR-0003). Auto-completes when every child task is
// in a terminal state (ADR-0011); cancellation is the only manual terminal.

export const MISSION_STATES = [
  "intake",
  "specified",
  "planned",
  "in-progress",
  "completed",
  "cancelled",
] as const;

export type MissionState = (typeof MISSION_STATES)[number];

export const MISSION_TERMINAL_STATES = ["completed", "cancelled"] as const;
export type TerminalMissionState = (typeof MISSION_TERMINAL_STATES)[number];

export const MISSION_TRANSITIONS = {
  intake: ["specified", "cancelled"],
  specified: ["planned", "cancelled"],
  planned: ["in-progress", "cancelled"],
  "in-progress": ["completed", "cancelled"],
  completed: [],
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
