import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { HandoffEnvelope } from "../domain/types.js";
import { MaestroError, handoffNotFound } from "../domain/errors.js";

export interface ReportOpts {
  readonly id?: string;
  readonly content: string;
}

export async function reportHandoff(
  store: HandoffStorePort,
  opts: ReportOpts,
): Promise<HandoffEnvelope> {
  let envelope: HandoffEnvelope | undefined;

  if (opts.id) {
    envelope = await store.get(opts.id);
    if (!envelope) throw handoffNotFound(opts.id);
  } else {
    const all = await store.list({ status: "picked-up" });
    envelope = all[0];
    if (!envelope) {
      throw new MaestroError("No picked-up handoffs to report on", [
        "Pick up a handoff first: maestro handoff-pickup --claim --agent <name>",
      ]);
    }
  }

  return (await store.updateStatus(envelope.handoff.id, "completed", {
    completedAt: new Date().toISOString(),
    report: opts.content,
  }))!;
}
