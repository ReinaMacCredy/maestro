import type {
  AssertionStorePort,
  Feature,
  FeatureStorePort,
  MissionStorePort,
} from "@/shared/domain/legacy-mission";
import type {
  Principle,
  PrincipleOutcomeRecord,
  PrincipleStorePort,
} from "@/features/principle";
import {
  buildPrincipleEffectiveness,
  PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
} from "@/features/principle";
import {
  ingestReply,
  type AgentReply,
  type ReplyStorePort,
} from "@/features/reply";
import type {
  PrincipleEffectivenessRow,
  ReplyInboxEntry,
} from "./screen-types.js";

export interface IngestResult {
  readonly replies: readonly AgentReply[];
  /** Cached outcomes (plus any appends from this ingest pass) for downstream aggregators. */
  readonly outcomesCache?: readonly PrincipleOutcomeRecord[];
  /** Always undefined now that handoff scoping is gone; retained for shape compatibility. */
  readonly handoffsCache?: undefined;
}

interface ReplyProjectionDeps {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly replyStore?: ReplyStorePort;
  readonly principleStore?: PrincipleStorePort;
  readonly cwd: string;
}

interface PrincipleEffectivenessDeps {
  readonly principleStore?: PrincipleStorePort;
}

export async function loadAndIngestReplies(
  deps: ReplyProjectionDeps,
  missionId: string,
  _currentProjectRoot: string,
): Promise<IngestResult> {
  if (!deps.replyStore) return { replies: [] };
  try {
    const replies = (await deps.replyStore.list()).filter((reply) => reply.missionId === missionId);
    if (replies.length === 0) return { replies: [] };

    const outcomesCache: PrincipleOutcomeRecord[] | undefined = deps.principleStore
      ? [...(await deps.principleStore.listOutcomes())]
      : undefined;

    for (const reply of replies) {
      if (await deps.replyStore.isIngested(missionId, reply.featureId)) continue;
      try {
        await ingestReply(
          {
            missionStore: deps.missionStore,
            featureStore: deps.featureStore,
            assertionStore: deps.assertionStore,
            replyStore: deps.replyStore,
            baseDir: deps.cwd,
          },
          missionId,
          reply.featureId,
        );
      } catch {
        // Snapshot projection must not throw on a single bad reply.
      }
    }

    return { replies, outcomesCache };
  } catch {
    return { replies: [] };
  }
}

export async function loadPrincipleEffectiveness(
  deps: PrincipleEffectivenessDeps,
  _currentProjectRoot: string,
  cachedOutcomes?: readonly PrincipleOutcomeRecord[],
  _cachedHandoffs?: undefined,
): Promise<readonly PrincipleEffectivenessRow[] | undefined> {
  const principleStore = deps.principleStore;
  if (!principleStore) return undefined;
  try {
    const [principles, outcomes] = await Promise.all([
      principleStore.list(),
      cachedOutcomes !== undefined
        ? Promise.resolve(cachedOutcomes)
        : principleStore.listOutcomes(),
    ]);
    return buildPrincipleEffectivenessRows(principles, outcomes);
  } catch {
    return undefined;
  }
}

export async function safeListReplies(
  replyStore: ReplyStorePort,
): Promise<readonly AgentReply[]> {
  try {
    return await replyStore.list();
  } catch {
    return [];
  }
}

export function buildPrincipleEffectivenessRows(
  principles: readonly Principle[],
  outcomes: readonly PrincipleOutcomeRecord[],
): readonly PrincipleEffectivenessRow[] {
  const rollup = buildPrincipleEffectiveness(principles, outcomes);
  const principleById = new Map(principles.map((principle) => [principle.id, principle]));

  const unhelpfulByPrinciple = new Map<string, PrincipleOutcomeRecord[]>();
  for (const record of [...outcomes].sort((a, b) => b.recordedAt.localeCompare(a.recordedAt))) {
    if (record.outcome !== "unhelpful") continue;
    const bucket = unhelpfulByPrinciple.get(record.principleId) ?? [];
    if (bucket.length < 3) bucket.push(record);
    unhelpfulByPrinciple.set(record.principleId, bucket);
  }

  const rows: PrincipleEffectivenessRow[] = [];
  for (const stats of rollup.values()) {
    const principle = principleById.get(stats.principleId);
    const decided = stats.helpful + stats.unhelpful;
    const examples = (unhelpfulByPrinciple.get(stats.principleId) ?? [])
      .map((record) => `${record.handoffId}: ${record.handoffId}`);

    rows.push({
      id: stats.principleId,
      name: principle?.name ?? stats.principleId,
      mode: principle?.mode ?? "advisory",
      helpful: stats.helpful,
      unhelpful: stats.unhelpful,
      pending: stats.pending,
      total: stats.total,
      effectivenessPct: stats.effectiveness === undefined
        ? undefined
        : Math.round(stats.effectiveness * 100),
      lowSample: decided < PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
      recentKickbackExamples: examples,
    });
  }

  rows.sort((left, right) => {
    const leftEffectiveness = left.effectivenessPct ?? 101;
    const rightEffectiveness = right.effectivenessPct ?? 101;
    if (leftEffectiveness !== rightEffectiveness) return leftEffectiveness - rightEffectiveness;
    const leftDecided = left.helpful + left.unhelpful;
    const rightDecided = right.helpful + right.unhelpful;
    return rightDecided - leftDecided;
  });
  return rows;
}

export function buildReplyInbox(
  features: readonly Feature[],
  replies: readonly AgentReply[],
): readonly ReplyInboxEntry[] {
  const featureById = new Map(features.map((feature) => [feature.id, feature]));
  const entries: ReplyInboxEntry[] = replies.map((reply) => {
    const feature = featureById.get(reply.featureId);
    return {
      featureId: reply.featureId,
      outcome: reply.outcome,
      writtenAt: reply.writtenAt,
      writtenBy: reply.writtenBy,
      featureTitle: feature?.title,
      featureStatus: feature?.status,
      pending: isReplyPending(reply, feature),
      notes: reply.notes,
    };
  });
  entries.sort((left, right) => right.writtenAt.localeCompare(left.writtenAt));
  return entries;
}

function isReplyPending(reply: AgentReply, feature: Feature | undefined): boolean {
  if (!feature) return true;
  if (reply.outcome === "completed") return feature.status !== "done";
  if (reply.outcome === "abandoned") return feature.status !== "blocked";
  return feature.status !== "pending";
}
