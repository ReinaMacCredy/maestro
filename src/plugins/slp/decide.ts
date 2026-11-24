import { CliError, requiredPosition, stringOption, type CliInvocation, type CliResult } from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { Store, resolveStoreLocation, tableExists } from "../../kernel/store.ts";
import { isRoom } from "../room.ts";
import type { SlpRole } from "./runtime-plan.ts";
import {
  activeLocalTeam,
  type ActiveLocalTeam,
  clipLine,
  hubReviewerName,
  markLessonCandidate,
  pushNotice,
  requireRunningGeneration,
  requireSlpActor,
  type SlpActor,
  type SlpReviewerActor,
} from "./context.ts";
import { migrateProject, migrateRoom } from "./migrations.ts";

interface ProvisionalDecision {
  choice: string;
  createdAt: string;
  generation: number;
  id: string;
  teamId: string;
  workId: string | null;
}

// The column may be missing on a room store no Hub command has migrated since
// the update; a reader never fails on it.
function provisionalColumn(store: Store): string {
  return store.hasColumn("slp_decisions", "provisional") ? "provisional" : "0 AS provisional";
}

export function provisionalDecisions(store: Store): ProvisionalDecision[] {
  if (!tableExists(store, "slp_decisions") || !store.hasColumn("slp_decisions", "provisional")) {
    return [];
  }
  return store.database
    .query<{
      choice: string;
      created_at: string;
      generation: number;
      id: string;
      team_id: string;
      work_id: string | null;
    }, []>(
      `SELECT id, team_id, generation, work_id, choice, created_at
       FROM slp_decisions WHERE provisional = 1 ORDER BY created_at, id`,
    )
    .all()
    .map((row) => ({
      choice: row.choice,
      createdAt: row.created_at,
      generation: row.generation,
      id: row.id,
      teamId: row.team_id,
      workId: row.work_id,
    }));
}

// A Hub pane not named `supervisor` never receives the BLOCKED push, so a standing escalation has to show in the store.
export function blockedForHub(
  room: Store,
  project: Store,
  team: { generation: number; team_id: string },
  roles: ReadonlyArray<{ name: string; role: string }>,
): Array<{ body: string; workId: string }> {
  const escalator = roles.find((role) => role.role === "lead");
  if (!escalator || !tableExists(project, "slp_work_entries")) return [];
  if (!project.hasColumn("slp_work_entries", "flag")) return [];
  const notes = project.database
    .query<{ body: string; created_at: string; work_id: string }, [string, number, string, string]>(
      `SELECT e.work_id, e.body, e.created_at FROM slp_work_entries e
       JOIN slp_work w ON w.id = e.work_id
       WHERE w.team_id = ? AND w.generation = ? AND e.actor = ? AND e.flag = 'blocked'
         AND e.id = (SELECT MAX(x.id) FROM slp_work_entries x WHERE x.work_id = e.work_id AND x.actor = ?)
       ORDER BY e.id`,
    )
    .all(team.team_id, team.generation, escalator.name, escalator.name);
  const ruled = room.database.query<{ present: number }, [string, number, string, string]>(
    `SELECT 1 AS present FROM slp_decisions
     WHERE team_id = ? AND generation = ? AND work_id = ? AND created_at > ? LIMIT 1`,
  );
  return notes
    .filter((note) => !ruled.get(team.team_id, team.generation, note.work_id, note.created_at))
    .map((note) => ({ body: note.body, workId: note.work_id }));
}

export function provisionalLines(decisions: ProvisionalDecision[]): string[] {
  return decisions.map((decision) =>
    `provisional: ${decision.id} ${decision.workId ?? "none"} ${clipLine(decision.choice, 120)}`
  );
}

