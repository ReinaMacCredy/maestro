import {
  CliError,
  requiredPosition,
  stringOption,
  type CliInvocation,
  type CliResult,
} from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { registerSessionCommand } from "./session-required.ts";
import type { WorkService } from "./work.ts";

export const dispatchLaneVocabulary = [
  { brief: "scout no-write", name: "scout" },
  { brief: "decision x2-3", name: "decision" },
  { brief: "delivery", name: "delivery" },
  { brief: "challenge", name: "challenge" },
  { brief: "shadow no-write", name: "shadow" },
] as const;

const dispatchLaneNames = dispatchLaneVocabulary.map(({ name }) => name);

export interface DispatchRecord {
  id: string;
  workId: string;
  objective: string;
  ownedScope: string;
  excludedScope: string;
  mutation: string;
  stopCondition: string;
  lane: string;
  evidenceRequired: string;
  pane: string | null;
  targetSession: string | null;
  openedBy: string | null;
  claimedBy: string | null;
  heldBy: string | null;
  state: "open" | "returned" | "cancelled";
  cancelReason: string | null;
  createdAt: string;
  updatedAt: string;
}

interface DispatchRow {
  id: string;
  work_id: string;
  objective: string;
  owned_scope: string;
  excluded_scope: string;
  mutation: string;
  stop_condition: string;
  lane: string;
  evidence_required: string;
  pane: string | null;
  target_session: string | null;
  opened_by: string | null;
  claimed_by: string | null;
  held_by: string | null;
  cancelled_at: string | null;
  cancel_reason: string | null;
  created_at: string;
  updated_at: string;
  returned: number;
}

export const handbackStatusVocabulary = [
  "DONE",
  "BLOCKED",
  "UNTESTABLE",
  "UNKNOWN",
  "FAILED",
  "CHALLENGE",
  "REOPEN_REQUEST",
  "DEPENDENCY_REQUEST",
  "COUNCIL_REQUEST",
] as const;

export type HandbackStatus = (typeof handbackStatusVocabulary)[number];

const handbackRequestStatuses = new Set<HandbackStatus>([
  "BLOCKED",
  "DEPENDENCY_REQUEST",
  "COUNCIL_REQUEST",
  "REOPEN_REQUEST",
]);

export interface HandbackRecord {
  id: string;
  dispatchId: string;
  status: HandbackStatus;
  request: string | null;
  candidate: string | null;
  claim: string;
  proof: string;
  assumptions: string;
  residualRisks: string;
  incidentalFindings: string;
  createdAt: string;
}

interface HandbackRow {
  id: string;
  dispatch_id: string;
  status: HandbackStatus;
  request: string | null;
  candidate: string | null;
  claim: string;
  proof: string;
  assumptions: string;
  residual_risks: string;
  incidental_findings: string;
  created_at: string;
}

interface CouncilDispatchRow {
  id: string;
  created_at: string;
  cancelled_at: string | null;
  returned_at: string | null;
}

export interface DispatchService {
  council(workId: string, dispatchId?: string): CouncilStatus;
  get(id: string): DispatchRecord | null;
  list(workId?: string): DispatchRecord[];
}

export interface HandbackService {
  get(id: string): HandbackRecord | null;
  list(dispatchId: string): HandbackRecord[];
}

export interface CouncilStatus {
  generationAnchor: string | null;
  workId: string;
  total: number;
  returned: number;
  resolved: number;
  sealed: boolean;
  unsealed: boolean;
  unsealReason: string | null;
}

const createHandbacksTableSql = `
  CREATE TABLE IF NOT EXISTS handbacks (
    id TEXT PRIMARY KEY,
    dispatch_id TEXT NOT NULL REFERENCES dispatches(id),
    status TEXT NOT NULL CHECK(status IN (
      ${handbackStatusVocabulary.map((status) => `'${status}'`).join(", ")}
    )),
    claim TEXT NOT NULL,
    proof TEXT NOT NULL,
    assumptions TEXT NOT NULL,
    residual_risks TEXT NOT NULL,
    incidental_findings TEXT NOT NULL,
    created_at TEXT NOT NULL
  );
`;

const createHandbacksIndexSql =
  "CREATE INDEX IF NOT EXISTS handbacks_dispatch_id ON handbacks(dispatch_id)";

function handbacksSupportCouncilRequest(context: PluginContext): boolean {
  const schema = context.store.database
    .query<{ sql: string | null }, []>(
      "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'handbacks'",
    )
    .get()?.sql;
  return schema?.includes("'COUNCIL_REQUEST'") ?? false;
}

function migrateHandbackStatusVocabulary(context: PluginContext): void {
  if (context.store.readOnly || handbacksSupportCouncilRequest(context)) return;
  const migration = context.store.database.transaction(() => {
    if (handbacksSupportCouncilRequest(context)) return;
    context.store.database.exec("ALTER TABLE handbacks RENAME TO handbacks_legacy_statuses");
    context.store.database.exec(createHandbacksTableSql);
    context.store.database.exec("INSERT INTO handbacks SELECT * FROM handbacks_legacy_statuses");
    context.store.database.exec("DROP TABLE handbacks_legacy_statuses");
    context.store.database.exec(createHandbacksIndexSql);
  });
  migration.immediate();
}

