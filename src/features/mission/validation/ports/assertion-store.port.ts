/**
 * Assertion store port
 * Defines the interface for assertion persistence operations
 * Assertions are stored in a single assertions.json file per mission
 */
import type { Assertion, CreateAssertionInput, UpdateAssertionInput } from "../domain/mission-types.js";

export interface AssertionStorePort {
  /** Get an assertion by ID within a mission, returns undefined if not found */
  get(missionId: string, assertionId: string): Promise<Assertion | undefined>;

  /** Check if an assertion exists in a mission */
  exists(missionId: string, assertionId: string): Promise<boolean>;

  /** Create a new assertion in a mission */
  create(missionId: string, input: CreateAssertionInput, id: string): Promise<Assertion>;

  /** Update an existing assertion */
  update(
    missionId: string,
    assertionId: string,
    input: UpdateAssertionInput,
  ): Promise<Assertion | undefined>;

  /** List all assertions for a mission */
  list(missionId: string): Promise<readonly Assertion[]>;

  /** List assertions filtered by milestone */
  listByMilestone(missionId: string, milestoneId: string): Promise<readonly Assertion[]>;

  /** Get multiple assertions by IDs */
  getMany(missionId: string, assertionIds: readonly string[]): Promise<readonly Assertion[]>;
}
