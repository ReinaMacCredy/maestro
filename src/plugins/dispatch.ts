import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { registerSessionCommand } from "./session-required.ts";
import type { WorkService } from "./work.ts";

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
  held_by: string | null;
  cancelled_at: string | null;
  cancel_reason: string | null;
  created_at: string;
  updated_at: string;
  returned: number;
}

export type HandbackStatus =
  | "DONE"
  | "BLOCKED"
  | "UNTESTABLE"
  | "UNKNOWN"
  | "FAILED"
  | "CHALLENGE"
  | "REOPEN_REQUEST"
  | "DEPENDENCY_REQUEST";

export interface HandbackRecord {
  id: string;
  dispatchId: string;
  status: HandbackStatus;
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
  claim: string;
  proof: string;
  assumptions: string;
  residual_risks: string;
  incidental_findings: string;
  created_at: string;
}

export interface DispatchService {
  council(workId: string): CouncilStatus;
  get(id: string): DispatchRecord | null;
  list(workId?: string): DispatchRecord[];
}

export interface HandbackService {
  get(id: string): HandbackRecord | null;
  list(dispatchId: string): HandbackRecord[];
}

export interface CouncilStatus {
  workId: string;
  total: number;
  returned: number;
  resolved: number;
  sealed: boolean;
  unsealed: boolean;
  unsealReason: string | null;
}

const handbackStatuses: readonly HandbackStatus[] = [
  "DONE",
  "BLOCKED",
  "UNTESTABLE",
  "UNKNOWN",
  "FAILED",
  "CHALLENGE",
  "REOPEN_REQUEST",
  "DEPENDENCY_REQUEST",
];

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

function councilStatus(context: PluginContext, workId: string): CouncilStatus {
  const counts = context.store.database
    .query<{ resolved: number; returned: number; total: number }, [string]>(
      `SELECT COUNT(*) AS total,
              COALESCE(SUM(CASE WHEN returned.dispatch_id IS NOT NULL THEN 1 ELSE 0 END), 0) AS returned,
              COALESCE(SUM(CASE
                WHEN returned.dispatch_id IS NOT NULL OR dispatches.cancelled_at IS NOT NULL
                THEN 1 ELSE 0 END), 0) AS resolved
       FROM dispatches
       LEFT JOIN (SELECT DISTINCT dispatch_id FROM handbacks) AS returned
         ON returned.dispatch_id = dispatches.id
       WHERE dispatches.work_id = ?`,
    )
    .get(workId) ?? { resolved: 0, returned: 0, total: 0 };
  const unseal = context.store.database
    .query<{ unseal_reason: string }, [string]>(
      "SELECT unseal_reason FROM dispatch_councils WHERE work_id = ?",
    )
    .get(workId);
  const unsealed = unseal !== null;
  return {
    workId,
    total: counts.total,
    returned: counts.returned,
    resolved: counts.resolved,
    sealed: counts.total > 1 && !unsealed && counts.resolved < counts.total,
    unsealed,
    unsealReason: unseal?.unseal_reason ?? null,
  };
}

function nextId(context: PluginContext): string {
  const next =
    context.store.database
      .query<{ next: number }, []>(
        "SELECT COALESCE(MAX(CAST(SUBSTR(id, 2) AS INTEGER)), 0) + 1 AS next FROM dispatches",
      )
      .get()?.next ?? 1;
  return `x${next}`;
}

function nextHandbackId(context: PluginContext): string {
  const next =
    context.store.database
      .query<{ next: number }, []>(
        "SELECT COALESCE(MAX(CAST(SUBSTR(id, 2) AS INTEGER)), 0) + 1 AS next FROM handbacks",
      )
      .get()?.next ?? 1;
  return `h${next}`;
}

