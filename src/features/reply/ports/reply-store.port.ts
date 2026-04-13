/**
 * Reply store port.
 *
 * Persists `.maestro/replies/<feature-id>.yaml` files, one per feature.
 * Malformed files are tolerated on read (logged and skipped) so one bad
 * reply does not poison the inbox.
 */
import type { WorkerReply } from "../domain/reply-types.js";

export interface ReplyStorePort {
  /** List every valid reply on disk. Malformed files are skipped. */
  list(): Promise<readonly WorkerReply[]>;

  /** Fetch a reply by feature id, or undefined when missing or malformed. */
  get(featureId: string): Promise<WorkerReply | undefined>;

  /** List replies whose `writtenAt` is greater than or equal to the ISO cutoff. */
  listSince(isoTimestamp: string): Promise<readonly WorkerReply[]>;

  /** Write (or overwrite) the reply for a feature. Atomic rename. */
  write(reply: WorkerReply): Promise<void>;

  /** True when the reply has already been ingested (sidecar marker present). */
  isIngested(featureId: string): Promise<boolean>;

  /** Mark a reply as ingested by creating the sidecar marker. Idempotent. */
  markIngested(featureId: string): Promise<void>;
}
