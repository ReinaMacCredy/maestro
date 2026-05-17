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
  const allowed = (MISSION_TRANSITIONS as Record<string, readonly MissionState[]>)[from];
  return allowed !== undefined && allowed.includes(to);
}

export class MissionTransitionError extends Error {
  readonly from: MissionState;
  readonly to: MissionState;
  readonly allowed: readonly MissionState[];

  constructor(from: MissionState, to: MissionState) {
    // `from` is typed as MissionState but reaches us as a string off-disk;
    // a legacy/unknown value (e.g. v1 "specified") would surface a TypeError
    // here instead of the intended MissionTransitionError. Default to [] so
    // the message stays informative.
    const allowed = (MISSION_TRANSITIONS as Record<string, readonly MissionState[]>)[from] ?? [];
    const knownState = (MISSION_STATES as readonly string[]).includes(from);
    const hint = !knownState
      ? `${from} is not a recognized mission state (legacy data?)`
      : allowed.length === 0
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
