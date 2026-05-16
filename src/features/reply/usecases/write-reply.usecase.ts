/**
 * Write a reply to disk. The CLI and agent paths both land here -- they
 * differ only in `writtenBy` ("human" for CLI, "agent" for prompt-driven).
 *
 * No cross-feature lookups -- this is a pure write. The snapshot ingest
 * path handles feature advance and principle outcome recording on the
 * next poll cycle. The adapter's `write()` invalidates any prior
 * `.ingested` sidecar so overwrites re-run ingest automatically.
 */
import type { AgentReport } from "@/shared/domain/legacy-mission";
import type { ReplyStorePort } from "../ports/reply-store.port.js";
import type { ReplyAuthor, ReplyOutcome, AgentReply } from "../domain/types.js";

export interface WriteReplyInput {
  readonly missionId: string;
  readonly featureId: string;
  readonly outcome: ReplyOutcome;
  readonly report?: AgentReport;
  readonly notes?: string;
  readonly writtenBy?: ReplyAuthor;
  readonly source?: string;
  /** Override timestamp for tests. Defaults to now(). */
  readonly writtenAt?: string;
}

export async function writeAgentReply(
  store: ReplyStorePort,
  input: WriteReplyInput,
): Promise<AgentReply> {
  const reply: AgentReply = {
    missionId: input.missionId,
    featureId: input.featureId,
    outcome: input.outcome,
    report: input.report,
    notes: input.notes,
    writtenAt: input.writtenAt ?? new Date().toISOString(),
    writtenBy: input.writtenBy ?? "human",
    source: input.source,
  };
  await store.write(reply);
  return reply;
}
