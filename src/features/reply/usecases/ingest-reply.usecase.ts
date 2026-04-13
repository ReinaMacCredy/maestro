/**
 * Ingest a reply: match it to a feature, advance feature state when the
 * claimed outcome is consistent with objective reality (assertions), and
 * record principle outcomes for the handoff that gated this feature.
 *
 * Objective-first inference: `completed` is accepted only when feature is
 * in `review` with all assertions passed or waived. Any mismatch downgrades
 * the claim to a kickback and records the discrepancy in the result's
 * `downgradeReason`.
 *
 * Idempotent: `replyStore.isIngested(featureId)` short-circuits repeated
 * polls. Callers should wrap in try/catch; this usecase never mutates on
 * parse/validation failures.
 */
import type {
  AssertionStorePort,
  Feature,
  FeatureStatus,
  MissionStorePort,
  FeatureStorePort,
  UpdateFeatureInput,
} from "@/features/mission/index.js";
import { canTransitionFeature, updateFeature } from "@/features/mission/index.js";
import type { ReplyStorePort } from "../ports/reply-store.port.js";
import type {
  ReplyIngestResult,
  ReplyOutcome,
  WorkerReply,
} from "../domain/reply-types.js";

/**
 * Optional hook for recording principle outcomes attributable to this
 * reply. Kept out of the store dependency list so the ingest usecase does
 * not have to know the principle-store shape; the composition root wires
 * the recorder in when it has the store and handoff data to support it.
 */
export type PrincipleOutcomeRecorder = (
  featureId: string,
  outcome: ReplyOutcome,
) => Promise<number>;

export interface IngestReplyDeps {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly replyStore: ReplyStorePort;
  readonly baseDir: string;
  readonly recordPrincipleOutcomes?: PrincipleOutcomeRecorder;
}

export async function ingestReply(
  deps: IngestReplyDeps,
  missionId: string,
  featureId: string,
): Promise<ReplyIngestResult | undefined> {
  const reply = await deps.replyStore.get(featureId);
  if (!reply) return undefined;
  if (await deps.replyStore.isIngested(featureId)) {
    return {
      reply,
      featureAdvanced: false,
      principlesRecorded: 0,
      kickedBack: reply.outcome !== "completed",
    };
  }

  const feature = await deps.featureStore.get(missionId, featureId);
  if (!feature) {
    // Reply exists but no feature. Mark as ingested so we do not spin on
    // every poll. Caller can delete the reply or fix the reference.
    await deps.replyStore.markIngested(featureId);
    return {
      reply,
      featureAdvanced: false,
      principlesRecorded: 0,
      kickedBack: false,
      downgradeReason: `Feature ${featureId} not found in mission ${missionId}`,
    };
  }

  const assertions = await deps.assertionStore.list(missionId);
  const featureAssertions = assertions.filter((a) => a.featureId === featureId);
  const assertionsPass = featureAssertions.every(
    (a) => a.result === "passed" || a.result === "waived",
  );

  const inferred = inferEffectiveOutcome(reply, feature, assertionsPass);
  const advance = await applyOutcomeTransition(deps, missionId, feature, reply, inferred);

  const principlesRecorded = deps.recordPrincipleOutcomes
    ? await deps.recordPrincipleOutcomes(featureId, inferred.outcome)
    : 0;

  await deps.replyStore.markIngested(featureId);

  return {
    reply,
    featureAdvanced: advance.advanced,
    principlesRecorded,
    kickedBack: inferred.outcome !== "completed",
    downgradeReason: inferred.downgradeReason,
  };
}

interface InferredOutcome {
  readonly outcome: ReplyOutcome;
  readonly downgradeReason?: string;
}

function inferEffectiveOutcome(
  reply: WorkerReply,
  feature: Feature,
  assertionsPass: boolean,
): InferredOutcome {
  if (reply.outcome === "completed") {
    if (feature.status !== "review") {
      return {
        outcome: "kicked-back",
        downgradeReason: `Claimed 'completed' but feature is '${feature.status}', not 'review'`,
      };
    }
    if (!assertionsPass) {
      return {
        outcome: "kicked-back",
        downgradeReason: "Claimed 'completed' but assertions have not all passed or been waived",
      };
    }
  }
  return { outcome: reply.outcome };
}

/** Translate an outcome into the target feature status. */
function targetStatusFor(outcome: ReplyOutcome): FeatureStatus {
  if (outcome === "completed") return "done";
  if (outcome === "kicked-back") return "pending";
  return "blocked"; // abandoned
}

async function applyOutcomeTransition(
  deps: IngestReplyDeps,
  missionId: string,
  feature: Feature,
  reply: WorkerReply,
  inferred: InferredOutcome,
): Promise<{ advanced: boolean }> {
  const target = targetStatusFor(inferred.outcome);
  if (target === feature.status) return { advanced: false };
  if (!canTransitionFeature(feature.status, target)) {
    // Some transitions require an intermediate hop (e.g. in-progress -> review).
    // If the agent pushed review -> pending (retry), we can do it in one step.
    // If they pushed in-progress -> done, we can't -- the feature must first be
    // moved to review by whoever is reviewing. Leave state alone and let the
    // human/CLI operator resolve.
    return { advanced: false };
  }

  const patch: UpdateFeatureInput = {
    status: target,
    report: reply.report,
  };
  if (inferred.outcome === "kicked-back" && target === "pending") {
    const reasonParts = [
      reply.notes ?? "",
      inferred.downgradeReason ?? "",
    ].filter((s) => s.length > 0);
    if (reasonParts.length > 0) {
      (patch as { retryReason?: string }).retryReason = reasonParts.join(" | ");
    }
  }

  await updateFeature(
    deps.missionStore,
    deps.featureStore,
    deps.baseDir,
    missionId,
    feature.id,
    patch,
  );
  return { advanced: true };
}

