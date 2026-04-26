import type {
  AssertionStorePort,
  Feature,
  FeatureStorePort,
  MissionStorePort,
  Principle,
  PrincipleOutcomeRecord,
  PrincipleStorePort,
} from "@/features/mission";
import {
  buildPrincipleEffectiveness,
  PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
} from "@/features/mission";
import { isHandoffInProject, type HandoffRecord, type HandoffStorePort } from "@/features/handoff";
import { ingestReply, type AgentReply, type ReplyOutcome, type ReplyStorePort } from "@/features/reply";
import type {
  PrincipleEffectivenessRow,
  ReplyInboxEntry,
} from "./screen-types.js";

export interface IngestResult {
  readonly replies: readonly AgentReply[];
  /** Cached outcomes (plus any appends from this ingest pass) for downstream aggregators. */
  readonly outcomesCache?: readonly PrincipleOutcomeRecord[];
  /** Cached handoff list for downstream aggregators to avoid a second global scan. */
  readonly handoffsCache?: readonly HandoffRecord[];
}

interface ReplyProjectionDeps {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly handoffStore?: HandoffStorePort;
  readonly replyStore?: ReplyStorePort;
  readonly principleStore?: PrincipleStorePort;
  readonly cwd: string;
}

interface PrincipleEffectivenessDeps {
  readonly handoffStore?: HandoffStorePort;
  readonly principleStore?: PrincipleStorePort;
}

export async function loadAndIngestReplies(
  deps: ReplyProjectionDeps,
  missionId: string,
  currentProjectRoot: string,
): Promise<IngestResult> {
  if (!deps.replyStore) return { replies: [] };
  try {
    const replies = (await deps.replyStore.list()).filter((reply) => reply.missionId === missionId);
    if (replies.length === 0) return { replies: [] };

    const outcomesCache: PrincipleOutcomeRecord[] | undefined = deps.principleStore
      ? [...(await deps.principleStore.listOutcomes())]
      : undefined;
    const handoffsCache: readonly HandoffRecord[] | undefined = deps.handoffStore
      ? filterHandoffsForProject(await deps.handoffStore.list(), currentProjectRoot)
      : undefined;

    const recordPrincipleOutcomes = buildPrincipleRecorder(deps, missionId, outcomesCache, handoffsCache);

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
            ...(recordPrincipleOutcomes ? { recordPrincipleOutcomes } : {}),
          },
          missionId,
          reply.featureId,
        );
      } catch {
        // Snapshot projection must not throw on a single bad reply.
      }
    }

    return { replies, outcomesCache, ...(handoffsCache ? { handoffsCache } : {}) };
  } catch {
    return { replies: [] };
  }
}

function buildPrincipleRecorder(
  deps: ReplyProjectionDeps,
  missionId: string,
  outcomesCache: PrincipleOutcomeRecord[] | undefined,
  handoffsCache: readonly HandoffRecord[] | undefined,
): ((featureId: string, outcome: ReplyOutcome) => Promise<{ recorded: number; complete: boolean }>) | undefined {
  const principleStore = deps.principleStore;
  if (!principleStore || !handoffsCache || !outcomesCache) return undefined;

  return async (featureId, outcome) => {
    const resolved = outcome === "completed" ? "helpful" : "unhelpful";
    try {
      const recentHandoffs = handoffsCache
        .filter((handoff) => handoff.refs.missionId === missionId && handoff.refs.featureId === featureId)
        .slice(0, 25);
      if (recentHandoffs.length === 0) {
        return { recorded: 0, complete: true };
      }

      let recorded = 0;
      let complete = true;
      const recordedAt = new Date().toISOString();
      for (const handoff of recentHandoffs) {
        const pending = filterPendingForHandoff(outcomesCache, handoff.id);
        for (const row of pending) {
          const record: PrincipleOutcomeRecord = {
            principleId: row.principleId,
            handoffId: handoff.id,
            featureId,
            missionId,
            outcome: resolved,
            recordedAt,
          };
          if (await principleStore.recordOutcome(record)) {
            outcomesCache.push(record);
            recorded += 1;
            continue;
          }
          complete = false;
        }
      }
      return { recorded, complete };
    } catch {
      return { recorded: 0, complete: false };
    }
  };
}

function filterPendingForHandoff(
  outcomes: readonly PrincipleOutcomeRecord[],
  handoffId: string,
): readonly PrincipleOutcomeRecord[] {
  const latestByPrinciple = new Map<string, PrincipleOutcomeRecord>();
  for (const record of outcomes) {
    if (record.handoffId !== handoffId) continue;
    const existing = latestByPrinciple.get(record.principleId);
    if (!existing || existing.recordedAt <= record.recordedAt) {
      latestByPrinciple.set(record.principleId, record);
    }
  }
  return [...latestByPrinciple.values()].filter((record) => record.outcome === "pending");
}

export async function loadPrincipleEffectiveness(
  deps: PrincipleEffectivenessDeps,
  currentProjectRoot: string,
  cachedOutcomes?: readonly PrincipleOutcomeRecord[],
  cachedHandoffs?: readonly HandoffRecord[],
): Promise<readonly PrincipleEffectivenessRow[] | undefined> {
  const principleStore = deps.principleStore;
  const handoffStore = deps.handoffStore;
  if (!principleStore) return undefined;
  try {
    const [principles, outcomes, handoffs] = await Promise.all([
      principleStore.list(),
      cachedOutcomes !== undefined
        ? Promise.resolve(cachedOutcomes)
        : principleStore.listOutcomes(),
      cachedHandoffs !== undefined
        ? Promise.resolve(cachedHandoffs)
        : (handoffStore ? handoffStore.list() : Promise.resolve<readonly HandoffRecord[]>([])),
    ]);
    const scopedHandoffs = cachedHandoffs !== undefined
      ? handoffs
      : filterHandoffsForProject(handoffs, currentProjectRoot);
    const hasHandoffScopeSource = handoffStore !== undefined || cachedHandoffs !== undefined;
    const scopedOutcomes = hasHandoffScopeSource
      ? filterOutcomesForHandoffs(outcomes, scopedHandoffs)
      : outcomes;
    return buildPrincipleEffectivenessRows(principles, scopedOutcomes, scopedHandoffs);
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

function filterHandoffsForProject(
  handoffs: readonly HandoffRecord[],
  currentProjectRoot: string,
): readonly HandoffRecord[] {
  return handoffs.filter((handoff) => isHandoffInProject(handoff, currentProjectRoot));
}

function filterOutcomesForHandoffs(
  outcomes: readonly PrincipleOutcomeRecord[],
  handoffs: readonly HandoffRecord[],
): readonly PrincipleOutcomeRecord[] {
  if (handoffs.length === 0) return [];
  const handoffIds = new Set(handoffs.map((handoff) => handoff.id));
  return outcomes.filter((record) => handoffIds.has(record.handoffId));
}

export function buildPrincipleEffectivenessRows(
  principles: readonly Principle[],
  outcomes: readonly PrincipleOutcomeRecord[],
  handoffs: readonly HandoffRecord[],
): readonly PrincipleEffectivenessRow[] {
  const rollup = buildPrincipleEffectiveness(principles, outcomes);
  const principleById = new Map(principles.map((principle) => [principle.id, principle]));
  const handoffById = new Map(handoffs.map((handoff) => [handoff.id, handoff]));

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
      .map((record) => {
        const handoff = handoffById.get(record.handoffId);
        const title = handoff?.name ?? handoff?.task ?? record.handoffId;
        return `${record.handoffId}: ${title}`;
      });

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