// w740 (room d154, d899): decide is the fifth waking operation. A ruling on an
// item is an instruction to the seat holding it, so the line goes to the
// assignee and not to the reviewer; that is what carries the Hub's order down
// to the Lead, which d154 relies on and no code sent before. A ruling
// with no item, or one a seat makes on its own item, wakes nobody.
async function pushDecision(
  store: Store,
  projectPath: string,
  actor: SlpRole | typeof hubReviewerName,
  actorName: string,
  workId: string,
  choice: string,
): Promise<void> {
  if (!tableExists(store, "slp_work")) return;
  const assignee = store.database
    .query<{ assigned_to: string }, [string]>("SELECT assigned_to FROM slp_work WHERE id = ?")
    .get(workId)?.assigned_to;
  if (!assignee || assignee === actorName) return;
  await pushNotice(projectPath, actor, assignee, `${workId} DECIDED`, choice, `maestro status ${workId}`);
}

// The Hub rules from the room store, which holds no slp_work; the item lives in
// the project store, so the assignee is read there, read-only, the way the
// lesson-candidate mark is written there.
async function pushHubDecision(target: HubWorkTarget, choice: string): Promise<void> {
  let store: Store;
  try {
    store = new Store(resolveStoreLocation(target.projectPath).path, { readonly: true });
  } catch {
    return;
  }
  try {
    await pushDecision(store, target.projectPath, hubReviewerName, hubReviewerName, target.workId, choice);
  } finally {
    store.close();
  }
}

interface HubWorkTarget {
  generation: number;
  projectPath: string;
  teamId: string;
  workId: string;
}

// w736 F3: a read reaches a stopped generation, a write does not. Every item
// the Hub accepted becomes unreadable the moment the team stops if the
// resolver only ever looks at RUNNING teams, and "SLP work not found from Hub"
// is the wrong sentence for a record that exists and is finished. Writing to a
// stopped generation stays refused - the seats are gone and the store is
// closed - but the refusal now names the state instead of denying the id.
type HubWorkAccess = "read" | "write";

interface HubTeamRow {
  generation: number;
  project_path: string;
  state: string;
  team_id: string;
}

// `tolerant` is the archive pass: the room remembers every generation it ever
// ran and some of those checkouts have since been deleted or moved, so a store
// that will not open there is skipped rather than failing the whole read. The
// RUNNING pass keeps the loud failure, because a live team whose project is
// unreachable is a fault worth stopping on.
function hubWorkMatches(
  teams: HubTeamRow[],
  workId: string,
  tolerant = false,
): HubWorkTarget[] {
  const matches: HubWorkTarget[] = [];
  for (const team of teams) {
    let projectStore: Store;
    try {
      projectStore = new Store(resolveStoreLocation(team.project_path).path, { readonly: true });
    } catch (error) {
      if (tolerant) continue;
      throw new CliError(
        "PROJECT_UNAVAILABLE",
        `cannot inspect SLP work in ${team.project_path}`,
        { cause: error instanceof Error ? error.message : String(error) },
      );
    }
    try {
      if (!tableExists(projectStore, "slp_work")) continue;
      const present = projectStore.database
        .query<{ present: number }, [string, string, number]>(
          `SELECT 1 AS present FROM slp_work
           WHERE id = ? AND team_id = ? AND generation = ?`,
        )
        .get(workId, team.team_id, team.generation);
      if (present) {
        matches.push({
          generation: team.generation,
          projectPath: team.project_path,
          teamId: team.team_id,
          workId,
        });
      }
    } finally {
      projectStore.close();
    }
  }
  return matches;
}

function oneHubWorkMatch(matches: HubWorkTarget[], workId: string): HubWorkTarget {
  // Newest generation first, so a team that reused an id across generations
  // answers with the latest; two different teams are still ambiguous and say so.
  const ordered = [...matches].sort((left, right) => right.generation - left.generation);
  const teams = new Set(ordered.map((match) => match.teamId));
  if (teams.size > 1) {
    throw new CliError(
      "AMBIGUOUS_WORK",
      `${workId} exists in multiple teams; qualify it as <team-id>:${workId}`,
      { candidates: ordered.map((match) => `${match.teamId}:${match.workId}`) },
    );
  }
  return ordered[0] as HubWorkTarget;
}

