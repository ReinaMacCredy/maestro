import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { DispatchService } from "./dispatch.ts";
import { registerSessionCommand } from "./session-required.ts";
import type { WorkService } from "./work.ts";

export interface DecisionRecord {
  id: string;
  text: string;
  rationale: string | null;
  dissent: string | null;
  reviewAt: string | null;
  needsOwner: boolean;
  state: "draft" | "locked" | "superseded" | "withdrawn";
  withdrawnAt: string | null;
  withdrawReason: string | null;
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
  dissent: string | null;
  review_at: string | null;
  needs_owner: number;
  state: "draft" | "locked" | "superseded";
  parent_id: string | null;
  work_id: string | null;
  supersedes_id: string | null;
  superseded_by_id: string | null;
  withdrawn_at: string | null;
  withdraw_reason: string | null;
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
    dissent: row.dissent,
    reviewAt: row.review_at,
    needsOwner: row.needs_owner === 1,
    state: row.withdrawn_at ? "withdrawn" : row.state,
    withdrawnAt: row.withdrawn_at,
    withdrawReason: row.withdraw_reason,
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

function requireSupersessionTarget(context: PluginContext, id: string): DecisionRecord {
  const decision = requireDecision(context, id);
  if (decision.state === "withdrawn") {
    throw new CliError("INVALID_STATE", `${id} is withdrawn`);
  }
  if (decision.state !== "locked") {
    throw new CliError("INVALID_STATE", `${id} must be locked before superseding`);
  }
  return decision;
}

function option(invocation: CliInvocation, name: string): string | null {
  const value = invocation.options[name];
  return typeof value === "string" ? value : null;
}

function reviewAtOption(invocation: CliInvocation): string | null {
  const value = option(invocation, "review-at");
  if (value === null) return null;
  const match = /^(\d{4})-(\d{2})-(\d{2})(?:T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:\d{2}))?$/.exec(value);
  const calendar = new Date(0);
  if (match) {
    calendar.setUTCFullYear(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
    calendar.setUTCHours(0, 0, 0, 0);
  }
  if (
    !match ||
    Number.isNaN(Date.parse(value)) ||
    calendar.getUTCFullYear() !== Number(match[1]) ||
    calendar.getUTCMonth() !== Number(match[2]) - 1 ||
    calendar.getUTCDate() !== Number(match[3])
  ) {
    throw new CliError("INVALID_VALUE", "--review-at must be an ISO date");
  }
  return value;
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
    decision.dissent ? `dissent: ${decision.dissent}` : null,
    decision.reviewAt ? `review at: ${decision.reviewAt}` : null,
    decision.needsOwner ? "needs owner: yes" : null,
    decision.withdrawReason ? `withdraw reason: ${decision.withdrawReason}` : null,
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
  inject: ["work", "dispatch"],
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
    context.store.ensureColumn(
      "decisions",
      "needs_owner",
      "ALTER TABLE decisions ADD COLUMN needs_owner INTEGER NOT NULL DEFAULT 0",
    );
    context.store.ensureColumn(
      "decisions",
      "dissent",
      "ALTER TABLE decisions ADD COLUMN dissent TEXT",
    );
    context.store.ensureColumn(
      "decisions",
      "review_at",
      "ALTER TABLE decisions ADD COLUMN review_at TEXT",
    );
    context.store.ensureColumn(
      "decisions",
      "withdrawn_at",
      "ALTER TABLE decisions ADD COLUMN withdrawn_at TEXT",
    );
    context.store.ensureColumn(
      "decisions",
      "withdraw_reason",
      "ALTER TABLE decisions ADD COLUMN withdraw_reason TEXT",
    );
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
      registerSessionCommand(
        context,
        "decision draft",
        (invocation): CliResult => {
          const first = required(invocation, 0, "decision text");
          const second = invocation.positionals[1];
          const existing = getDecision(context, first);
          const needsOwner = invocation.options["needs-owner"] === true;
          const suppliedDissent = option(invocation, "dissent");
          const suppliedReviewAt = reviewAtOption(invocation);
          if (existing && second !== undefined) {
            if (existing.state === "withdrawn") {
              throw new CliError("INVALID_STATE", `${existing.id} is withdrawn`, {
                id: existing.id,
                state: existing.state,
              });
            }
            if (existing.state !== "draft") {
              throw new CliError(
                "LOCKED_DECISION",
                `${existing.id} is ${existing.state}; create a draft with --supersedes instead`,
                { id: existing.id, state: existing.state },
              );
            }
            const text = required(invocation, 1, "replacement text");
            const rationale = option(invocation, "rationale") ?? existing.rationale;
            const dissent = suppliedDissent ?? existing.dissent;
            const reviewAt = suppliedReviewAt ?? existing.reviewAt;
            const updatedAt = new Date().toISOString();
            context.store.database
              .query(
                "UPDATE decisions SET text = ?, rationale = ?, dissent = ?, review_at = ?, needs_owner = ?, updated_at = ? WHERE id = ? AND state = 'draft'",
              )
              .run(
                text,
                rationale,
                dissent,
                reviewAt,
                needsOwner || existing.needsOwner ? 1 : 0,
                updatedAt,
                existing.id,
              );
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
            requireSupersessionTarget(context, supersedesId);
          }
          const now = new Date().toISOString();
          const transaction = context.store.database.transaction(() => {
            if (parentId) requireDecision(context, parentId);
            if (workId) {
              const work = context.work as WorkService;
              if (!work.get(workId)) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
            }
            if (supersedesId) {
              requireSupersessionTarget(context, supersedesId);
            }
            const council = workId
              ? (context.dispatch as DispatchService).council(workId)
              : null;
            const sealedCouncil = council?.sealed ? council.generationAnchor : null;
            const id = context.store.nextPrefixedId("decisions", "d");
            context.store.database
              .query(
                `INSERT INTO decisions
                  (id, text, rationale, dissent, review_at, needs_owner, state, parent_id, work_id, supersedes_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?)`,
              )
              .run(
                id,
                text,
                rationale,
                suppliedDissent,
                suppliedReviewAt,
                needsOwner ? 1 : 0,
                parentId,
                workId,
                supersedesId,
                now,
                now,
              );
            context.log.append({
              type: "decision.draft",
              entityType: "decision",
              entityId: id,
              sessionId: context.sessions.current().id,
              payload: {
                text,
                parentId,
                workId,
                supersedesId,
                ...(sealedCouncil ? { sealedCouncil } : {}),
              },
            });
            return { decision: service.get(id) as DecisionRecord, sealedCouncil };
          });
          const { decision: created, sealedCouncil } = transaction.immediate();
          if (sealedCouncil && workId) {
            process.stderr.write(
              `[sealed] ${workId} council is sealed; this draft is readable by its lanes\n`,
            );
          }
          return { data: { decision: created }, text: format(created) };
        },
        {
          description: "Create or edit a draft decision.",
          flags: {
            "--rationale": { description: "Record why this decision was made.", value: true },
            "--dissent": { description: "Record the dissenting view.", value: true },
            "--review-at": { description: "Schedule review at an ISO date.", value: true },
            "--needs-owner": { description: "Mark this draft as requiring an owner decision." },
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
      registerSessionCommand(context, "decision withdraw", (invocation): CliResult => {
        const id = required(invocation, 0, "decision id");
        const reason = option(invocation, "reason");
        if (!reason?.trim()) {
          throw new CliError(
            "MISSING_ARGUMENT",
            "decision withdraw requires --reason <text>",
          );
        }
        const withdraw = context.store.database.transaction(() => {
          const decision = requireDecision(context, id);
          if (decision.state !== "draft") {
            if (decision.state === "locked" || decision.state === "superseded") {
              const command = `maestro decision draft "<replacement>" --supersedes ${id}`;
              throw new CliError(
                "INVALID_STATE",
                `${id} is ${decision.state}; locked decisions are retired with: ${command}`,
                { command, id, state: decision.state },
              );
            }
            throw new CliError("INVALID_STATE", `${id} is ${decision.state}`, {
              id,
              state: decision.state,
            });
          }
          const withdrawnAt = new Date().toISOString();
          context.store.database
            .query(
              `UPDATE decisions
               SET withdrawn_at = ?, withdraw_reason = ?, updated_at = ?
               WHERE id = ? AND state = 'draft' AND withdrawn_at IS NULL`,
            )
            .run(withdrawnAt, reason, withdrawnAt, id);
          context.log.append({
            type: "decision.withdraw",
            entityType: "decision",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { reason },
          });
          return service.get(id) as DecisionRecord;
        });
        const withdrawn = withdraw.immediate();
        return { data: { decision: withdrawn }, text: format(withdrawn) };
      }, {
        description: "Withdraw a draft decision that will not be locked.",
        flags: {
          "--reason": { description: "Record why the draft was withdrawn.", value: true },
        },
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "decision lock", (invocation): CliResult => {
        const id = required(invocation, 0, "decision id");
        const suppliedDissent = option(invocation, "dissent");
        const suppliedReviewAt = reviewAtOption(invocation);
        const lock = context.store.database.transaction(() => {
          const decision = requireDecision(context, id);
          if (decision.state !== "draft") {
            throw new CliError("INVALID_STATE", `${id} is ${decision.state}`);
          }
          const updatedAt = new Date().toISOString();
          if (decision.supersedesId) {
            const predecessor = requireDecision(context, decision.supersedesId);
            if (predecessor.state !== "locked") {
              throw new CliError(
                "SUPERSESSION_CONFLICT",
                `${id} cannot supersede ${predecessor.id}; ${predecessor.id} is already superseded by ${predecessor.supersededById ?? "another decision"}`,
                {
                  id,
                  predecessorId: predecessor.id,
                  supersededById: predecessor.supersededById,
                },
              );
            }
            const superseded = context.store.database
              .query(
                `UPDATE decisions
                 SET state = 'superseded', superseded_by_id = ?, updated_at = ?
                 WHERE id = ? AND state = 'locked' AND superseded_by_id IS NULL`,
              )
              .run(id, updatedAt, predecessor.id);
            if (superseded.changes === 0) {
              const current = requireDecision(context, predecessor.id);
              throw new CliError(
                "SUPERSESSION_CONFLICT",
                `${id} cannot supersede ${predecessor.id}; ${predecessor.id} is already superseded by ${current.supersededById ?? "another decision"}`,
                {
                  id,
                  predecessorId: predecessor.id,
                  supersededById: current.supersededById,
                },
              );
            }
            context.log.append({
              type: "decision.supersede",
              entityType: "decision",
              entityId: id,
              sessionId: context.sessions.current().id,
              payload: { supersedesId: predecessor.id },
            });
          }
          const locked = context.store.database
            .query(
              "UPDATE decisions SET state = 'locked', dissent = ?, review_at = ?, updated_at = ? WHERE id = ? AND state = 'draft'",
            )
            .run(
              suppliedDissent ?? decision.dissent,
              suppliedReviewAt ?? decision.reviewAt,
              updatedAt,
              id,
            );
          if (locked.changes === 0) {
            const current = requireDecision(context, id);
            throw new CliError("INVALID_STATE", `${id} is ${current.state}`);
          }
          context.log.append({
            type: "decision.lock",
            entityType: "decision",
            entityId: id,
            sessionId: context.sessions.current().id,
          });
          return service.get(id) as DecisionRecord;
        });
        const locked = lock.immediate();
        return { data: { decision: locked }, text: format(locked) };
      }, {
        description: "Lock a draft decision against further edits.",
        flags: {
          "--dissent": { description: "Record the dissenting view.", value: true },
          "--review-at": { description: "Schedule review at an ISO date.", value: true },
        },
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      context.cli.register("decision show", (invocation): CliResult => {
        const decision = requireDecision(context, required(invocation, 0, "decision id"));
        return { data: { decision }, text: format(decision) };
      }, {
        description: "Show one decision and its links.",
        mutates: false,
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
            text: decisions.map((decision) =>
              `${decision.id} [${decision.state}] ${decision.text}` +
              (decision.withdrawReason ? ` | withdraw reason: ${decision.withdrawReason}` : "")
            ).join("\n"),
          };
        },
        { description: "List decisions and their current states.", mutates: false },
      ),
    );
  },
};
