import { CliError, requiredPosition, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { tableExists } from "../kernel/store.ts";
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

// d876: a ruling the slp plugin stored stays readable while the plugin is off.
function storedSlpRuling(context: PluginContext, id: string): DecisionSection | null {
  if (!tableExists(context.store, "slp_decisions")) return null;
  const row = context.store.database
    .query<{ actor: string; choice: string; scope: string; why: string; work_id: string | null }, [string]>(
      "SELECT actor, choice, scope, why, work_id FROM slp_decisions WHERE id = ?",
    )
    .get(id);
  if (!row) return null;
  return {
    data: { slpDecisions: [{ actor: row.actor, choice: row.choice, id, scope: row.scope, why: row.why, workId: row.work_id }] },
    label: "SLP rulings",
    text: [
      `${id} [${row.scope}] by ${row.actor}`,
      ...(row.work_id ? [`work: ${row.work_id}`] : []),
      `choice: ${row.choice}`,
      `why: ${row.why}`,
      "the slp plugin that recorded this is off; its full view: maestro plugin enable slp",
    ].join("\n"),
  };
}

export interface DecisionSection {
  data: Record<string, unknown>;
  label: string;
  text: string;
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

// A ruling can run to a thousand characters; in a pane that is a wall (d823).
// Wrapping is a terminal courtesy only: piped output stays one line per field
// so rg and head keep finding whole phrases.
function wrap(line: string, width: number | null): string {
  if (width === null || line.length <= width) return line;
  const lines: string[] = [];
  let current = "";
  for (const word of line.split(" ")) {
    const indent = lines.length === 0 ? "" : "  ";
    if (current && `${indent}${current} ${word}`.length > width) {
      lines.push(`${indent}${current}`);
      current = word;
    } else {
      current = current ? `${current} ${word}` : word;
    }
  }
  lines.push(`${lines.length === 0 ? "" : "  "}${current}`);
  return lines.join("\n");
}

function format(decision: DecisionRecord): string {
  return formatDecision(decision, null);
}

export function formatDecision(decision: DecisionRecord, width: number | null): string {
  return [
    wrap(`${decision.id} [${decision.state}] ${decision.text}`, width),
    decision.rationale ? wrap(`rationale: ${decision.rationale}`, width) : null,
    decision.dissent ? wrap(`dissent: ${decision.dissent}`, width) : null,
    decision.reviewAt ? `review at: ${decision.reviewAt}` : null,
    decision.needsOwner ? "needs owner: yes" : null,
    decision.withdrawReason ? wrap(`withdraw reason: ${decision.withdrawReason}`, width) : null,
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

    const insertDraft = (invocation: CliInvocation, needsOwner: boolean): DecisionRecord => {
      const text = requiredPosition(invocation, 0, "decision text");
      // A one-positional invocation naming an existing decision reads as an
      // edit that forgot its replacement text, and the create path cannot
      // tell it from a decision whose text is literally that id. It minted
      // one titled d727 beside the decision it meant to edit (w562).
      const named = getDecision(context, text);
      if (named) {
        const retire = named.state === "draft"
          ? `maestro decision withdraw ${named.id} --reason "<why>" to drop it`
          : `maestro decision draft "<text>" --supersedes ${named.id} to retire it`;
        throw new CliError(
          "MISSING_ARGUMENT",
          `${named.id} already exists, so this reads as an edit with no replacement text; run: maestro decision draft ${named.id} "<replacement text>" to edit its text, maestro decision draft ${named.id} --rationale "<why>" to edit a field alone, or ${retire}`,
          { field: "replacement text", id: named.id, state: named.state },
        );
      }
      const rationale = option(invocation, "rationale");
      const dissent = option(invocation, "dissent");
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
      const id = context.store.nextCountedId("d", ["decisions", "slp_decisions"]);
      context.store.database
        .query(
          `INSERT INTO decisions
            (id, text, rationale, dissent, needs_owner, state, parent_id, work_id, supersedes_id, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?)`,
        )
        .run(
          id,
          text,
          rationale,
          dissent,
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
        },
      });
      return service.get(id) as DecisionRecord;
    };

    const lockDraft = (id: string, suppliedDissent: string | null): DecisionRecord => {
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
          "UPDATE decisions SET state = 'locked', dissent = ?, updated_at = ? WHERE id = ? AND state = 'draft'",
        )
        .run(
          suppliedDissent ?? decision.dissent,
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
    };

    context.effect(() =>
      registerSessionCommand(context, "decision add", (invocation): CliResult => {
        if (invocation.options["needs-owner"] === true) {
          const command = 'maestro decision draft "<text>" --needs-owner';
          throw new CliError(
            "INVALID_ARGUMENT",
            `decision add locks at once, and a decision that waits on the owner stays a draft; run: ${command}`,
            { command },
          );
        }
        const added = context.store.database
          .transaction(() => lockDraft(insertDraft(invocation, false).id, null))
          .immediate();
        return { data: { decision: added }, text: format(added) };
      }, {
        description: "Record a decision and lock it in one step.",
        flags: {
          "--rationale": { description: "Record why this decision was made.", value: true },
          "--dissent": { description: "Record the dissenting view.", value: true },
          "--parent": { description: "Attach this decision beneath a parent decision.", value: true },
          "--work": { description: "Link this decision to a work item.", value: true },
          "--supersedes": { description: "Supersede a locked decision with this one.", value: true },
          "--needs-owner": { hidden: true },
        },
        positionals: [{ name: "text", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "decision draft",
        (invocation): CliResult => {
          const first = requiredPosition(invocation, 0, "decision text");
          const second = invocation.positionals[1];
          const needsOwner = invocation.options["needs-owner"] === true;
          const suppliedDissent = option(invocation, "dissent");
          const suppliedRationale = option(invocation, "rationale");
          // One positional naming an existing decision plus an edit field is an
          // edit that keeps the text; the same shape with no field is the
          // forgotten-replacement case below (w562, w587).
          const fieldEdit = second === undefined &&
            (suppliedRationale !== null || suppliedDissent !== null || needsOwner) &&
            getDecision(context, first) !== null;
          if (second !== undefined || fieldEdit) {
            // The edit UPDATE sets text, rationale, dissent and needs_owner;
            // these three were parsed and dropped in silence, so a supersession
            // could be typed, accepted and lost (w562).
            const links = ["supersedes", "parent", "work"].filter((name) =>
              option(invocation, name) !== null
            );
            if (links.length > 0) {
              const named = links.map((name) => `--${name}`).join(", ");
              throw new CliError(
                "INVALID_ARGUMENT",
                `${named} ${links.length === 1 ? "belongs" : "belong"} to a new draft, not to an edit of ${first}; an edit changes text, rationale, dissent and owner requirement only. Withdraw ${first} and draft the replacement with its links, or edit the text alone`,
                { id: first, options: links },
              );
            }
            const edit = context.store.database.transaction(() => {
              const existing = getDecision(context, first);
              if (!existing) {
                if (second === undefined) {
                  throw new CliError("NOT_FOUND", `decision not found: ${first}`, { id: first });
                }
                throw new CliError("UNKNOWN_ARGUMENT", `unknown argument: ${second}`, {
                  argument: second,
                });
              }
              const text = second === undefined
                ? existing.text
                : requiredPosition(invocation, 1, "replacement text");
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
              const rationale = suppliedRationale ?? existing.rationale;
              const dissent = suppliedDissent ?? existing.dissent;
              const updatedAt = new Date().toISOString();
              const updated = context.store.database
                .query(
                  `UPDATE decisions
                   SET text = ?, rationale = ?, dissent = ?, needs_owner = ?, updated_at = ?
                   WHERE id = ? AND state = 'draft' AND withdrawn_at IS NULL
                   RETURNING id`,
                )
                .get(
                  text,
                  rationale,
                  dissent,
                  needsOwner || existing.needsOwner ? 1 : 0,
                  updatedAt,
                  existing.id,
                ) as { id: string } | null;
              if (updated?.id !== existing.id) {
                throw new CliError(
                  "INVALID_STATE",
                  `${existing.id} changed while the edit was pending`,
                  { id: existing.id },
                );
              }
              context.log.append({
                type: "decision.draft",
                entityType: "decision",
                entityId: existing.id,
                sessionId: context.sessions.current().id,
                payload: {
                  text,
                  edit: true,
                },
              });
              return {
                decision: service.get(existing.id) as DecisionRecord,
                previous: existing.text,
              };
            });
            const updated = edit.immediate();
            return {
              data: { decision: updated.decision, previous: updated.previous },
              text: `${format(updated.decision)}\nprevious: ${updated.previous}`,
            };
          }

          const created = context.store.database
            .transaction(() => insertDraft(invocation, needsOwner))
            .immediate();
          return { data: { decision: created }, text: format(created) };
        },
        {
          description: "Create or edit a draft that waits on the owner; the usual decision is recorded and locked in one step by decision add.",
          flags: {
            "--rationale": { description: "Record why this decision was made.", value: true },
            "--dissent": { description: "Record the dissenting view.", value: true },
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
        const id = requiredPosition(invocation, 0, "decision id");
        const reason = option(invocation, "reason");
        if (!reason?.trim()) {
          throw new CliError(
            "MISSING_ARGUMENT",
            `decision withdraw requires --reason <text>; run: maestro decision withdraw ${id} --reason "<why>"`,
            { command: `maestro decision withdraw ${id} --reason "<why>"`, id },
          );
        }
        const withdraw = context.store.database.transaction(() => {
          const decision = requireDecision(context, id);
          if (decision.state !== "draft") {
            if (decision.state === "locked") {
              const command = `maestro decision add "<replacement>" --supersedes ${id}`;
              throw new CliError(
                "INVALID_STATE",
                `${id} is locked; locked decisions are retired with: ${command}`,
                { command, id, state: decision.state },
              );
            }
            if (decision.state === "superseded") {
              const successor = decision.supersededById;
              const command = `maestro decision show ${successor ?? id}`;
              throw new CliError(
                "INVALID_STATE",
                successor
                  ? `${id} is superseded by ${successor}; inspect the successor with: ${command}`
                  : `${id} is superseded; inspect it with: ${command}`,
                {
                  command,
                  id,
                  state: decision.state,
                  ...(successor ? { supersededById: successor } : {}),
                },
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
        const id = requiredPosition(invocation, 0, "decision id");
        const suppliedDissent = option(invocation, "dissent");
        const locked = context.store.database
          .transaction(() => lockDraft(id, suppliedDissent))
          .immediate();
        return { data: { decision: locked }, text: format(locked) };
      }, {
        description: "Lock a draft decision against further edits.",
        flags: {
          "--dissent": { description: "Record the dissenting view.", value: true },
        },
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      context.cli.register("decision show", async (invocation): Promise<CliResult> => {
        const id = requiredPosition(invocation, 0, "decision id");
        const decision = getDecision(context, id);
        // w736 F4, l4: other decision spaces answer to the same dNNN ids; each is printed, none hides another.
        const sections = (await context.events.serial<{ id: string }>("decision.show", { id }))
          .filter((section): section is DecisionSection => typeof section === "object" && section !== null);
        if (sections.length === 0) {
          const stored = storedSlpRuling(context, id);
          if (stored) sections.push(stored);
        }
        if (!decision && sections.length === 0) {
          throw new CliError("NOT_FOUND", `decision not found: ${id}`, { id });
        }
        const width = invocation.options.wide === true || !process.stdout.isTTY
          ? null
          : process.stdout.columns || 80;
        // w743 F5: the store label alone cannot separate two spaces that live in
        // the same store, and the ledger's own render carries no space at all. So
        // when this one id answers in both, each block is headed with the space
        // that answered. One space answering prints exactly what it printed
        // before: a heading on a single answer would be noise about a collision
        // that is not happening.
        const collided = (decision ? 1 : 0) + sections.length > 1;
        return {
          data: Object.assign({ decision }, ...sections.map((section) => section.data)),
          text: [
            ...(decision
              ? [`${collided ? `${id} in the decision ledger:\n` : ""}${formatDecision(decision, width)}`]
              : []),
            ...sections.map((section) => `${collided ? `${id} in the ${section.label}:\n` : ""}${section.text}`),
          ].join("\n\n"),
        };
      }, {
        description: "Show one decision and its links; long fields wrap for a terminal.",
        flags: { "--wide": { description: "Print every field on one raw line even in a terminal." } },
        mutates: false,
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      context.cli.register(
        "decision list",
        (invocation): CliResult => {
          const workId = option(invocation, "work");
          const database = context.store.database;
          if (workId && !database.query("SELECT 1 FROM work WHERE id = ?").get(workId)) {
            throw new CliError("NOT_FOUND", `work not found: ${workId}; run: maestro work list`, {
              command: "maestro work list",
              id: workId,
            });
          }
          const total = database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM decisions")
            .get()?.count ?? 0;
          const order = "ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)";
          const rows = workId
            ? database.query<DecisionRow, [string]>(`SELECT * FROM decisions WHERE work_id = ? ${order}`).all(workId)
            : invocation.options.all === true
            ? database.query<DecisionRow, []>(`SELECT * FROM decisions ${order}`).all()
            : database
                .query<DecisionRow, []>(
                  "SELECT * FROM decisions ORDER BY updated_at DESC, CAST(SUBSTR(id, 2) AS INTEGER) DESC LIMIT 20",
                )
                .all();
          const decisions = rows.map(fromRow);
          const lines = decisions.map((decision) =>
            `${decision.id} [${decision.state}] ${decision.text}` +
            (decision.withdrawReason ? ` | withdraw reason: ${decision.withdrawReason}` : "")
          );
          if (!workId && invocation.options.all !== true && total > decisions.length) {
            lines.push(`showing ${decisions.length} of ${total}; maestro decision list --all`);
          }
          return { data: { decisions, total }, text: lines.join("\n") };
        },
        {
          description: "List the 20 most recently updated decisions, newest first, with the total.",
          flags: {
            "--all": { description: "List every decision in id order." },
            "--work": { description: "List the decisions linked to one work ID.", value: true },
          },
          mutates: false,
        },
      ),
    );
  },
};
