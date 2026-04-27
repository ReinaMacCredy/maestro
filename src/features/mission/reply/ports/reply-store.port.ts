/**
 * Reply store port.
 *
 * Persists `.maestro/replies/<mission-id>/<feature-id>.yaml` files.
 * Malformed files are tolerated on read (logged and skipped) so one bad
 * reply does not poison the inbox.
 */
import type { AgentReply } from "../domain/reply-types.js";

export interface ReplyStorePort {
  /** List every valid reply on disk. Malformed files are skipped. */
  list(): Promise<readonly AgentReply[]>;

  /** Fetch a reply by mission+feature id, or undefined when missing or malformed. */
  get(missionId: string, featureId: string): Promise<AgentReply | undefined>;

  /** List replies whose `writtenAt` is greater than or equal to the ISO cutoff. */
  listSince(isoTimestamp: string): Promise<readonly AgentReply[]>;

  /** Write (or overwrite) the reply for a feature. Atomic rename. */
  write(reply: AgentReply): Promise<void>;

  /** True when the reply has already been ingested (sidecar marker present). */
  isIngested(missionId: string, featureId: string): Promise<boolean>;

  /** Mark a reply as ingested by creating the sidecar marker. Idempotent. */
  markIngested(missionId: string, featureId: string): Promise<void>;
}
