import type { Contract } from "../types/contract.js";
import type { ContractStoreQueryPort, ContractVersionStorePort } from "../repo/contract-store.port.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";

// Active L1 statuses that should be visible to the L2 readers. Drafts and
// discarded contracts are deliberately invisible to the trust substrate.
function isMirrorableStatus(contract: Contract): boolean {
  return (
    contract.status === "locked"
    || contract.status === "amended"
    || contract.status === "fulfilled"
    || contract.status === "broken"
  );
}

// Read the current versioned contract; if missing but an active L1 record
// exists for the task, backfill the L2 store with v1 first.
export async function readCurrentContractWithBackfill(
  versionStore: ContractVersionStorePort,
  legacyStore: ContractStoreQueryPort | undefined,
  taskId: string,
): Promise<Contract | undefined> {
  const current = await versionStore.readCurrent(taskId);
  if (current !== undefined) return current;
  if (legacyStore === undefined) return undefined;

  const legacy = await legacyStore.getByTaskId(taskId);
  if (!legacy || !isMirrorableStatus(legacy)) return undefined;

  await versionStore.write(taskId, 1, legacy);
  return legacy;
}

// Returns a draft contract for the task if one exists, regardless of L2
// version-store state.
export async function readDraftContract(
  legacyStore: ContractStoreQueryPort | undefined,
  taskId: string,
): Promise<Contract | undefined> {
  if (legacyStore === undefined) return undefined;
  const legacy = await legacyStore.getByTaskId(taskId);
  if (legacy && legacy.status === "draft") return legacy;
  return undefined;
}

// Same shape as readCurrentContractWithBackfill but for the full version
// history.
export async function readContractHistoryWithBackfill(
  versionStore: ContractVersionStorePort,
  legacyStore: ContractStoreQueryPort | undefined,
  taskId: string,
): Promise<readonly Contract[]> {
  const versions = await versionStore.history(taskId);
  if (versions.length > 0) return versions;
  if (legacyStore === undefined) return [];

  const legacy = await legacyStore.getByTaskId(taskId);
  if (!legacy || !isMirrorableStatus(legacy)) return [];

  await versionStore.write(taskId, 1, legacy);
  return [legacy];
}

// Apply an amend batch's `add` / `remove` to a contract's existing path set.
// Returns the new path set plus paths from `add` that conflicted with `base`.
// In-batch duplicates within `add` are deduplicated silently (idempotent add).
export function applyPathChanges(
  existing: readonly string[],
  add: readonly string[],
  remove: readonly string[],
): { result: string[]; skipped: string[] } {
  const removeSet = new Set(remove);
  const base = existing.filter((p) => !removeSet.has(p));
  // Decide skip vs. accept against `base` only — never against the
  // partially-built result. This keeps the outcome independent of the
  // order paths appear in `add` (e.g., ["*.ts", "foo.ts"] and
  // ["foo.ts", "*.ts"] now produce the same result).
  const baseSet = new Set(base);
  const accepted: string[] = [];
  const acceptedSet = new Set<string>();
  const skipped: string[] = [];
  for (const p of add) {
    if (baseSet.has(p) || matchesAnyGlob(base, p)) {
      skipped.push(p);
      continue;
    }
    if (acceptedSet.has(p)) {
      // Duplicate within the same batch — skip silently (idempotent add).
      continue;
    }
    accepted.push(p);
    acceptedSet.add(p);
  }
  return { result: [...base, ...accepted], skipped };
}
