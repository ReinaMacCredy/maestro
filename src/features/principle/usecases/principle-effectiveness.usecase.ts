/**
 * Principle effectiveness aggregator.
 *
 * Collapses the raw outcomes.jsonl into per-principle helpful/unhelpful
 * counts. Takes the latest record per (principleId, handoffId) pair so
 * a `completed -> kicked-back` correction supersedes the earlier signal.
 *
 * Effectiveness = helpful / (helpful + unhelpful). Pending outcomes are
 * counted separately; they do not dilute the ratio.
 */
import type {
  Principle,
  PrincipleEffectiveness,
  PrincipleOutcomeRecord,
} from "../domain/types.js";

export function buildPrincipleEffectiveness(
  principles: readonly Principle[],
  outcomes: readonly PrincipleOutcomeRecord[],
): ReadonlyMap<string, PrincipleEffectiveness> {
  // For each (principleId, handoffId) pair, keep only the newest record.
  const latestByPair = new Map<string, PrincipleOutcomeRecord>();
  for (const record of outcomes) {
    const key = `${record.principleId}::${record.handoffId}`;
    const existing = latestByPair.get(key);
    if (!existing || existing.recordedAt <= record.recordedAt) {
      latestByPair.set(key, record);
    }
  }

  const byPrinciple = new Map<string, { helpful: number; unhelpful: number; pending: number }>();
  for (const principle of principles) {
    byPrinciple.set(principle.id, { helpful: 0, unhelpful: 0, pending: 0 });
  }
  for (const record of latestByPair.values()) {
    // Tally outcomes even for principles that have since been removed -- the
    // data is still meaningful historical signal.
    const bucket = byPrinciple.get(record.principleId) ?? { helpful: 0, unhelpful: 0, pending: 0 };
    if (record.outcome === "helpful") bucket.helpful += 1;
    else if (record.outcome === "unhelpful") bucket.unhelpful += 1;
    else bucket.pending += 1;
    byPrinciple.set(record.principleId, bucket);
  }

  const result = new Map<string, PrincipleEffectiveness>();
  for (const [principleId, counts] of byPrinciple) {
    const decided = counts.helpful + counts.unhelpful;
    result.set(principleId, {
      principleId,
      helpful: counts.helpful,
      unhelpful: counts.unhelpful,
      pending: counts.pending,
      total: decided + counts.pending,
      effectiveness: decided > 0 ? counts.helpful / decided : undefined,
    });
  }
  return result;
}

/** Minimum decided-outcome count before the effectiveness ratio is considered signal, not noise. */
export const PRINCIPLE_SMALL_SAMPLE_THRESHOLD = 3;

export function hasSufficientSample(e: PrincipleEffectiveness): boolean {
  return (e.helpful + e.unhelpful) >= PRINCIPLE_SMALL_SAMPLE_THRESHOLD;
}
