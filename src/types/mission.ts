import type { MissionState } from "./mission-state.js";

export type MissionId = string;

export interface Mission {
  readonly id: MissionId;
  readonly slug: string;
  readonly title: string;
  readonly state: MissionState;
  readonly spec_path?: string;
  readonly cancel_reason?: string;
  readonly created_at: string;
  readonly updated_at: string;
}

export function generateMissionId(): MissionId {
  const rand = Math.random().toString(36).slice(2, 8);
  return `pln-${Date.now().toString(36)}-${rand}`;
}

export const MISSION_ID_PATTERN = /^pln-[a-z0-9]+-[a-z0-9]+$/;

export function isMissionId(value: unknown): value is MissionId {
  return typeof value === "string" && MISSION_ID_PATTERN.test(value);
}
