import type { CorrectionStorePort } from "@/features/memory";
import type { RatchetAssertion } from "../domain/types.js";
import type { RatchetStorePort } from "../ports/ratchet-store.port.js";

export interface PromoteOpts {
  readonly correctionId: string;
  readonly check: string;
}

export interface PromoteResult {
  readonly assertion: RatchetAssertion;
}

export async function promoteToRatchet(
  corrStore: CorrectionStorePort,
  ratchetStore: RatchetStorePort,
  opts: PromoteOpts,
): Promise<PromoteResult> {
  const correction = await corrStore.get(opts.correctionId);
  if (!correction) {
    throw new Error(`Correction not found: ${opts.correctionId}`);
  }

  const suite = await ratchetStore.getSuite();
  const now = new Date().toISOString();
  const id = `ratchet-${correction.id}`;

  const assertion: RatchetAssertion = {
    id,
    correctionId: correction.id,
    rule: correction.rule,
    check: opts.check,
    createdAt: now,
  };

  await ratchetStore.writeSuite({
    assertions: [...suite.assertions, assertion],
  });

  await corrStore.update(correction.id, { promotedToRatchet: now });

  return { assertion };
}
