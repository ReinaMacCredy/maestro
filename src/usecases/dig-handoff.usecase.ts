import { existsSync } from "node:fs";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { CassPort } from "../ports/cass.port.js";
import type { CassSearchResponse } from "../domain/types.js";
import { MaestroError } from "../domain/errors.js";
import { CASS_INSTALL_HINT, MAESTRO_DIR } from "../domain/defaults.js";
import { warn } from "../lib/output.js";
import { writeText } from "../lib/fs.js";
import { join } from "node:path";

export interface DigOpts {
  readonly id?: string;
  readonly limit?: number;
  readonly dir: string;
}

export async function digHandoff(
  store: HandoffStorePort,
  cass: CassPort,
  query: string,
  opts: DigOpts,
): Promise<CassSearchResponse> {
  if (!(await cass.hasBinary())) {
    throw new MaestroError("CASS binary not found", [
      CASS_INSTALL_HINT,
    ]);
  }

  let envelope;
  if (opts.id) {
    envelope = await store.get(opts.id);
    if (!envelope) {
      throw new MaestroError(`Handoff ${opts.id} not found`, [
        "List handoffs: maestro handoff --list",
      ]);
    }
  } else {
    const all = await store.list();
    envelope = all[0];
    if (!envelope) {
      throw new MaestroError("No handoffs found to search", [
        "Create one first: maestro handoff",
      ]);
    }
  }

  const { session } = envelope.handoff;

  // Lazy indexing with sentinel to avoid re-indexing on repeated digs
  if (session.sourcePath) {
    const sentinel = join(opts.dir, MAESTRO_DIR, "handoffs", envelope.handoff.id, ".cass-indexed");
    if (!existsSync(sentinel)) {
      try {
        await cass.indexOnce([session.sourcePath]);
        await writeText(sentinel, new Date().toISOString());
      } catch {
        warn("CASS indexing failed, searching with existing index");
      }
    }
  }

  return cass.search(query, {
    agent: session.agent === "unknown" ? undefined : session.agent.replace("-", "_"),
    workspace: opts.dir,
    limit: opts.limit,
  });
}
