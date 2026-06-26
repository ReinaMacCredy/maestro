export type ReplyOutcome = "accepted" | "rejected" | "needs_changes";
export type ReplyAuthor = "agent" | "human";

export interface AgentReply {
  readonly featureId: string;
  readonly outcome: ReplyOutcome;
  readonly writtenAt: string;
  readonly writtenBy: ReplyAuthor;
  readonly notes?: string;
}

export interface ReplyStorePort {
  readonly list?: () => Promise<readonly AgentReply[]>;
}
