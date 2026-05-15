import type { Contract } from "../domain/contract/contract-types.js";
import type { ContractStoreQueryPort } from "../ports/contract-store.port.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";

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
// exists for the task, backfill the L2 store with v1 first. This handles
// pre-fix repos where the L1 verbs wrote contracts but no L2 mirror existed.
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
// version-store state. Used to give a helpful "lock it first" hint when
// task verify / verdict request / plan check are run against a task that
// has a draft but no locked contract yet.
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
// history. If the L2 store has no versions but L1 has an active record,
// backfill v1 and return [contract].
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
