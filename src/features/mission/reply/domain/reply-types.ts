/**
 * Reply feature domain types.
 *
 * A reply is the agent's (or human's) inbound half of the agent contract:
 * after maestro publishes an agent prompt, the agent writes
 * `.maestro/replies/<mission-id>/<feature-id>.yaml` to signal completion,
 * kickback, or abandonment. Ingest then advances feature state and records
 * principle outcomes.
 */
import type { AgentReport } from "@/shared/domain/legacy-mission";

/** How the caller claims the work resolved. Cross-checked against objective state on ingest. */
export const REPLY_OUTCOMES = ["completed", "kicked-back", "abandoned"] as const;
export type ReplyOutcome = (typeof REPLY_OUTCOMES)[number];

/** Who wrote the reply. "agent" comes from agent-driven paths; "human" from the CLI override. */
export type ReplyAuthor = "agent" | "human";

/** Canonical reply record persisted at `.maestro/replies/<mission-id>/<feature-id>.yaml`. */
export interface AgentReply {
  readonly missionId: string;
  readonly featureId: string;
  readonly outcome: ReplyOutcome;
  readonly report?: AgentReport;
  readonly notes?: string;
  readonly writtenAt: string;
  readonly writtenBy: ReplyAuthor;
  /** Optional free-form origin marker, e.g. "cli", "mcp", "agent:claude". */
  readonly source?: string;
}

/** Outcome of ingesting a reply: what maestro did in response. */
export interface ReplyIngestResult {
  readonly reply: AgentReply;
  readonly featureAdvanced: boolean;
  readonly principlesRecorded: number;
  readonly kickedBack: boolean;
  /** Human-readable explanation when declared outcome was downgraded (e.g. completed -> kicked-back on failing assertions). */
  readonly downgradeReason?: string;
}
