/**
 * Pick up an existing UKI handoff, optionally transitioning it from
 * `pending` to `picked-up` in the same call.
 *
 * If an id is supplied, fetches that specific handoff. Otherwise fetches
 * the latest pending handoff in the store. Throws a MaestroError with
 * actionable hints when there is nothing to pick up.
 */
import { MaestroError } from "../domain/errors.js";
import type { UkiHandoff } from "../domain/uki-types.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";

export interface PickupUkiHandoffOptions {
  readonly id?: string;
  readonly claim?: boolean;
  readonly pickedUpBy?: string;
}

export async function pickupUkiHandoff(
  handoffStore: HandoffStorePort,
  opts: PickupUkiHandoffOptions = {},
): Promise<UkiHandoff> {
  if (opts.claim) {
    const claimed = await handoffStore.claimPending(opts.id, opts.pickedUpBy);
    if (claimed) {
      return claimed;
    }
  }

  let handoff: UkiHandoff | undefined;
  if (opts.id) {
    handoff = await handoffStore.get(opts.id);
    if (!handoff) {
      throw new MaestroError(`Handoff ${opts.id} not found`, [
        "List handoffs: maestro handoff list",
        `Check that id '${opts.id}' is correct`,
      ]);
    }
  } else {
    handoff = await handoffStore.getLatestPending();
    if (!handoff) {
      throw new MaestroError("No pending handoffs to pick up", [
        "Create one: maestro handoff create --session-core '...' --summary '...' --next-action '...' --artifact 'branch_<name>' --confidence-work 0.9",
        "Or list all: maestro handoff list",
      ]);
    }
  }

  return handoff;
}
