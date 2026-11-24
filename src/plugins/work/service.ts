import type { PluginContext } from "../../kernel/loader.ts";
import { expireDeadLease, getWork, listWork, toWork, type WorkRow, type WorkService } from "./records.ts";

export function createWorkService(context: PluginContext): WorkService {
  return {
    get: (id) => getWork(context, id),
    list: () => listWork(context),
    children: (id) =>
      context.store.database
        .query<WorkRow, [string]>(
          "SELECT * FROM work WHERE parent_id = ? ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)",
        )
        .all(id)
        .map(toWork)
        .map((work) => expireDeadLease(context, work)),
    release: (id, holder, updatedAt) =>
      context.store.database
        .query(
          "UPDATE work SET state = 'open', held_by = NULL, updated_at = ? WHERE id = ? AND held_by = ?",
        )
        .run(updatedAt, id, holder).changes > 0,
    snapshot: () =>
      context.store.database
        .query<WorkRow, []>("SELECT * FROM work ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)")
        .all()
        .map(toWork),
  };
}
