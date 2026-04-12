/**
 * Conductor Replay: extract prior session insights from handoff history.
 *
 * When a feature has prior handoffs (matched by mission + feature refs),
 * this usecase projects the replay-worthy fields into a compact summary
 * that the worker prompt generator injects as context.
 */

import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type {
  UkiHandoff,
  UkiHandoffMode,
  UkiVerificationResult,
} from "../domain/uki-types.js";
import { normalizeUkiToken } from "../lib/uki-token.js";

const MAX_REPLAY_HANDOFFS = 3;

export interface PriorSessionSummary {
  readonly handoffId: string;
  readonly timestamp: string;
  readonly mode: UkiHandoffMode;
  readonly summary: string;
  readonly risks: readonly string[];
  readonly divergences: readonly string[];
  readonly causalDrivers: readonly string[];
  readonly blindSpot?: string;
  readonly assumptions?: readonly string[];
  readonly verificationResults?: readonly UkiVerificationResult[];
  readonly completedWork?: readonly string[];
}

/**
 * Load the most recent prior handoffs for a feature, projected to
 * replay-worthy fields. Returns undefined when no useful handoffs exist.
 *
 * Handoffs with all replay fields empty are skipped -- they add no value.
 * Result is capped at MAX_REPLAY_HANDOFFS (3), newest first.
 */
export async function loadPriorHandoffs(
  handoffStore: HandoffStorePort,
  missionId: string,
  featureId: string,
): Promise<readonly PriorSessionSummary[] | undefined> {
  const all = handoffStore.listRecentByFeatureRefs
    ? await handoffStore.listRecentByFeatureRefs(missionId, featureId, MAX_REPLAY_HANDOFFS)
    : await handoffStore.list();
  const normalizedMissionId = normalizeUkiToken(missionId);

  const summaries: PriorSessionSummary[] = [];
  for (const handoff of all) {
    if (summaries.length >= MAX_REPLAY_HANDOFFS) break;
    const refs = handoff.content.maestroRefs;
    if (!matchesFeatureRefs(refs.missionId, normalizedMissionId, featureId, refs.featureId)) continue;

    const summary = extractReplaySummary(handoff);
    if (summary) summaries.push(summary);
  }

  return summaries.length > 0 ? summaries : undefined;
}

function matchesFeatureRefs(
  candidateMissionId: string | undefined,
  normalizedMissionId: string,
  expectedFeatureId: string,
  candidateFeatureId: string | undefined,
): boolean {
  if (!candidateMissionId || candidateFeatureId !== expectedFeatureId) {
    return false;
  }

  return (
    candidateMissionId === normalizedMissionId
    || normalizeUkiToken(candidateMissionId) === normalizedMissionId
  );
}

function extractReplaySummary(handoff: UkiHandoff): PriorSessionSummary | undefined {
  const { content } = handoff;

  const completedWork = content.mode === "execute" ? content.completedWork : undefined;

  const hasReplayContent =
    content.risks.length > 0 ||
    content.divergences.length > 0 ||
    content.causalDrivers.length > 0 ||
    content.blindSpot !== undefined ||
    (content.assumptions !== undefined && content.assumptions.length > 0) ||
    (content.verificationResults !== undefined && content.verificationResults.length > 0) ||
    (completedWork !== undefined && completedWork.length > 0);

  if (!hasReplayContent) return undefined;

  return {
    handoffId: handoff.id,
    timestamp: handoff.timestamp,
    mode: content.mode,
    summary: content.summary,
    risks: content.risks,
    divergences: content.divergences,
    causalDrivers: content.causalDrivers,
    blindSpot: content.blindSpot,
    assumptions: content.assumptions,
    verificationResults: content.verificationResults,
    completedWork,
  };
}