function position(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function option(invocation: CliInvocation, name: string): string | null {
  const value = invocation.options[name];
  return typeof value === "string" ? value : null;
}

function requiredOption(invocation: CliInvocation, name: string): string {
  const value = option(invocation, name.slice(2));
  if (value === null || value.trim() === "") {
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
    `claim: ${handback.claim}`,
    `proof: ${handback.proof}`,
    `assumptions not verified: ${handback.assumptions}`,
    `residual risks: ${handback.residualRisks}`,
    `incidental findings: ${handback.incidentalFindings}`,
  ].join("\n");
}

function formatCouncil(council: CouncilStatus): string | null {
  if (council.total < 2) return null;
  const state = council.unsealed ? "unsealed" : council.sealed ? "sealed" : "complete";
  return `council: ${state} (${council.returned}/${council.total} returned)`;
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
        held_by TEXT,
        cancelled_at TEXT,
        cancel_reason TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS dispatches_work_id ON dispatches(work_id);
      CREATE TABLE IF NOT EXISTS handbacks (
        id TEXT PRIMARY KEY,
        dispatch_id TEXT NOT NULL REFERENCES dispatches(id),
        status TEXT NOT NULL CHECK(status IN (
          'DONE', 'BLOCKED', 'UNTESTABLE', 'UNKNOWN', 'FAILED', 'CHALLENGE',
          'REOPEN_REQUEST', 'DEPENDENCY_REQUEST'
        )),
        claim TEXT NOT NULL,
        proof TEXT NOT NULL,
        assumptions TEXT NOT NULL,
        residual_risks TEXT NOT NULL,
        incidental_findings TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS handbacks_dispatch_id ON handbacks(dispatch_id);
      CREATE TABLE IF NOT EXISTS dispatch_councils (
        work_id TEXT PRIMARY KEY REFERENCES work(id),
        unsealed_at TEXT NOT NULL,
        unseal_reason TEXT NOT NULL
      );
    `);
    const hasDispatchColumn = (name: string) =>
      context.store.database
        .query<{ name: string }, []>("PRAGMA table_info(dispatches)")
        .all()
        .some((column) => column.name === name);
    if (!hasDispatchColumn("pane")) {
      try {
        context.store.migrate("ALTER TABLE dispatches ADD COLUMN pane TEXT");
      } catch (error) {
        if (!hasDispatchColumn("pane")) throw error;
      }
    }
    if (!context.store.readOnly && !hasDispatchColumn("terminal_held_by_backfilled")) {
      try {
        const backfill = context.store.database.transaction(() => {
          context.store.migrate(
            "ALTER TABLE dispatches ADD COLUMN terminal_held_by_backfilled INTEGER NOT NULL DEFAULT 1",
          );
          context.store.database
            .query(
              `UPDATE dispatches
               SET held_by = NULL
               WHERE held_by IS NOT NULL
                 AND (
                   cancelled_at IS NOT NULL OR
                   EXISTS(SELECT 1 FROM handbacks WHERE handbacks.dispatch_id = dispatches.id)
                 )`,
            )
            .run();
        });
        backfill.immediate();
      } catch (error) {
        if (!hasDispatchColumn("terminal_held_by_backfilled")) throw error;
      }
    }

    const service: DispatchService = {
      council: (workId) => councilStatus(context, workId),
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
          const workId = position(invocation, 0, "work id");
          const work = context.work as WorkService;
          const subject = work.get(workId);
          if (!subject) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
          if (subject.state === "done" || subject.state === "cancelled") {
            throw new CliError(
              "INVALID_STATE",
              `${workId} is ${subject.state}; a lane contract binds live work`,
            );
          }
          const objective = requiredOption(invocation, "--objective");
          const ownedScope = requiredOption(invocation, "--owned-scope");
          const excludedScope = requiredOption(invocation, "--excluded-scope");
          const mutation = requiredOption(invocation, "--mutation");
          const stopCondition = requiredOption(invocation, "--stop-condition");
          const lane = requiredOption(invocation, "--lane");
          const evidenceRequired = requiredOption(invocation, "--evidence-required");
          const pane = requiredOption(invocation, "--pane");
          const targetSession = option(invocation, "target-session");
          const id = nextId(context);
          const now = new Date().toISOString();
          context.store.database
            .query(
              `INSERT INTO dispatches
                (id, work_id, objective, owned_scope, excluded_scope, mutation, stop_condition,
                 lane, evidence_required, pane, target_session, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
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
              now,
              now,
            );
          context.log.append({
            type: "dispatch.open",
            entityType: "dispatch",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { pane, workId, targetSession },
          });
          const created = service.get(id) as DispatchRecord;
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
          const id = position(invocation, 0, "dispatch id");
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
            if (!dispatch.heldBy) {
              const now = new Date().toISOString();
              context.store.database
                .query("UPDATE dispatches SET held_by = ?, updated_at = ? WHERE id = ?")
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
        "dispatch cancel",
        (invocation): CliResult => {
          const id = position(invocation, 0, "dispatch id");
          const dispatch = requireDispatch(context, id);
          if (dispatch.state !== "open") {
            throw new CliError("INVALID_STATE", `${id} is ${dispatch.state}`);
          }
          const reason = requiredOption(invocation, "--reason");
          const now = new Date().toISOString();
          context.store.database
            .query(
              "UPDATE dispatches SET cancelled_at = ?, cancel_reason = ?, held_by = NULL, updated_at = ? WHERE id = ?",
            )
            .run(now, reason, now, id);
          context.log.append({
            type: "dispatch.cancel",
            entityType: "dispatch",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { reason },
          });
          const cancelled = service.get(id) as DispatchRecord;
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
          const dispatch = requireDispatch(context, position(invocation, 0, "dispatch id"));
          return { data: { dispatch }, text: format(dispatch) };
        },
        {
          description: "Show one stored dispatch contract.",
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
            if (!work.get(workId)) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
          }
          const dispatches = service.list(workId);
          const council = workId ? service.council(workId) : null;
          const councilLine = council ? formatCouncil(council) : null;
          return {
            data: { dispatches, council },
            text: [councilLine, dispatches.map(format).join("\n\n")]
              .filter((part): part is string => Boolean(part))
              .join("\n\n"),
          };
        },
        {
          description: "List dispatch contracts, optionally for one work item.",
          positionals: [{ name: "work-id", required: false }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "dispatch unseal",
        (invocation): CliResult => {
          const workId = position(invocation, 0, "work id");
          const work = context.work as WorkService;
          if (!work.get(workId)) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
          const council = service.council(workId);
          if (council.total < 2) {
            throw new CliError("INVALID_STATE", `${workId} has no council to unseal`);
          }
          if (council.unsealed) {
            throw new CliError("INVALID_STATE", `${workId} council is already unsealed`);
          }
          const reason = requiredOption(invocation, "--reason");
          const unsealedAt = new Date().toISOString();
          context.store.database
            .query(
              "INSERT INTO dispatch_councils(work_id, unsealed_at, unseal_reason) VALUES (?, ?, ?)",
            )
            .run(workId, unsealedAt, reason);
          context.log.append({
            type: "dispatch.unseal",
            entityType: "work",
            entityId: workId,
            sessionId: context.sessions.current().id,
            payload: { reason },
          });
          const opened = service.council(workId);
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
          const dispatchId = position(invocation, 0, "dispatch id");
          const dispatch = requireDispatch(context, dispatchId);
          if (dispatch.state === "cancelled") {
            throw new CliError("INVALID_STATE", `${dispatchId} is cancelled`);
          }
          const status = requiredOption(invocation, "--status");
          if (!handbackStatuses.includes(status as HandbackStatus)) {
            throw new CliError(
              "INVALID_STATUS",
              `invalid handback status ${status}; expected one of: ${handbackStatuses.join(", ")}`,
              { statuses: handbackStatuses },
            );
          }
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
          const id = nextHandbackId(context);
          const createdAt = new Date().toISOString();
          const sessionId = context.sessions.current().id;
          const work = context.work as WorkService;
          context.store.database.transaction(() => {
            context.store.database
              .query(
                `INSERT INTO handbacks
                  (id, dispatch_id, status, claim, proof, assumptions, residual_risks,
                   incidental_findings, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
              )
              .run(
                id,
                dispatchId,
                status,
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
            work.release(dispatch.workId, sessionId, createdAt);
            context.log.append({
              type: "handback.file",
              entityType: "handback",
              entityId: id,
              sessionId,
              payload: { dispatchId, status },
            });
          })();
          const filed = handbackService.get(id) as HandbackRecord;
          return { data: { handback: filed }, text: formatHandback(filed) };
        },
        {
          description: "File a shape-checked return packet for a dispatch.",
          flags: {
            "--status": { description: "Set the handback status.", value: true },
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
          rootDescription: "File and read durable return packets.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "handback show",
        (invocation): CliResult => {
          const handback = requireHandback(context, position(invocation, 0, "handback id"));
          const dispatch = requireDispatch(context, handback.dispatchId);
          const council = service.council(dispatch.workId);
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
          description: "Show one stored handback packet.",
          positionals: [{ name: "id", required: true }],
        },
      ),
    );
  },
};
