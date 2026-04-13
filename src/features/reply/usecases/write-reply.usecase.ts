/**
 * Write a reply to disk. The CLI and agent paths both land here -- they
 * differ only in `writtenBy` ("human" for CLI, "agent" for prompt-driven).
 *
 * No cross-feature lookups -- this is a pure write. Ingest (Sprint 2)
 * handles feature advance and principle outcome recording.
 */
import type { WorkerReport } from "@/features/mission/index.js";
import type { ReplyStorePort } from "../ports/reply-store.port.js";
import type { ReplyAuthor, ReplyOutcome, WorkerReply } from "../domain/reply-types.js";
import { clearIngestedMarker } from "../adapters/fs-reply-store.adapter.js";

export interface WriteReplyInput {
  readonly featureId: string;
  readonly outcome: ReplyOutcome;
  readonly report?: WorkerReport;
  readonly notes?: string;
  readonly writtenBy?: ReplyAuthor;
  readonly source?: string;
  /** Override timestamp for tests. Defaults to now(). */
  readonly writtenAt?: string;
  /** Project root for invalidating the `.ingested` sidecar on overwrite. */
  readonly projectDir?: string;
}

export async function writeWorkerReply(
  store: ReplyStorePort,
  input: WriteReplyInput,
): Promise<WorkerReply> {
  const reply: WorkerReply = {
    featureId: input.featureId,
    outcome: input.outcome,
    report: input.report,
    notes: input.notes,
    writtenAt: input.writtenAt ?? new Date().toISOString(),
    writtenBy: input.writtenBy ?? "human",
    source: input.source,
  };
  await store.write(reply);
  // Overwriting a reply invalidates any prior ingestion marker so the next
  // snapshot poll re-runs ingest against the fresh content.
  if (input.projectDir) {
    await clearIngestedMarker(input.projectDir, input.featureId);
  }
  return reply;
}
