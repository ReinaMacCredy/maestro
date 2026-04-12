import type { Principle, CreatePrincipleInput } from "../domain/principle-types.js";
import type { MilestoneProfile } from "../domain/mission-types.js";

export interface PrincipleStorePort {
  /** Return all principles in the store. */
  list(): Promise<readonly Principle[]>;

  /** Return principles that apply to a given milestone profile. */
  listByProfile(profile: MilestoneProfile): Promise<readonly Principle[]>;

  /** Get a single principle by id. Returns undefined if not found. */
  get(id: string): Promise<Principle | undefined>;

  /** Create a new principle. Throws if id already exists. */
  create(input: CreatePrincipleInput): Promise<Principle>;

  /** Remove a principle by id. Returns true if removed, false if not found. */
  remove(id: string): Promise<boolean>;
}
