import type { ReplyStorePort } from "./ports/reply-store.port.js";
import { FsReplyStoreAdapter } from "./adapters/fs-reply-store.adapter.js";

export interface ReplyServices {
  readonly replyStore: ReplyStorePort;
}

export function buildReplyServices(projectDir: string): ReplyServices {
  return {
    replyStore: new FsReplyStoreAdapter(projectDir),
  };
}
