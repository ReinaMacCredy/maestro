import type { Mission, MissionId } from "../types/mission.js";
import type { MissionState } from "../types/mission-state.js";

export interface CreateMissionInput {
  readonly slug: string;
  readonly title: string;
  readonly state: MissionState;
  readonly spec_path?: string;
}

export type MissionPatch = Partial<
  Omit<Mission, "id" | "slug" | "created_at" | "updated_at">
>;

export interface MissionStorePort {
  create(input: CreateMissionInput): Promise<Mission>;
  get(id: MissionId): Promise<Mission | undefined>;
  update(id: MissionId, patch: MissionPatch): Promise<Mission>;
  list(): Promise<readonly Mission[]>;
  listByState(state: MissionState): Promise<readonly Mission[]>;
}

export class MissionNotFoundError extends Error {
  readonly missionId: MissionId;
  constructor(missionId: MissionId) {
    super(`Mission ${missionId} not found`);
    this.name = "MissionNotFoundError";
    this.missionId = missionId;
  }
}

export class DuplicateMissionSlugError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`Mission with slug ${slug} already exists`);
    this.name = "DuplicateMissionSlugError";
    this.slug = slug;
  }
}
