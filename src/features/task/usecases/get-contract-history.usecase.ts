import type { Contract } from "../domain/contract/contract-types.js";
import type { ContractStoreQueryPort } from "../ports/contract-store.port.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";
import { readContractHistoryWithBackfill } from "./read-current-contract-with-backfill.js";

export async function getContractHistory(
  store: ContractVersionStorePort,
  legacyStoreOrTaskId: ContractStoreQueryPort | string,
  maybeTaskId?: string,
): Promise<readonly Contract[]> {
  if (typeof legacyStoreOrTaskId === "string") {
    return readContractHistoryWithBackfill(store, undefined, legacyStoreOrTaskId);
  }
  return readContractHistoryWithBackfill(store, legacyStoreOrTaskId, maybeTaskId as string);
}