export function resolveHubWorkTarget(
  context: PluginContext,
  reference: string,
  access: HubWorkAccess = "write",
): HubWorkTarget {
  migrateRoom(context.store);
  const separator = reference.indexOf(":");
  const qualifiedTeam = separator > 0 ? reference.slice(0, separator) : null;
  const workId = separator > 0 ? reference.slice(separator + 1) : reference;
  if (!workId) throw new CliError("INVALID_WORK_ID", "--work requires a work id");
  const all = context.store.database
    .query<HubTeamRow, []>(
      `SELECT team_id, generation, project_path, state FROM slp_teams
       ORDER BY project_path, generation`,
    )
    .all()
    .filter((team) => qualifiedTeam === null || team.team_id === qualifiedTeam);
  const running = all.filter((team) => team.state === "RUNNING");
  const matches = hubWorkMatches(running, workId);
  if (matches.length > 0) return oneHubWorkMatch(matches, workId);
  const closed = all.filter((team) => team.state !== "RUNNING");
  const archived = closed.length > 0 ? hubWorkMatches(closed, workId, true) : [];
  if (archived.length === 0) {
    throw new CliError("NOT_FOUND", `SLP work not found from Hub: ${reference}`);
  }
  const found = oneHubWorkMatch(archived, workId);
  if (access === "read") return found;
  const state = all.find(
    (team) => team.team_id === found.teamId && team.generation === found.generation,
  )?.state ?? "STOPPED";
  throw new CliError(
    "NO_ACTIVE_TEAM",
    `${found.teamId}:g${found.generation} is ${state}; a stopped generation's work is readable` +
      ` (maestro status ${workId}) and no longer writable`,
    { generation: found.generation, state, team: found.teamId, work: workId },
  );
}

// room d117: the Hub Supervisor reviews the Lead's work from ~/maestro. It has
// no pane in the team, so it cannot go through requireSlpActor; instead the
// work id is resolved to its team, the PROJECT store is opened, and a reviewer
// actor named `hub-supervisor` is handed to the same accept and rework bodies
// the seats use.
interface HubReviewerBinding {
  actor: SlpReviewerActor;
  close: () => void;
  context: PluginContext;
  // The unqualified id: the Hub may address work as `<team-id>:<work-id>`.
  workId: string;
}

export function bindHubReviewer(
  context: PluginContext,
  reference: string,
  access: HubWorkAccess = "write",
): HubReviewerBinding {
  const target = resolveHubWorkTarget(context, reference, access);
  const projectStore = new Store(resolveStoreLocation(target.projectPath).path);
  try {
    // The Hub writes this store while the team's own seats are live in it;
    // without this a concurrent seat write fails the review outright rather
    // than waiting, which is the guard team start already sets for itself.
    projectStore.database.exec("PRAGMA busy_timeout = 300000");
    migrateProject(projectStore);
    // w736 F3: a read binds a stopped generation too; only the writes need it
    // live, and resolveHubWorkTarget has already refused those by name.
    const team = projectStore.database
      .query<ActiveLocalTeam, [string, number]>(
        `SELECT team_id, generation, room_store_path, project_path,
                configuration_json, pack_digest, workspace_id, runtime_pane_id,
                supervisor_pane_id
         FROM slp_local_teams
         WHERE team_id = ? AND generation = ?` +
          (access === "read" ? "" : " AND state = 'RUNNING'"),
      )
      .get(target.teamId, target.generation);
    if (!team) {
      throw new CliError(
        "NO_ACTIVE_TEAM",
        `${target.teamId}:g${target.generation} is no longer running`,
      );
    }
    return {
      actor: { name: hubReviewerName, role: hubReviewerName, team },
      close: () => projectStore.close(),
      context: { ...context, store: projectStore },
      workId: target.workId,
    };
  } catch (error) {
    projectStore.close();
    throw error;
  }
}

