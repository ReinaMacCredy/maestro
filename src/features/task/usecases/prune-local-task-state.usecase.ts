import type { CandidateStorePort } from "../ports/candidate-store.port.js";
import type { TaskContinuationStorePort } from "../ports/task-continuation-store.port.js";

export type PruneKinds = "both" | "candidates" | "continuations";

export interface PruneKindReport {
  readonly purged: number;
  readonly kept: number;
  readonly oldestKeptAt?: string;
  readonly newestPurgedAt?: string;
}

export interface PruneReport {
  readonly dryRun: boolean;
  readonly keep: number;
  readonly all: boolean;
  readonly kinds: PruneKinds;
  readonly candidates: PruneKindReport;
  readonly continuations: PruneKindReport;
}

export interface PruneDeps {
  readonly candidateStore: CandidateStorePort;
  readonly continuationStore: TaskContinuationStorePort;
}

export interface PruneOptions {
  readonly keep: number;
  readonly kinds: PruneKinds;
  readonly all: boolean;
  readonly dryRun: boolean;
}

export async function pruneLocalTaskState(
  deps: PruneDeps,
  opts: PruneOptions,
): Promise<PruneReport> {
  const runCandidates = opts.kinds === "both" || opts.kinds === "candidates";
  const runContinuations = opts.kinds === "both" || opts.kinds === "continuations";

  const [candidates, continuations] = await Promise.all([
    runCandidates ? pruneCandidates(deps.candidateStore, opts) : { purged: 0, kept: 0 },
    runContinuations ? pruneContinuations(deps.continuationStore, opts) : { purged: 0, kept: 0 },
  ]);

  return {
    dryRun: opts.dryRun,
    keep: opts.keep,
    all: opts.all,
    kinds: opts.kinds,
    candidates,
    continuations,
  };
}

async function pruneCandidates(
  store: CandidateStorePort,
  opts: PruneOptions,
): Promise<PruneKindReport> {
  const all = await store.all();
  const sorted = [...all].sort((left, right) => right.capturedAt.localeCompare(left.capturedAt));
  const { keepers, purgeables } = partition(sorted, opts);

  if (!opts.dryRun) {
    await Promise.all(purgeables.map((candidate) => store.delete(candidate.id)));
  }

  return buildKindReport(keepers, purgeables, (entry) => entry.capturedAt);
}

async function pruneContinuations(
  store: TaskContinuationStorePort,
  opts: PruneOptions,
): Promise<PruneKindReport> {
  const all = await store.listCompleted();
  const sorted = [...all].sort((left, right) => right.lastActiveAt.localeCompare(left.lastActiveAt));
  const { keepers, purgeables } = partition(sorted, opts);

  if (!opts.dryRun) {
    await Promise.all(purgeables.map((summary) => store.deleteCompleted(summary.taskId)));
  }

  return buildKindReport(keepers, purgeables, (entry) => entry.lastActiveAt);
}

function partition<T>(
  sortedDesc: readonly T[],
  opts: PruneOptions,
): { keepers: readonly T[]; purgeables: readonly T[] } {
  if (opts.all) {
    return { keepers: [], purgeables: sortedDesc };
  }
  return {
    keepers: sortedDesc.slice(0, opts.keep),
    purgeables: sortedDesc.slice(opts.keep),
  };
}

function buildKindReport<T>(
  keepers: readonly T[],
  purgeables: readonly T[],
  timestampOf: (entry: T) => string,
): PruneKindReport {
  const oldestKept = keepers.length > 0 ? timestampOf(keepers[keepers.length - 1]!) : undefined;
  const newestPurged = purgeables.length > 0 ? timestampOf(purgeables[0]!) : undefined;
  return {
    purged: purgeables.length,
    kept: keepers.length,
    ...(oldestKept ? { oldestKeptAt: oldestKept } : {}),
    ...(newestPurged ? { newestPurgedAt: newestPurged } : {}),
  };
}
