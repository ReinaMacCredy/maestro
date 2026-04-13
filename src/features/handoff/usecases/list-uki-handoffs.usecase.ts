/**
 * List UKI handoffs in the store, optionally filtered by status.
 *
 * Thin pass-through to the HandoffStorePort; kept as a usecase for
 * symmetry with create/pickup and to centralize any future filter
 * logic (e.g. pagination, time windowing).
 */
import type { UkiHandoff, UkiHandoffStatus } from "../domain/uki-types.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";

export interface ListUkiHandoffsOptions {
  readonly status?: UkiHandoffStatus;
}

export async function listUkiHandoffs(
  handoffStore: HandoffStorePort,
  opts: ListUkiHandoffsOptions = {},
): Promise<readonly UkiHandoff[]> {
  return handoffStore.list({ status: opts.status });
}
