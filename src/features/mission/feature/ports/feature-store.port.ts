/**
 * Feature store port
 * Defines the interface for feature persistence operations
 * Features are stored as one JSON file per feature
 */
import type { Feature, CreateFeatureInput, UpdateFeatureInput } from "../domain/mission-types.js";

export interface FeatureStorePort {
  /** Get a feature by ID within a mission, returns undefined if not found */
  get(missionId: string, featureId: string): Promise<Feature | undefined>;

  /** Check if a feature exists in a mission */
  exists(missionId: string, featureId: string): Promise<boolean>;

  /** Create a new feature in a mission */
  create(missionId: string, input: CreateFeatureInput, id: string): Promise<Feature>;

  /** Update an existing feature */
  update(
    missionId: string,
    featureId: string,
    input: UpdateFeatureInput,
  ): Promise<Feature | undefined>;

  /** List all features for a mission */
  list(missionId: string): Promise<readonly Feature[]>;

  /** List features filtered by milestone and/or status */
  list(
    missionId: string,
    filter?: { milestoneId?: string; status?: string },
  ): Promise<readonly Feature[]>;

  /** Get multiple features by IDs */
  getMany(missionId: string, featureIds: readonly string[]): Promise<readonly Feature[]>;
}
