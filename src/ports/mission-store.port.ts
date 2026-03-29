/**
 * Mission store port
 * Defines the interface for mission persistence operations
 */
import type { Mission, CreateMissionInput, UpdateMissionInput } from "../domain/mission-types.js";

export interface MissionStorePort {
  /** Get all mission IDs */
  listIds(): Promise<readonly string[]>;

  /** Get a mission by ID, returns undefined if not found */
  get(id: string): Promise<Mission | undefined>;

  /** Check if a mission exists */
  exists(id: string): Promise<boolean>;

  /**
   * Stage a new mission for creation.
   * Writes to a staging area before finalizing.
   * Returns the mission ID.
   */
  stage(input: CreateMissionInput, id: string): Promise<string>;

  /**
   * Finalize a staged mission.
   * Moves from staging to the active missions directory.
   */
  finalize(id: string): Promise<void>;

  /** Update an existing mission */
  update(id: string, input: UpdateMissionInput): Promise<Mission | undefined>;

  /** List all missions (newest first) */
  list(): Promise<readonly Mission[]>;
}
