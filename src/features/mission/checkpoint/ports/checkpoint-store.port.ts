/**
 * Checkpoint store port
 * Defines the interface for checkpoint persistence operations
 * Checkpoints are stored with timestamp-based filenames under checkpoints/
 */
import type { Checkpoint } from "../../domain/mission-types.js";

export interface CheckpointStorePort {
  /** Get a checkpoint by ID within a mission, returns undefined if not found */
  get(missionId: string, checkpointId: string): Promise<Checkpoint | undefined>;

  /** Save a new checkpoint for a mission, returns the checkpoint ID */
  save(
    missionId: string,
    data: Omit<Checkpoint, "id">,
  ): Promise<Checkpoint>;

  /** List all checkpoints for a mission, sorted newest first */
  list(missionId: string): Promise<readonly Checkpoint[]>;

  /** Get the latest checkpoint for a mission, returns undefined if none exist */
  getLatest(missionId: string): Promise<Checkpoint | undefined>;

  /** Load the latest checkpoint (alias for getLatest with clearer intent) */
  load(missionId: string): Promise<Checkpoint | undefined>;
}
