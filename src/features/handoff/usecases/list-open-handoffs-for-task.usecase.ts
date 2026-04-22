import type { LaunchStorePort } from "../domain/launch-types.js";

// Returns plain ids (not records) so the task feature can depend on this
// without importing any handoff domain types.
export async function listOpenHandoffsForTask(
  store: LaunchStorePort,
  taskId: string,
): Promise<readonly string[]> {
  const all = await store.list();
  return all
    .filter((record) => !record.consumedAt && record.refs.taskId === taskId)
    .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1))
    .map((record) => record.id);
}
