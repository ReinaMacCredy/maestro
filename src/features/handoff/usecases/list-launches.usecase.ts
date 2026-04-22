import type { HandoffLaunchRecord, LaunchStorePort } from "../domain/launch-types.js";

export interface ListLaunchesOptions {
  readonly openOnly?: boolean;
}

export async function listLaunches(
  store: LaunchStorePort,
  options: ListLaunchesOptions = {},
): Promise<readonly HandoffLaunchRecord[]> {
  const all = await store.list();
  const filtered = options.openOnly ? all.filter((record) => !record.consumedAt) : all;
  return [...filtered].sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
}