function fromRow(row: DispatchRow): DispatchRecord {
  return {
    id: row.id,
    workId: row.work_id,
    objective: row.objective,
    ownedScope: row.owned_scope,
    excludedScope: row.excluded_scope,
    mutation: row.mutation,
    stopCondition: row.stop_condition,
    lane: row.lane,
    evidenceRequired: row.evidence_required,
    pane: row.pane ?? null,
    targetSession: row.target_session,
    openedBy: row.opened_by ?? null,
    claimedBy: row.claimed_by ?? null,
    heldBy: row.held_by,
    state: row.cancelled_at ? "cancelled" : row.returned ? "returned" : "open",
    cancelReason: row.cancel_reason,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

function getDispatch(context: PluginContext, id: string): DispatchRecord | null {
  const row = context.store.database
    .query<DispatchRow, [string]>(
      `SELECT dispatches.*,
              EXISTS(SELECT 1 FROM handbacks WHERE handbacks.dispatch_id = dispatches.id) AS returned
       FROM dispatches
       WHERE dispatches.id = ?`,
    )
    .get(id);
  return row ? fromRow(row) : null;
}

function requireDispatch(context: PluginContext, id: string): DispatchRecord {
  const dispatch = getDispatch(context, id);
  if (!dispatch) {
    throw new CliError("NOT_FOUND", `dispatch not found: ${id}; run: maestro dispatch list`, {
      command: "maestro dispatch list",
      id,
    });
  }
  return dispatch;
}

function listDispatches(context: PluginContext, workId?: string): DispatchRecord[] {
  const rows = workId
    ? context.store.database
        .query<DispatchRow, [string]>(
          `SELECT dispatches.*,
                  EXISTS(SELECT 1 FROM handbacks WHERE handbacks.dispatch_id = dispatches.id) AS returned
           FROM dispatches
           WHERE dispatches.work_id = ?
           ORDER BY CAST(SUBSTR(dispatches.id, 2) AS INTEGER)`,
        )
        .all(workId)
    : context.store.database
        .query<DispatchRow, []>(
          `SELECT dispatches.*,
                  EXISTS(SELECT 1 FROM handbacks WHERE handbacks.dispatch_id = dispatches.id) AS returned
           FROM dispatches
           ORDER BY CAST(SUBSTR(dispatches.id, 2) AS INTEGER)`,
        )
        .all();
  return rows.map(fromRow);
}

function fromHandbackRow(row: HandbackRow): HandbackRecord {
  return {
    id: row.id,
    dispatchId: row.dispatch_id,
    status: row.status,
    request: row.request ?? null,
    candidate: row.candidate ?? null,
    claim: row.claim,
    proof: row.proof,
    assumptions: row.assumptions,
    residualRisks: row.residual_risks,
    incidentalFindings: row.incidental_findings,
    createdAt: row.created_at,
  };
}

function getHandback(context: PluginContext, id: string): HandbackRecord | null {
  const row = context.store.database
    .query<HandbackRow, [string]>("SELECT * FROM handbacks WHERE id = ?")
    .get(id);
  return row ? fromHandbackRow(row) : null;
}

function requireHandback(context: PluginContext, id: string): HandbackRecord {
  const handback = getHandback(context, id);
  if (!handback) {
    throw new CliError("NOT_FOUND", `handback not found: ${id}`, { id });
  }
  return handback;
}

function listHandbacks(context: PluginContext, dispatchId: string): HandbackRecord[] {
  return context.store.database
    .query<HandbackRow, [string]>(
      "SELECT * FROM handbacks WHERE dispatch_id = ? ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)",
    )
    .all(dispatchId)
    .map(fromHandbackRow);
}

function councilStatus(
  context: PluginContext,
  workId: string,
  dispatchId?: string,
): CouncilStatus {
  const rows = context.store.database
    .query<CouncilDispatchRow, [string]>(
      `SELECT dispatches.id,
              dispatches.created_at,
              dispatches.cancelled_at,
              MIN(handbacks.created_at) AS returned_at
       FROM dispatches
       LEFT JOIN handbacks ON handbacks.dispatch_id = dispatches.id
       WHERE dispatches.work_id = ?
       GROUP BY dispatches.id, dispatches.created_at, dispatches.cancelled_at
       ORDER BY dispatches.created_at, CAST(SUBSTR(dispatches.id, 2) AS INTEGER)`,
    )
    .all(workId);
  const generations: Array<{ end: number; start: number }> = [];
  let generationStart = 0;
  let hasUnresolved = false;
  let latestResolution: string | null = null;
  for (const [index, row] of rows.entries()) {
    if (
      index > 0 &&
      !hasUnresolved &&
      latestResolution !== null &&
      latestResolution <= row.created_at
    ) {
      generations.push({ end: index, start: generationStart });
      generationStart = index;
    }
    const resolution = row.cancelled_at ?? row.returned_at;
    if (resolution === null) {
      hasUnresolved = true;
    } else if (latestResolution === null || resolution > latestResolution) {
      latestResolution = resolution;
    }
  }
  if (rows.length > 0) generations.push({ end: rows.length, start: generationStart });

  let selected = generations[generations.length - 1] ?? null;
  if (dispatchId) {
    const anchor = rows.findIndex((row) => row.id === dispatchId);
    selected = generations.find(({ end, start }) => anchor >= start && anchor < end) ?? null;
  } else {
    for (const generation of generations) {
      if (generation.end - generation.start > 1) selected = generation;
    }
  }
  const members = selected ? rows.slice(selected.start, selected.end) : [];
  const liveMembers = members.filter((row) => row.cancelled_at === null);
  const counts = {
    resolved: liveMembers.filter((row) => row.returned_at !== null).length,
    returned: liveMembers.filter((row) => row.returned_at !== null).length,
    total: liveMembers.length,
  };
  const generationAnchor = members[0]?.id ?? null;
  const hasGenerationAnchor = context.store.database
    .query<{ name: string }, []>("PRAGMA table_info(dispatch_councils)")
    .all()
    .some((column) => column.name === "generation_anchor");
  const unseal = generationAnchor
    ? hasGenerationAnchor
      ? context.store.database
          .query<{ unseal_reason: string }, [string, string]>(
            `SELECT unseal_reason FROM dispatch_councils
             WHERE work_id = ? AND generation_anchor = ?`,
          )
          .get(workId, generationAnchor)
      : generationAnchor === rows[0]?.id
        ? context.store.database
            .query<{ unseal_reason: string }, [string]>(
              "SELECT unseal_reason FROM dispatch_councils WHERE work_id = ?",
            )
            .get(workId)
        : null
    : null;
  const unsealed = unseal !== null;
  return {
    generationAnchor,
    workId,
    total: counts.total,
    returned: counts.returned,
    resolved: counts.resolved,
    sealed: counts.total > 1 && !unsealed && counts.resolved < counts.total,
    unsealed,
    unsealReason: unseal?.unseal_reason ?? null,
  };
}

function requiredOption(invocation: CliInvocation, name: string): string {
  const value = stringOption(invocation, name.slice(2));
  if (value === undefined || value.trim() === "") {
    throw new CliError("MISSING_ARGUMENT", `missing or blank ${name}`, { field: name });
  }
  return value;
}

function format(dispatch: DispatchRecord): string {
  return [
    `${dispatch.id} [${dispatch.state}]`,
    `work: ${dispatch.workId}`,
    `objective: ${dispatch.objective}`,
    `owned scope: ${dispatch.ownedScope}`,
    `excluded scope: ${dispatch.excludedScope}`,
    `mutation: ${dispatch.mutation}`,
    `stop condition: ${dispatch.stopCondition}`,
    `lane: ${dispatch.lane}`,
    `evidence required: ${dispatch.evidenceRequired}`,
    `pane: ${dispatch.pane ?? "none"}`,
    `target session: ${dispatch.targetSession ?? "none"}`,
    `opened by: ${dispatch.openedBy ?? "none"}`,
    `claimed by: ${dispatch.claimedBy ?? "none"}`,
    `held by: ${dispatch.heldBy ?? "none"}`,
    dispatch.cancelReason ? `cancel reason: ${dispatch.cancelReason}` : null,
  ]
    .filter((line): line is string => line !== null)
    .join("\n");
}

function formatHandback(handback: HandbackRecord): string {
  return [
    `${handback.id} [${handback.status}]`,
    `dispatch: ${handback.dispatchId}`,
    handback.request
      ? `${handback.status === "BLOCKED" ? "retry when" : "requested"}: ${handback.request}`
      : null,
    handback.candidate !== null ? `candidate: ${handback.candidate}` : null,
    `claim: ${handback.claim}`,
    `proof: ${handback.proof}`,
    `assumptions not verified: ${handback.assumptions}`,
    `residual risks: ${handback.residualRisks}`,
    `incidental findings: ${handback.incidentalFindings}`,
  ]
    .filter((line): line is string => line !== null)
    .join("\n");
}

function formatHandbackIds(handbacks: HandbackRecord[]): string | null {
  if (handbacks.length === 0) return null;
  const ids = handbacks.map(({ id }) => id).join(", ");
  return `${handbacks.length === 1 ? "handback" : "handbacks"}: ${ids}`;
}

function formatHandbackSummary(handback: HandbackRecord): string {
  return `${handback.id} [${handback.status}] ${handback.claim.split("\n")[0] ?? ""}`;
}

function formatCouncil(council: CouncilStatus): string | null {
  if (council.total < 2) return null;
  const state = council.unsealed ? "unsealed" : council.sealed ? "sealed" : "complete";
  return `council: ${state} (${council.returned}/${council.total} returned)`;
}

function formatLane(dispatch: DispatchRecord, workState: string, holderLive: boolean): string {
  const holderState = dispatch.heldBy ? holderLive ? "live" : "dead" : "none";
  return `lane ${dispatch.pane ?? "none"} | ${dispatch.id} | ${dispatch.lane} | dispatch=${dispatch.state} | work=${workState} | holder=${holderState}`;
}

export const dispatchPlugin: BuiltInPlugin = {
  name: "dispatch",
  inject: ["work"],
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS dispatches (
        id TEXT PRIMARY KEY,
        work_id TEXT NOT NULL REFERENCES work(id),
        objective TEXT NOT NULL,
        owned_scope TEXT NOT NULL,
        excluded_scope TEXT NOT NULL,
        mutation TEXT NOT NULL,
        stop_condition TEXT NOT NULL,
        lane TEXT NOT NULL,
        evidence_required TEXT NOT NULL,
        pane TEXT,
        target_session TEXT,
        opened_by TEXT,
        claimed_by TEXT,
        held_by TEXT,
        cancelled_at TEXT,
        cancel_reason TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS dispatches_work_id ON dispatches(work_id);
      ${createHandbacksTableSql}
      ${createHandbacksIndexSql};
      CREATE TABLE IF NOT EXISTS dispatch_councils (
        work_id TEXT NOT NULL REFERENCES work(id),
        generation_anchor TEXT NOT NULL REFERENCES dispatches(id),
        unsealed_at TEXT NOT NULL,
        unseal_reason TEXT NOT NULL,
        PRIMARY KEY(work_id, generation_anchor)
      );
    `);
    migrateHandbackStatusVocabulary(context);
    const hasCouncilGenerationAnchor = context.store.database
      .query<{ name: string }, []>("PRAGMA table_info(dispatch_councils)")
      .all()
      .some((column) => column.name === "generation_anchor");
    if (!context.store.readOnly && !hasCouncilGenerationAnchor) {
      const migrateCouncils = context.store.database.transaction(() => {
        const legacyRows = context.store.database
          .query<
            { unseal_reason: string; unsealed_at: string; work_id: string },
            []
          >("SELECT work_id, unsealed_at, unseal_reason FROM dispatch_councils")
          .all();
        context.store.database.exec(`
          ALTER TABLE dispatch_councils RENAME TO dispatch_councils_legacy;
          CREATE TABLE dispatch_councils (
            work_id TEXT NOT NULL REFERENCES work(id),
            generation_anchor TEXT NOT NULL REFERENCES dispatches(id),
            unsealed_at TEXT NOT NULL,
            unseal_reason TEXT NOT NULL,
            PRIMARY KEY(work_id, generation_anchor)
          );
        `);
        const insert = context.store.database.query(
          `INSERT INTO dispatch_councils
            (work_id, generation_anchor, unsealed_at, unseal_reason)
           VALUES (?, ?, ?, ?)`,
        );
        for (const row of legacyRows) {
          const anchor = context.store.database
            .query<{ id: string }, [string]>(
              `SELECT id FROM dispatches
               WHERE work_id = ?
               ORDER BY created_at, CAST(SUBSTR(id, 2) AS INTEGER)
               LIMIT 1`,
            )
            .get(row.work_id)?.id;
          if (anchor) insert.run(row.work_id, anchor, row.unsealed_at, row.unseal_reason);
        }
        context.store.database.exec("DROP TABLE dispatch_councils_legacy");
      });
      migrateCouncils.immediate();
    }
    context.store.ensureColumn(
      "dispatches",
      "pane",
      "ALTER TABLE dispatches ADD COLUMN pane TEXT",
    );
    context.store.ensureColumn(
      "dispatches",
      "opened_by",
      "ALTER TABLE dispatches ADD COLUMN opened_by TEXT",
    );
    context.store.ensureColumn(
      "dispatches",
      "claimed_by",
      "ALTER TABLE dispatches ADD COLUMN claimed_by TEXT",
    );
    context.store.ensureColumn(
      "handbacks",
      "request",
      "ALTER TABLE handbacks ADD COLUMN request TEXT",
    );
    context.store.ensureColumn(
      "handbacks",
      "candidate",
      "ALTER TABLE handbacks ADD COLUMN candidate TEXT",
    );
    context.store.migrate(`
      UPDATE dispatches
      SET held_by = NULL, claimed_by = NULL
      WHERE held_by IS NOT NULL
        AND (
          cancelled_at IS NOT NULL OR
          EXISTS(SELECT 1 FROM handbacks WHERE handbacks.dispatch_id = dispatches.id)
        )
    `);

    const service: DispatchService = {
      council: (workId, dispatchId) => councilStatus(context, workId, dispatchId),
      get: (id) => getDispatch(context, id),
      list: (workId) => listDispatches(context, workId),
    };
    context.effect(() => context.provide("dispatch", service));
    const handbackService: HandbackService = {
      get: (id) => getHandback(context, id),
      list: (dispatchId) => listHandbacks(context, dispatchId),
    };
    context.effect(() => context.provide("handback", handbackService));

    context.effect(() =>
      registerSessionCommand(
        context,
        "dispatch open",
        (invocation): CliResult => {
          const workId = requiredPosition(invocation, 0, "work id");
          const work = context.work as WorkService;
          const objective = requiredOption(invocation, "--objective");
          const ownedScope = requiredOption(invocation, "--owned-scope");
          const excludedScope = requiredOption(invocation, "--excluded-scope");
          const mutation = requiredOption(invocation, "--mutation");
          const stopCondition = requiredOption(invocation, "--stop-condition");
          const lane = requiredOption(invocation, "--lane");
          if (!dispatchLaneNames.some((allowed) => allowed === lane)) {
            throw new CliError(
              "INVALID_LANE",
              `invalid lane ${lane}; expected one of: ${dispatchLaneNames.join(", ")}`,
              { lane, lanes: dispatchLaneNames },
            );
          }
          const evidenceRequired = requiredOption(invocation, "--evidence-required");
          const pane = requiredOption(invocation, "--pane");
          const targetSessionValue = stringOption(invocation, "target-session");
          if (targetSessionValue !== undefined && targetSessionValue.trim() === "") {
            throw new CliError(
              "MISSING_ARGUMENT",
              "missing or blank --target-session",
              { field: "--target-session" },
            );
          }
          const targetSession = targetSessionValue ?? null;
          const openedBy = context.sessions.current().id;
          const open = context.store.database.transaction(() => {
            const subject = work.get(workId);
            if (!subject) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
            if (subject.state === "done" || subject.state === "cancelled") {
              throw new CliError(
                "INVALID_STATE",
                `${workId} is ${subject.state}; a lane contract binds live work`,
              );
            }
            const id = context.store.nextPrefixedId("dispatches", "x");
            const now = new Date().toISOString();
            context.store.database
              .query(
                `INSERT INTO dispatches
                  (id, work_id, objective, owned_scope, excluded_scope, mutation, stop_condition,
                   lane, evidence_required, pane, target_session, opened_by, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
              )
              .run(
                id,
                workId,
                objective,
                ownedScope,
                excludedScope,
                mutation,
                stopCondition,
                lane,
                evidenceRequired,
                pane,
                targetSession,
                openedBy,
                now,
                now,
              );
            context.log.append({
              type: "dispatch.open",
              entityType: "dispatch",
              entityId: id,
              sessionId: openedBy,
              payload: { pane, workId, targetSession },
            });
            return service.get(id) as DispatchRecord;
          });
          const created = open.immediate();
          return { data: { dispatch: created }, text: format(created) };
        },
        {
          description: "Store a complete lane contract on a work item.",
          flags: {
            "--objective": { description: "State the observable objective.", value: true },
            "--owned-scope": { description: "Name the lane-owned scope.", value: true },
            "--excluded-scope": { description: "Name the excluded scope.", value: true },
            "--mutation": { description: "Declare the mutation boundary.", value: true },
            "--stop-condition": { description: "State when the lane stops.", value: true },
            "--lane": { description: "Name the lane type.", value: true },
            "--evidence-required": { description: "Name the required evidence.", value: true },
            "--pane": { description: "Record the pane that owns the lane.", value: true },
            "--target-session": { description: "Address a session if already known.", value: true },
          },
          positionals: [{ name: "work-id", required: true }],
          rootDescription: "Store lane contracts and their return packets.",
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "dispatch accept",
        (invocation): CliResult => {
          const id = requiredPosition(invocation, 0, "dispatch id");
          const sessionId = context.sessions.current().id;
          const accept = context.store.database.transaction(() => {
            const dispatch = requireDispatch(context, id);
            if (dispatch.state !== "open") {
              throw new CliError("INVALID_STATE", `${id} is ${dispatch.state}`);
            }
            if (dispatch.targetSession && dispatch.targetSession !== sessionId) {
              throw new CliError(
                "TARGET_MISMATCH",
                `${id} targets ${dispatch.targetSession}; current session is ${sessionId}`,
                { id, targetSession: dispatch.targetSession },
              );
            }
            if (dispatch.heldBy && dispatch.heldBy !== sessionId) {
              throw new CliError("DISPATCH_HELD", `${id} is held by ${dispatch.heldBy}`, {
                heldBy: dispatch.heldBy,
                id,
              });
            }
            if (dispatch.claimedBy && dispatch.claimedBy !== sessionId) {
              throw new CliError("DISPATCH_CLAIMED", `${id} is claimed by ${dispatch.claimedBy}`, {
                claimedBy: dispatch.claimedBy,
                id,
              });
            }
            if (!dispatch.heldBy && !dispatch.claimedBy) {
              const now = new Date().toISOString();
              const column = dispatch.targetSession ? "held_by" : "claimed_by";
              context.store.database
                .query(`UPDATE dispatches SET ${column} = ?, updated_at = ? WHERE id = ?`)
                .run(sessionId, now, id);
              context.log.append({
                type: "dispatch.accept",
                entityType: "dispatch",
                entityId: id,
                sessionId,
              });
            }
            return service.get(id) as DispatchRecord;
          });
          const accepted = accept.immediate();
          return { data: { dispatch: accepted }, text: format(accepted) };
        },
        {
          description: "Accept a dispatch without taking the work write lease.",
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "dispatch confirm",
        (invocation): CliResult => {
          const id = requiredPosition(invocation, 0, "dispatch id");
          const claimedSession = requiredOption(invocation, "--session");
          const openerSession = context.sessions.current().id;
          const confirm = context.store.database.transaction(() => {
            const dispatch = requireDispatch(context, id);
            if (dispatch.state !== "open") {
              throw new CliError("INVALID_STATE", `${id} is ${dispatch.state}`);
            }
            if (dispatch.openedBy !== openerSession) {
              throw new CliError(
                "DISPATCH_CONFIRM_FORBIDDEN",
                `${id} was opened by ${dispatch.openedBy ?? "an unknown session"}; current session is ${openerSession}`,
                { currentSession: openerSession, id, openedBy: dispatch.openedBy },
              );
            }
            if (dispatch.claimedBy !== claimedSession) {
              throw new CliError(
                "CLAIM_MISMATCH",
                `${id} is claimed by ${dispatch.claimedBy ?? "no session"}; requested confirmation is ${claimedSession}`,
                { claimedBy: dispatch.claimedBy, id, requestedSession: claimedSession },
              );
            }
            const now = new Date().toISOString();
            const updated = context.store.database
              .query(
                `UPDATE dispatches
                 SET claimed_by = NULL, held_by = ?, updated_at = ?
                 WHERE id = ? AND claimed_by = ? AND held_by IS NULL`,
              )
              .run(claimedSession, now, id, claimedSession);
            if (updated.changes !== 1) {
              throw new CliError("INVALID_STATE", `${id} changed while confirmation was pending`);
            }
            context.log.append({
              type: "dispatch.confirm",
              entityType: "dispatch",
              entityId: id,
              sessionId: openerSession,
              payload: { sessionId: claimedSession },
            });
            return service.get(id) as DispatchRecord;
          });
          const confirmed = confirm.immediate();
          return { data: { dispatch: confirmed }, text: format(confirmed) };
        },
        {
          description: "Confirm an untargeted dispatch claim as its opener.",
          flags: { "--session": { description: "Name the claimed session to confirm.", value: true } },
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "dispatch cancel",
        (invocation): CliResult => {
          const id = requiredPosition(invocation, 0, "dispatch id");
          const reason = requiredOption(invocation, "--reason");
          const cancel = context.store.database.transaction(() => {
            const dispatch = requireDispatch(context, id);
            if (dispatch.state === "returned") {
              throw new CliError("HANDBACK_EXISTS", `${id} already has a handback`, {
                dispatchId: id,
              });
            }
            if (dispatch.state !== "open") {
              throw new CliError("INVALID_STATE", `${id} is ${dispatch.state}`);
            }
            const now = new Date().toISOString();
            const cancelled = context.store.database
              .query(
                `UPDATE dispatches
                 SET cancelled_at = ?, cancel_reason = ?, claimed_by = NULL, held_by = NULL,
                     updated_at = ?
                 WHERE id = ? AND cancelled_at IS NULL
                   AND NOT EXISTS(SELECT 1 FROM handbacks WHERE dispatch_id = dispatches.id)`,
              )
              .run(now, reason, now, id);
            if (cancelled.changes === 0) {
              throw new CliError("INVALID_STATE", `${id} changed while cancellation was pending`);
            }
            context.log.append({
              type: "dispatch.cancel",
              entityType: "dispatch",
              entityId: id,
              sessionId: context.sessions.current().id,
              payload: { reason },
            });
            return service.get(id) as DispatchRecord;
          });
          const cancelled = cancel.immediate();
          return { data: { dispatch: cancelled }, text: format(cancelled) };
        },
        {
          description: "Cancel a dispatch while keeping its row and reason.",
          flags: { "--reason": { description: "Record why the lane was abandoned.", value: true } },
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "dispatch show",
        (invocation): CliResult => {
          const dispatch = requireDispatch(context, requiredPosition(invocation, 0, "dispatch id"));
          const handbacks = handbackService.list(dispatch.id);
          return {
            data: { dispatch, handbacks },
            text: [format(dispatch), formatHandbackIds(handbacks)]
              .filter((line): line is string => line !== null)
              .join("\n"),
          };
        },
        {
          description: "Show one stored dispatch contract.",
          mutates: false,
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "dispatch list",
        (invocation): CliResult => {
          const workId = invocation.positionals[0];
          if (workId) {
            const work = context.work as WorkService;
            const subject = work.get(workId);
            if (!subject) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
            const dispatches = service.list(workId);
            const council = service.council(workId);
            const councilLine = formatCouncil(council);
            const lanes = dispatches.map((dispatch) =>
              formatLane(
                dispatch,
                subject.state,
                dispatch.heldBy ? context.sessions.isAlive(dispatch.heldBy) : false,
              )
            );
            return {
              data: { dispatches, council },
              text: [councilLine, lanes.join("\n")]
                .filter((part): part is string => Boolean(part))
                .join("\n\n"),
            };
          }
          const dispatches = service.list();
          return {
            data: { dispatches, council: null },
            text: dispatches.map(format).join("\n\n"),
          };
        },
        {
          description: "List dispatch contracts, optionally for one work item.",
          mutates: false,
          positionals: [{ name: "work-id", required: false }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "dispatch unseal",
        (invocation): CliResult => {
          const workId = requiredPosition(invocation, 0, "work id");
          const work = context.work as WorkService;
          if (!work.get(workId)) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
          const reason = requiredOption(invocation, "--reason");
          const unseal = context.store.database.transaction(() => {
            const council = service.council(workId);
            if (council.total < 2) {
              throw new CliError("INVALID_STATE", `${workId} has no council to unseal`);
            }
            if (council.unsealed) {
              throw new CliError("INVALID_STATE", `${workId} council is already unsealed`);
            }
            if (!council.sealed || !council.generationAnchor) {
              throw new CliError("INVALID_STATE", `${workId} council is complete`);
            }
            const unsealedAt = new Date().toISOString();
            context.store.database
              .query(
                `INSERT INTO dispatch_councils
                  (work_id, generation_anchor, unsealed_at, unseal_reason)
                 VALUES (?, ?, ?, ?)`,
              )
              .run(workId, council.generationAnchor, unsealedAt, reason);
            context.log.append({
              type: "dispatch.unseal",
              entityType: "work",
              entityId: workId,
              sessionId: context.sessions.current().id,
              payload: { generationAnchor: council.generationAnchor, reason },
            });
            return service.council(workId);
          });
          const opened = unseal.immediate();
          return {
            data: { council: opened },
            text: `${formatCouncil(opened)}\nreason: ${reason}`,
          };
        },
        {
          description: "Open a sealed council early and record why.",
          flags: { "--reason": { description: "Record why the council opened early.", value: true } },
          positionals: [{ name: "work-id", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "handback file",
        (invocation): CliResult => {
          const dispatchId = requiredPosition(invocation, 0, "dispatch id");
          const dispatch = requireDispatch(context, dispatchId);
          if (dispatch.state === "cancelled") {
            throw new CliError("INVALID_STATE", `${dispatchId} is cancelled`);
          }
          const status = requiredOption(invocation, "--status");
          if (!handbackStatusVocabulary.includes(status as HandbackStatus)) {
            throw new CliError(
              "INVALID_STATUS",
              `invalid handback status ${status}; expected one of: ${handbackStatusVocabulary.join(", ")}`,
              { statuses: handbackStatusVocabulary },
            );
          }
          const requestValue = stringOption(invocation, "request");
          const request = handbackRequestStatuses.has(status as HandbackStatus)
            ? requiredOption(invocation, "--request")
            : requestValue?.trim() ? requestValue : null;
          const candidate = stringOption(invocation, "candidate") ?? null;
          const claim = requiredOption(invocation, "--claim");
          const proof = requiredOption(invocation, "--proof");
          const layers = ["source", "artifact", "installed", "live", "journey"];
          if (!layers.some((layer) => new RegExp(`\\b${layer}\\b`, "i").test(proof))) {
            throw new CliError(
              "EVIDENCE_LAYER_REQUIRED",
              `proof must name an evidence layer: ${layers.join(", ")}`,
              { layers },
            );
          }
          const assumptions = requiredOption(invocation, "--assumptions");
          const residualRisks = requiredOption(invocation, "--residual-risks");
          const incidentalFindings = requiredOption(invocation, "--incidental-findings");
          const sessionId = context.sessions.current().id;
          const file = context.store.database.transaction(() => {
            const current = requireDispatch(context, dispatchId);
            if (current.state === "cancelled") {
              throw new CliError("INVALID_STATE", `${dispatchId} is cancelled`);
            }
            if (current.state === "returned") {
              const latest = listHandbacks(context, dispatchId).at(-1);
              throw new CliError(
                "HANDBACK_EXISTS",
                `${dispatchId} already returned ${latest?.id ?? "a handback"}; a second stop point is a second dispatch: ask the Lead ([from peer][${dispatchId}]) or record late evidence with: maestro work note ${current.workId} "after ${latest?.id ?? "<h-id>"}: <evidence>"`,
                { dispatchId, handbackId: latest?.id, workId: current.workId },
              );
            }
            if (current.claimedBy) {
              const command = `maestro dispatch confirm ${dispatchId} --session ${current.claimedBy}`;
              throw new CliError(
                "DISPATCH_UNCONFIRMED",
                `${dispatchId} is claimed but not held; the opener must run: ${command}`,
                { claimedBy: current.claimedBy, command, dispatchId },
              );
            }
            if (!current.heldBy) {
              throw new CliError(
                "DISPATCH_UNHELD",
                `${dispatchId} has not been accepted; run: maestro dispatch accept ${dispatchId}`,
                { command: `maestro dispatch accept ${dispatchId}`, dispatchId },
              );
            }
            if (current.heldBy !== sessionId) {
              throw new CliError(
                "DISPATCH_HELD",
                `${dispatchId} is held by ${current.heldBy}; current session is ${sessionId}`,
                { currentSession: sessionId, dispatchId, heldBy: current.heldBy },
              );
            }
            const id = context.store.nextPrefixedId("handbacks", "h");
            const createdAt = new Date().toISOString();
            context.store.database
              .query(
                `INSERT INTO handbacks
                  (id, dispatch_id, status, request, candidate, claim, proof, assumptions,
                   residual_risks, incidental_findings, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
              )
              .run(
                id,
                dispatchId,
                status,
                request,
                candidate,
                claim,
                proof,
                assumptions,
                residualRisks,
                incidentalFindings,
                createdAt,
              );
            context.store.database
              .query("UPDATE dispatches SET held_by = NULL, updated_at = ? WHERE id = ?")
              .run(createdAt, dispatchId);
            context.log.append({
              type: "handback.file",
              entityType: "handback",
              entityId: id,
              sessionId,
              payload: { dispatchId, status },
            });
            return handbackService.get(id) as HandbackRecord;
          });
          const filed = file.immediate();
          return { data: { handback: filed }, text: formatHandback(filed) };
        },
        {
          description: "File a shape-checked return packet for a dispatch.",
          flags: {
            "--status": { description: "Set the handback status.", value: true },
            "--request": { description: "State the retry condition or requested action.", value: true },
            "--candidate": { description: "Record an opaque commit or digest.", value: true },
            "--claim": { description: "State what is now believed true.", value: true },
            "--proof": { description: "Name layered evidence for the claim.", value: true },
            "--assumptions": { description: "List unverified assumptions or None.", value: true },
            "--residual-risks": { description: "List residual risks or None.", value: true },
            "--incidental-findings": {
              description: "List incidental findings or None.",
              value: true,
            },
          },
          positionals: [{ name: "dispatch-id", required: true }],
          rootDescription: "File, list, and read durable return packets.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "handback list",
        (invocation): CliResult => {
          const id = requiredPosition(invocation, 0, "dispatch or work id");
          const dispatch = service.get(id);
          const handbacks = dispatch
            ? handbackService.list(dispatch.id)
            : (() => {
                const work = (context.work as WorkService).get(id);
                if (!work) {
                  throw new CliError("NOT_FOUND", `dispatch or work not found: ${id}`, { id });
                }
                return service
                  .list(id)
                  .flatMap(({ id: dispatchId }) => handbackService.list(dispatchId))
                  .sort((left, right) => Number(left.id.slice(1)) - Number(right.id.slice(1)));
              })();
          // A sealed council hides its first views from everyone, including
          // this list and its --json form; only the id and the seal show.
          const sealed: string[] = [];
          const visible = handbacks.filter((handback) => {
            const owner = service.get(handback.dispatchId);
            if (!owner || !service.council(owner.workId, owner.id).sealed) return true;
            sealed.push(handback.id);
            return false;
          });
          return {
            data: { handbacks: visible, sealed },
            text: [
              ...visible.map(formatHandbackSummary),
              ...sealed.map((id) => `${id} [SEALED] council not yet returned`),
            ].join("\n"),
          };
        },
        {
          description: "List handbacks for one dispatch or work item.",
          mutates: false,
          positionals: [{ name: "dispatch-or-work-id", required: true }],
          rootDescription: "File, list, and read durable return packets.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "handback show",
        (invocation): CliResult => {
          const id = requiredPosition(invocation, 0, "handback or dispatch id");
          const handback = handbackService.get(id) ?? (() => {
            const dispatch = service.get(id);
            if (!dispatch) return requireHandback(context, id);
            const latest = handbackService.list(dispatch.id).at(-1);
            if (!latest) {
              throw new CliError("NOT_FOUND", `dispatch has no handback: ${id}`, { id });
            }
            return latest;
          })();
          const dispatch = requireDispatch(context, handback.dispatchId);
          const council = service.council(dispatch.workId, dispatch.id);
          if (council.sealed) {
            throw new CliError(
              "SEALED",
              `council ${dispatch.workId} is SEALED (${council.returned}/${council.total} returned)`,
              {
                returned: council.returned,
                total: council.total,
                workId: dispatch.workId,
              },
            );
          }
          const councilLine = formatCouncil(council);
          return {
            data: { handback, council: councilLine ? council : null },
            text: [formatHandback(handback), councilLine, council.unsealReason ? `reason: ${council.unsealReason}` : null]
              .filter((line): line is string => line !== null)
              .join("\n"),
          };
        },
        {
          description: "Show the handback for a handback or dispatch id.",
          mutates: false,
          positionals: [{ name: "id", required: true }],
        },
      ),
    );
  },
};
