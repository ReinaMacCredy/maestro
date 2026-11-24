import type { BuiltInPlugin } from "../kernel/loader.ts";
import { registerWorkAdd } from "./work/add.ts";
import { registerWorkBlockers } from "./work/blockers.ts";
import { registerWorkCancel } from "./work/cancel.ts";
import { registerWorkDone } from "./work/done.ts";
import { registerWorkReclaim, registerWorkRelease, registerWorkStart } from "./work/lease.ts";
import { registerWorkList } from "./work/list.ts";
import { registerWorkNote } from "./work/note.ts";
import { createWorkService } from "./work/service.ts";
import { registerWorkShow } from "./work/show.ts";

export { latestCheckpoints } from "./work/records.ts";
export type { Checkpoint, WorkAddGateInput, WorkGateInput, WorkRecord, WorkService } from "./work/records.ts";

export const workPlugin: BuiltInPlugin = {
  name: "work",
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS work (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        kind TEXT NOT NULL,
        state TEXT NOT NULL CHECK(state IN ('open', 'active', 'done')),
        parent_id TEXT REFERENCES work(id),
        acceptance TEXT,
        atomic_reason TEXT,
        evidence TEXT,
        held_by TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS work_blockers (
        work_id TEXT NOT NULL REFERENCES work(id),
        blocker_id TEXT NOT NULL REFERENCES work(id),
        PRIMARY KEY(work_id, blocker_id)
      );
      CREATE TABLE IF NOT EXISTS work_notes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        work_id TEXT NOT NULL REFERENCES work(id),
        text TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
    `);
    for (const [name, migration] of [
      ["cancelled_at", "ALTER TABLE work ADD COLUMN cancelled_at TEXT"],
      ["cancel_reason", "ALTER TABLE work ADD COLUMN cancel_reason TEXT"],
      ["reclaimed_from", "ALTER TABLE work ADD COLUMN reclaimed_from TEXT"],
      ["reclaimed_by", "ALTER TABLE work ADD COLUMN reclaimed_by TEXT"],
      ["reclaim_reason", "ALTER TABLE work ADD COLUMN reclaim_reason TEXT"],
      ["candidate", "ALTER TABLE work ADD COLUMN candidate TEXT"],
    ] as const) {
      context.store.ensureColumn("work", name, migration);
    }

    const service = createWorkService(context);
    context.effect(() => context.provide("work", service));

    registerWorkAdd(context, service);
    registerWorkStart(context, service);
    registerWorkRelease(context, service);
    registerWorkReclaim(context, service);
    registerWorkNote(context);
    registerWorkDone(context, service);
    registerWorkBlockers(context, service);
    registerWorkCancel(context, service);
    registerWorkShow(context, service);
    registerWorkList(context);
  },
};