async function decideHubWork(
  context: PluginContext,
  input: {
    choice: string;
    replaces: string | null;
    scope: string;
    target: HubWorkTarget;
    why: string;
  },
): Promise<CliResult> {
  let transactionOpen = false;
  let id = "";
  const now = new Date().toISOString();
  try {
    context.store.database.exec("BEGIN IMMEDIATE");
    transactionOpen = true;
    const running = context.store.database
      .query<{ present: number }, [string, number]>(
        `SELECT 1 AS present FROM slp_teams
         WHERE team_id = ? AND generation = ? AND state = 'RUNNING'`,
      )
      .get(input.target.teamId, input.target.generation);
    if (!running) {
      throw new CliError(
        "NO_ACTIVE_TEAM",
        `${input.target.teamId}:g${input.target.generation} is no longer running`,
      );
    }
    if (input.replaces) {
      const prior = context.store.database
        .query<{ scope: string }, [string]>(
          "SELECT scope FROM slp_decisions WHERE id = ?",
        )
        .get(input.replaces);
      if (!prior) throw new CliError("NOT_FOUND", `decision to replace not found: ${input.replaces}`);
      if (prior.scope !== input.scope) {
        throw new CliError("INVALID_SCOPE", `${input.replaces} has scope ${prior.scope}, not ${input.scope}`);
      }
    }
    id = context.store.nextCountedId("d", ["decisions", "slp_decisions"]);
    const insertDecision = context.store.database.query(
      `INSERT INTO slp_decisions
        (id, team_id, generation, choice, why, scope, work_id, replaces_id,
         actor, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'hub-supervisor', ?)`,
    );
    const values = [
      id,
      input.target.teamId,
      input.target.generation,
      input.choice,
      input.why,
      input.scope,
      input.target.workId,
      input.replaces,
      now,
    ] as const;
    insertDecision.run(...values);
    if (input.replaces) clearProvisional(context.store, input.replaces);
    context.store.database
      .query(
        `INSERT INTO slp_activity
          (team_id, generation, actor, operation, target_type, target_id, created_at)
         VALUES (?, ?, 'hub-supervisor', 'decide', 'decision', ?, ?)`,
      )
      .run(input.target.teamId, input.target.generation, id, now);
    context.store.database.exec("COMMIT");
    transactionOpen = false;
  } catch (error) {
    if (transactionOpen) {
      try {
        context.store.database.exec("ROLLBACK");
      } catch {}
    }
    throw error;
  }
  // room d129 point 4: the Hub's ruling lands in the Hub store, but the item it
  // corrects lives in the project store, so the mark is written there. A store
  // that cannot be opened leaves the ruling committed and the mark unwritten
  // rather than failing a decision that is already recorded; the mark is a
  // reminder, never the ruling.
  if (input.replaces) {
    try {
      const projectStore = new Store(resolveStoreLocation(input.target.projectPath).path);
      try {
        markLessonCandidate(projectStore, input.target.workId, `decision ${id}`, now);
      } finally {
        projectStore.close();
      }
    } catch (error) {
      process.stderr.write(
        `lesson candidate not marked on ${input.target.workId}: ${
          error instanceof Error ? error.message : String(error)
        }\n`,
      );
    }
  }
  await pushHubDecision(input.target, input.choice);
  context.sessions.record("decide");
  const decision = {
    actor: "hub-supervisor",
    choice: input.choice,
    createdAt: now,
    id,
    replaces: input.replaces,
    scope: input.scope,
    why: input.why,
    workId: input.target.workId,
  };
  return {
    data: { decision },
    text: `${id} [${input.scope}] ${input.choice}`,
  };
}

// room d125: a decision that replaces a provisional ruling settles it; the
// superseded row keeps its text and loses only the flag. Runs inside the
// caller's transaction.
function clearProvisional(store: Store, id: string): void {
  if (!store.hasColumn("slp_decisions", "provisional")) return;
  store.database.query("UPDATE slp_decisions SET provisional = 0 WHERE id = ?").run(id);
}

