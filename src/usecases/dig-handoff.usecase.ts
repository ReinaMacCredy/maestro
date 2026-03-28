import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { CassPort } from "../ports/cass.port.js";
import type { CassSearchResponse } from "../domain/types.js";
import { MaestroError } from "../domain/errors.js";

export interface DigOpts {
  readonly id?: string;
  readonly limit?: number;
}

export async function digHandoff(
  store: HandoffStorePort,
  cass: CassPort,
  query: string,
  opts: DigOpts,
): Promise<CassSearchResponse> {
  if (!(await cass.isAvailable())) {
    throw new MaestroError("CASS is not available", [
      "Install: brew install dicklesworthstone/tap/cass",
      "Then: cass index",
    ]);
  }

  // Find the handoff to scope the search
  let envelope;
  if (opts.id) {
    envelope = await store.get(opts.id);
    if (!envelope) {
      throw new MaestroError(`Handoff ${opts.id} not found`, [
        "List available handoffs: maestro handoff-pickup --list",
      ]);
    }
  } else {
    const all = await store.list();
    envelope = all[0];
    if (!envelope) {
      throw new MaestroError("No handoffs found to search", [
        "Create one first: maestro handoff --sitrep '...' --quickstart '...'",
      ]);
    }
  }

  const { session } = envelope.handoff;

  return cass.search(query, {
    agent: session.agent === "unknown" ? undefined : session.agent.replace("-", "_"),
    workspace: process.cwd(),
    limit: opts.limit,
  });
}
