import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { WorkService } from "./work.ts";

export interface DecisionRecord {
  id: string;
  text: string;
  rationale: string | null;
  state: "draft" | "locked" | "superseded";
  parentId: string | null;
  workId: string | null;
  supersedesId: string | null;
  supersededById: string | null;
  createdAt: string;
  updatedAt: string;
}

interface DecisionRow {
  id: string;
  text: string;
  rationale: string | null;
  state: "draft" | "locked" | "superseded";
  parent_id: string | null;
  work_id: string | null;
  supersedes_id: string | null;
  superseded_by_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface DecisionService {
  get(id: string): DecisionRecord | null;
  list(): DecisionRecord[];
}

function fromRow(row: DecisionRow): DecisionRecord {
  return {
    id: row.id,
    text: row.text,
    rationale: row.rationale,
    state: row.state,
    parentId: row.parent_id,
    workId: row.work_id,
    supersedesId: row.supersedes_id,
    supersededById: row.superseded_by_id,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

function getDecision(context: PluginContext, id: string): DecisionRecord | null {
  const row = context.store.database
    .query<DecisionRow, [string]>("SELECT * FROM decisions WHERE id = ?")
    .get(id);
  return row ? fromRow(row) : null;
}

function requireDecision(context: PluginContext, id: string): DecisionRecord {
  const decision = getDecision(context, id);
  if (!decision) throw new CliError("NOT_FOUND", `decision not found: ${id}`, { id });
  return decision;
}

function nextId(context: PluginContext): string {
  const next =
    context.store.database
      .query<{ next: number }, []>(
        "SELECT COALESCE(MAX(CAST(SUBSTR(id, 2) AS INTEGER)), 0) + 1 AS next FROM decisions",
      )
      .get()?.next ?? 1;
  return `d${next}`;
}

function option(invocation: CliInvocation, name: string): string | null {
  const value = invocation.options[name];
  return typeof value === "string" ? value : null;
}

function required(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function format(decision: DecisionRecord): string {
  return [
    `${decision.id} [${decision.state}] ${decision.text}`,
    decision.rationale ? `rationale: ${decision.rationale}` : null,
    decision.parentId ? `parent: ${decision.parentId}` : null,
    decision.workId ? `work: ${decision.workId}` : null,
    decision.supersedesId ? `supersedes: ${decision.supersedesId}` : null,
    decision.supersededById ? `superseded by: ${decision.supersededById}` : null,
  ]
    .filter((value): value is string => value !== null)
    .join("\n");
}

export const decisionPlugin: BuiltInPlugin = {
  name: "decision",
  inject: ["work"],
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS decisions (
        id TEXT PRIMARY KEY,
        text TEXT NOT NULL,
        state TEXT NOT NULL CHECK(state IN ('draft', 'locked', 'superseded')),
        parent_id TEXT REFERENCES decisions(id),
        work_id TEXT REFERENCES work(id),
        supersedes_id TEXT REFERENCES decisions(id),
        superseded_by_id TEXT REFERENCES decisions(id),
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
    `);
    const hasDecisionColumn = (name: string) =>
      context.store.database
        .query<{ name: string }, []>("PRAGMA table_info(decisions)")
        .all()
        .some((column) => column.name === name);
    if (!hasDecisionColumn("rationale")) {
      try {
        context.store.migrate("ALTER TABLE decisions ADD COLUMN rationale TEXT");
      } catch (error) {
        // Concurrent startup can race the same ALTER; losing is fine.
        if (!hasDecisionColumn("rationale")) throw error;
      }
    }
    const service: DecisionService = {
      get: (id) => getDecision(context, id),
      list: () =>
        context.store.database
          .query<DecisionRow, []>(
            "SELECT * FROM decisions ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)",
          )
          .all()
          .map(fromRow),
    };
    context.effect(() => context.provide("decision", service));

    context.effect(() =>
      context.cli.register(
        "decision draft",
        (invocation): CliResult => {
          const first = required(invocation, 0, "decision text");
          const second = invocation.positionals[1];
          const existing = getDecision(context, first);
          if (existing && second !== undefined) {
            if (existing.state !== "draft") {
              throw new CliError(
                "LOCKED_DECISION",
                `${existing.id} is ${existing.state}; create a draft with --supersedes instead`,
                { id: existing.id, state: existing.state },
              );
            }
            const text = required(invocation, 1, "replacement text");
            const rationale = option(invocation, "rationale") ?? existing.rationale;
            const updatedAt = new Date().toISOString();
            context.store.database
              .query(
                "UPDATE decisions SET text = ?, rationale = ?, updated_at = ? WHERE id = ? AND state = 'draft'",
              )
              .run(text, rationale, updatedAt, existing.id);
            context.log.append({
              type: "decision.draft",
              entityType: "decision",
              entityId: existing.id,
              sessionId: context.sessions.current().id,
              payload: { text, edit: true },
            });
            const updated = service.get(existing.id);
            return {
              data: { decision: updated, previous: existing.text },
              text: `${format(updated as DecisionRecord)}\nprevious: ${existing.text}`,
            };
          }
          if (second !== undefined) {
            throw new CliError("UNKNOWN_ARGUMENT", `unknown argument: ${second}`, {
              argument: second,
            });
          }

          const text = first;
          const rationale = option(invocation, "rationale");
          const parentId = option(invocation, "parent");
          const workId = option(invocation, "work");
          const supersedesId = option(invocation, "supersedes");
          if (parentId) requireDecision(context, parentId);
          if (workId) {
            const work = context.work as WorkService;
            if (!work.get(workId)) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
          }
          if (supersedesId) {
            const superseded = requireDecision(context, supersedesId);
            if (superseded.state !== "locked") {
              throw new CliError("INVALID_STATE", `${supersedesId} must be locked before superseding`);
            }
          }
          const id = nextId(context);
          const now = new Date().toISOString();
          const transaction = context.store.database.transaction(() => {
            context.store.database
              .query(
                `INSERT INTO decisions
                  (id, text, rationale, state, parent_id, work_id, supersedes_id, created_at, updated_at)
                 VALUES (?, ?, ?, 'draft', ?, ?, ?, ?, ?)`,
              )
              .run(id, text, rationale, parentId, workId, supersedesId, now, now);
            if (supersedesId) {
              context.store.database
                .query(
                  "UPDATE decisions SET state = 'superseded', superseded_by_id = ?, updated_at = ? WHERE id = ?",
                )
                .run(id, now, supersedesId);
            }
          });
          transaction();
          context.log.append({
            type: supersedesId ? "decision.supersede" : "decision.draft",
            entityType: "decision",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { text, parentId, workId, supersedesId },
          });
          const created = service.get(id) as DecisionRecord;
          return { data: { decision: created }, text: format(created) };
        },
        {
          description: "Create or edit a draft decision.",
          flags: {
            "--rationale": { description: "Record why this decision was made.", value: true },
            "--parent": { description: "Attach this draft beneath a parent decision.", value: true },
            "--work": { description: "Link this draft to a work item.", value: true },
            "--supersedes": { description: "Supersede a locked decision with this draft.", value: true },
          },
          positionals: [
            { name: "text-or-id", required: true },
            { name: "replacement", required: false },
          ],
          rootDescription: "Record durable decisions and their lifecycle.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register("decision lock", (invocation): CliResult => {
        const id = required(invocation, 0, "decision id");
        const decision = requireDecision(context, id);
        if (decision.state !== "draft") {
          throw new CliError("INVALID_STATE", `${id} is ${decision.state}`);
        }
        const updatedAt = new Date().toISOString();
        context.store.database
          .query("UPDATE decisions SET state = 'locked', updated_at = ? WHERE id = ?")
          .run(updatedAt, id);
        context.log.append({
          type: "decision.lock",
          entityType: "decision",
          entityId: id,
          sessionId: context.sessions.current().id,
        });
        const locked = service.get(id) as DecisionRecord;
        return { data: { decision: locked }, text: format(locked) };
      }, {
        description: "Lock a draft decision against further edits.",
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      context.cli.register("decision show", (invocation): CliResult => {
        const decision = requireDecision(context, required(invocation, 0, "decision id"));
        return { data: { decision }, text: format(decision) };
      }, {
        description: "Show one decision and its links.",
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      context.cli.register(
        "decision list",
        (): CliResult => {
          const decisions = service.list();
          return {
            data: { decisions },
            text: decisions.map((decision) => `${decision.id} [${decision.state}] ${decision.text}`).join("\n"),
          };
        },
        { description: "List decisions and their current states." },
      ),
    );
  },
};
