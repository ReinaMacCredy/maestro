import type { Handoff, HandoffEnvelope, HandoffStatus } from "../domain/types.js";

export interface HandoffStorePort {
  create(handoff: Handoff): Promise<string>;
  get(id: string): Promise<HandoffEnvelope | undefined>;
  getLatestPending(): Promise<HandoffEnvelope | undefined>;
  listIds(): Promise<readonly string[]>;
  list(filter?: { status?: HandoffStatus }): Promise<readonly HandoffEnvelope[]>;
  updateStatus(
    id: string,
    status: HandoffStatus,
    meta?: { pickedUpBy?: string; completedAt?: string; report?: string },
  ): Promise<HandoffEnvelope | undefined>;
}
