export type {
  AgentReply,
  ReplyOutcome,
  ReplyAuthor,
  ReplyIngestResult,
} from "./domain/reply-types.js";
export { REPLY_OUTCOMES } from "./domain/reply-types.js";
export { validateAgentReply } from "./domain/reply-validators.js";

export type { ReplyStorePort } from "./ports/reply-store.port.js";
export { FsReplyStoreAdapter } from "./adapters/fs-reply-store.adapter.js";

export {
  writeAgentReply,
  type WriteReplyInput,
} from "./usecases/write-reply.usecase.js";

export {
  ingestReply,
  type IngestReplyDeps,
  type PrincipleOutcomeRecorder,
} from "./usecases/ingest-reply.usecase.js";

export { registerReplyCommand } from "./commands/reply.command.js";
export { buildReplyServices } from "./services.js";
export type { ReplyServices } from "./services.js";
