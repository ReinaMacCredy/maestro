import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { HandoffEnvelope } from "../domain/types.js";
import { MaestroError, handoffNotFound } from "../domain/errors.js";

export interface PickupOpts {
  readonly id?: string;
  readonly agent: string;
  readonly peek?: boolean;
}

export async function pickupHandoff(
  store: HandoffStorePort,
  opts: PickupOpts,
): Promise<HandoffEnvelope> {
  let envelope: HandoffEnvelope | undefined;

  if (opts.id) {
    envelope = await store.get(opts.id);
    if (!envelope) throw handoffNotFound(opts.id);
  } else {
    envelope = await store.getLatestPending();
    if (!envelope) {
      throw new MaestroError("No pending handoffs found", [
        "Create one first: maestro handoff --sitrep '...' --quickstart '...'",
      ]);
    }
  }

  if (envelope.status === "pending" && !opts.peek) {
    envelope = (await store.updateStatus(envelope.handoff.id, "picked-up", {
      pickedUpBy: opts.agent,
    }))!;
  }

  return envelope;
}

export async function listHandoffs(
  store: HandoffStorePort,
): Promise<readonly HandoffEnvelope[]> {
  return store.list();
}
