import type { Contract } from "../domain/contract/contract-types.js";
import type { ContractStoreQueryPort } from "../ports/contract-store.port.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";
import { readCurrentContractWithBackfill } from "./read-current-contract-with-backfill.js";

export async function getCurrentContract(
  store: ContractVersionStorePort,
  legacyStoreOrTaskId: ContractStoreQueryPort | string,
  maybeTaskId?: string,
): Promise<Contract | undefined> {
  // Two-arg form: getCurrentContract(store, taskId) — preserves the
  // pre-bridge call signature for tests that use the v2 store directly.
  if (typeof legacyStoreOrTaskId === "string") {
    return readCurrentContractWithBackfill(store, undefined, legacyStoreOrTaskId);
  }
  return readCurrentContractWithBackfill(store, legacyStoreOrTaskId, maybeTaskId as string);
}