export async function decide(context: PluginContext, invocation: CliInvocation): Promise<CliResult> {
  const choice = requiredPosition(invocation, 0, "choice");
  const why = stringOption(invocation, "why")?.trim();
  if (!why) throw new CliError("MISSING_ARGUMENT", "decide requires --why <reason>");
  const local = activeLocalTeam(context);
  let actor: string;
  let actorRole: SlpRole = "lead";
  let teamId: string;
  let generation: number;
  let defaultScope: "technical" | "owner";
  let allowedScopes: readonly string[];
  if (local) {
    const role = requireSlpActor(context, ["lead"]);
    requireRunningGeneration(context.store, role.team);
    actor = role.name;
    actorRole = role.role;
    teamId = role.team.team_id;
    generation = role.team.generation;
    defaultScope = "technical";
    allowedScopes = [defaultScope];
  } else if (isRoom(context.store.database)) {
    actor = "hub-supervisor";
    teamId = "hub";
    generation = 0;
    defaultScope = "owner";
    allowedScopes = ["owner", "cross-team"];
  } else {
    throw new CliError("NO_ACTIVE_TEAM", "decide requires a Hub or running team context");
  }
  const scope = stringOption(invocation, "scope") ?? defaultScope;
  if (!allowedScopes.includes(scope)) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      `${actor} cannot decide scope ${scope}; allowed: ${allowedScopes.join(", ")}`,
    );
  }
  const workId = stringOption(invocation, "work") ?? null;
  const replaces = stringOption(invocation, "replaces") ?? null;
  if (workId && !local) {
    return await decideHubWork(context, {
      choice,
      replaces,
      scope,
      target: resolveHubWorkTarget(context, workId),
      why,
    });
  }
  if (workId && local) {
    const present = context.store.database
      .query<{ present: number }, [string, string, number]>(
        "SELECT 1 AS present FROM slp_work WHERE id = ? AND team_id = ? AND generation = ?",
      )
      .get(workId, teamId, generation);
    if (!present) throw new CliError("NOT_FOUND", `SLP work not found: ${workId}`);
  }
  if (replaces) {
    if (!tableExists(context.store, "slp_decisions")) {
      throw new CliError("NOT_FOUND", `decision to replace not found: ${replaces}`);
    }
    const prior = context.store.database
      .query<{ scope: string }, [string, string, number]>(
        `SELECT scope FROM slp_decisions
         WHERE id = ? AND team_id = ? AND generation = ?`,
      )
      .get(replaces, teamId, generation);
    if (!prior) throw new CliError("NOT_FOUND", `decision to replace not found: ${replaces}`);
    if (prior.scope !== scope) {
      throw new CliError("INVALID_SCOPE", `${replaces} has scope ${prior.scope}, not ${scope}`);
    }
  }
  if (!local) migrateRoom(context.store);
  let id = "";
  const now = new Date().toISOString();
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    if (local) requireRunningGeneration(context.store, local);
    id = context.store.nextCountedId("d", ["decisions", "slp_decisions"]);
    context.store.database
      .query(
        `INSERT INTO slp_decisions
          (id, team_id, generation, choice, why, scope, work_id, replaces_id,
           actor, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(id, teamId, generation, choice, why, scope, workId, replaces, actor, now);
    if (replaces && workId && local) {
      markLessonCandidate(context.store, workId, `decision ${id}`, now);
    }
    if (replaces && !local) clearProvisional(context.store, replaces);
    context.store.database
      .query(
        `INSERT INTO slp_activity
          (team_id, generation, actor, operation, target_type, target_id, created_at)
         VALUES (?, ?, ?, 'decide', 'decision', ?, ?)`,
      )
      .run(teamId, generation, actor, id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  if (local && workId) {
    await pushDecision(context.store, local.project_path, actorRole, actor, workId, choice);
  }
  context.sessions.record("decide");
  const decision = { actor, choice, createdAt: now, id, replaces, scope, why, workId };
  return { data: { decision }, text: `${id} [${scope}] ${choice}` };
}

export const decisionIdPattern = /^d\d+$/;

interface SlpDecisionRow {
  actor: string;
  choice: string;
  created_at: string;
  generation: number;
  id: string;
  provisional: number;
  replaces_id: string | null;
  scope: string;
  team_id: string;
  why: string;
  work_id: string | null;
}

interface SlpDecisionData {
  actor: string;
  choice: string;
  createdAt: string;
  id: string;
  provisional: boolean;
  replaces: string | null;
  scope: string;
  store: "hub" | "project";
  why: string;
  workId: string | null;
}

// A seat reads its own store's rows unfiltered; from the Hub store it reads
// its own team's rows plus the owner and cross-team rulings, which carry no
// work id and belong to no team (teamId null asks for the whole store).
export function decisionRowsById(store: Store, id: string, teamId: string | null): SlpDecisionRow[] {
  if (!tableExists(store, "slp_decisions")) return [];
  const columns =
    `SELECT id, team_id, generation, choice, why, scope, work_id, replaces_id, actor, created_at,
            ${provisionalColumn(store)}
     FROM slp_decisions WHERE id = ?`;
  return teamId === null
    ? store.database.query<SlpDecisionRow, [string]>(columns).all(id)
    : store.database
      .query<SlpDecisionRow, [string, string]>(
        `${columns} AND (team_id = ? OR scope IN ('owner', 'cross-team'))`,
      )
      .all(id, teamId);
}

export function decisionData(row: SlpDecisionRow, store: "hub" | "project"): SlpDecisionData {
  return {
    actor: row.actor,
    choice: row.choice,
    createdAt: row.created_at,
    id: row.id,
    provisional: row.provisional === 1,
    replaces: row.replaces_id,
    scope: row.scope,
    store,
    why: row.why,
    workId: row.work_id,
  };
}

// w736 F4: a store can hold two decision spaces at once - the project decision
// ledger (`decisions`, with the draft/lock/withdraw lifecycle) and the SLP
// rulings (`slp_decisions`) - and their ids are separate sequences that already
// overlap. `maestro status dNNN` reads the second; `maestro decision show dNNN`
// read only the first and answered NOT_FOUND for every ruling that had been
// recorded. It now reads both and labels each, which is w697's shape: a store
// never silently wins an id it shares.
//
// l4 (w719, w720): F4 asked this of the Hub room only, so inside a project
// checkout every SLP decision a team records stayed NOT_FOUND to the same verb
// that prints it one directory up - and a reader who gets NOT_FOUND from one
// store and a row from another learns the opposite of where the row lives. The
// space is read from whichever store the command runs in, and the label names
// that store rather than the seat that authored the row, which is the label
// l4 found pointing the wrong way.
export function slpDecisionResult(
  context: PluginContext,
  id: string,
): { decisions: unknown[]; text: string } | null {
  const store = isRoom(context.store.database) ? "hub" : "project";
  const decisions = decisionRowsById(context.store, id, null).map((row) => decisionData(row, store));
  if (decisions.length === 0) return null;
  return { decisions, text: decisions.map(decisionText).join("\n\n") };
}

function decisionText(decision: SlpDecisionData): string {
  return [
    `${decision.id} [${decision.scope}] by ${decision.actor} (${decision.store} store)`,
    `work: ${decision.workId ?? "none"}`,
    ...(decision.provisional ? ["provisional: yes"] : []),
    ...(decision.replaces ? [`replaces: ${decision.replaces}`] : []),
    `choice: ${decision.choice}`,
    `why: ${decision.why}`,
  ].join("\n");
}

// w697: decision ids are per-store sequences, so the same id can in principle
// name a row in both stores. Every match is returned carrying the store it came
// from rather than one store silently winning; the two stores hold no
// overlapping id today (d857), so the plural case is a shape, not a claim.
export function decisionResult(decisions: readonly SlpDecisionData[], id: string): CliResult {
  if (decisions.length === 0) throw new CliError("NOT_FOUND", `SLP decision not found: ${id}`);
  return {
    data: { decisions },
    text: decisions.map(decisionText).join("\n\n"),
  };
}

export function readSlpDecision(context: PluginContext, actor: SlpActor, id: string): CliResult {
  const local = decisionRowsById(context.store, id, null)
    .map((row) => decisionData(row, "project"));
  const roomStore = new Store(actor.team.room_store_path, { readonly: true });
  let hub: SlpDecisionData[] = [];
  try {
    hub = decisionRowsById(roomStore, id, actor.team.team_id)
      .map((row) => decisionData(row, "hub"));
  } finally {
    roomStore.close();
  }
  return decisionResult([...local, ...hub], id);
}
