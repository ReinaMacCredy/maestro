import { existsSync } from "node:fs";
import { CliError, type CliInvocation, type CliResult } from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { Store, resolveStoreLocation, tableExists } from "../../kernel/store.ts";
import { isRoom } from "../room.ts";
import { HerdrSlpRuntime } from "./runtime.ts";
import { compactLine, compactsForPane } from "../session-telemetry.ts";
import {
  clipLine,
  hubReviewerName,
  requireActiveOrLegacy,
  requireSlpActor,
  requireSlpWork,
  roleRows,
  type SlpActor,
  type SlpReviewerActor,
  type SlpWorkRow,
  workData,
} from "./context.ts";
import {
  bindHubReviewer,
  blockedForHub,
  decisionData,
  decisionIdPattern,
  decisionResult,
  decisionRowsById,
  provisionalDecisions,
  provisionalLines,
  readSlpDecision,
} from "./decide.ts";
import { runtimePaneIdOf } from "./lifecycle.ts";
import { planForTeam } from "./pack.ts";
import type { WorkState } from "./store.ts";

function stopSuffix(stop: { emergency: boolean; reason: string } | null): string {
  if (!stop || stop.emergency || stop.reason === "") return "";
  return ` (supervisor): ${stop.reason}`;
}

// d870/d873: the verdict reads as a question, not a finding, because from a
// read a downed Herdr looks the same as a dead team - and it names the one
// command that closes a real orphan by hand, which works today on a pane-less
// generation. Nothing here stops anything.
function orphanedSuffix(team: Record<string, unknown>): string {
  if (team.orphaned !== true) return "";
  return `; ORPHANED? no lead pane - a Herdr that is down reads the same from here;` +
    ` if it is real, close it with: maestro team stop ${String(team.teamId)} --emergency`;
}

function runtimePaneIdInProject(projectPath: string, teamId: string, generation: number): string {
  const location = resolveStoreLocation(projectPath);
  if (!existsSync(location.path)) return "";
  const store = new Store(location.path, { readonly: true });
  try {
    if (!tableExists(store, "slp_local_teams") || !store.hasColumn("slp_local_teams", "runtime_pane_id")) return "";
    return runtimePaneIdOf(store, teamId, generation);
  } finally {
    store.close();
  }
}

type SlpStatusRole = ReturnType<typeof roleRows>[number];

interface SlpNextStep {
  mayRun: string[];
  waitingOn: string | null;
}

// l9 (w739, d896): the text read printed the first line of a 4,000-word brief
// and said nothing about the rest, so a seat read its item, believed it had read
// the request, and was wrong. A clipped objective now names its own size and the
// command that prints it whole. An objective that fits on one line is unchanged.
function objectiveLines(id: string, objective: string, limit: number): string[] {
  const head = clipLine(objective, limit);
  const full = objective.trim();
  if (full === head) return [`objective: ${head}`];
  const lines = full.split("\n").length;
  return [
    `objective: ${head}`,
    `objective truncated here: ${full.length} characters over ${lines} line${lines === 1 ? "" : "s"};` +
      ` read it in full with: maestro status ${id} --json`,
  ];
}

function lessonCandidateSources(store: Store, workId: string): string[] {
  if (!tableExists(store, "slp_lesson_candidates")) return [];
  return store.database
    .query<{ source: string }, [string]>(
      "SELECT source FROM slp_lesson_candidates WHERE work_id = ? ORDER BY recorded_at, source",
    )
    .all(workId)
    .map((row) => row.source);
}

function reworkGrantOpen(context: PluginContext, work: SlpWorkRow): boolean {
  if (work.state !== "RETURNED" || !tableExists(context.store, "slp_rework_grants")) return false;
  return context.store.database
    .query<{ present: number }, [string, number]>(
      `SELECT 1 AS present FROM slp_rework_grants
       WHERE work_id = ? AND return_revision = ? AND consumed_at IS NULL`,
    )
    .get(work.id, work.return_revision) !== null;
}

// d758: what the caller may run on one item, or whom it waits on.
function nextStep(
  actor: SlpReviewerActor,
  roles: SlpStatusRole[],
  work: SlpWorkRow,
  grantOpen: boolean,
): SlpNextStep {
  const assigneeRole = roles.find((role) => role.name === work.assigned_to)?.role ?? "peer";
  // room d117: the Lead's reviewer is the Hub, which holds no seat here - so
  // no seat is ever "reviewing" such an item, the waiting-on line names the
  // Hub, and the Hub itself reads the accept and rework lines when it is the
  // one asking.
  const hubReviews = assigneeRole === "lead";
  const reviewerName = hubReviews
    ? hubReviewerName
    : roles.find((role) => role.role === "lead")?.name ?? "lead";
  const reviewing = hubReviews
    ? actor.role === hubReviewerName
    : actor.role === "lead" && actor.name !== work.assigned_to;
  const mine = work.assigned_to === actor.name;
  switch (work.state) {
    case "OPEN":
      return mine
        ? { mayRun: [`work take ${work.id}`], waitingOn: null }
        : {
          mayRun: reviewing ? [`work accept ${work.id} --outcome cancelled`] : [],
          waitingOn: work.assigned_to,
        };
    case "ACTIVE":
      return work.owner === actor.name
        ? { mayRun: [`work return ${work.id} "<result>"`], waitingOn: null }
        : { mayRun: [], waitingOn: work.owner ?? work.assigned_to };
    case "RETURNED":
      if (grantOpen) {
        return mine
          ? { mayRun: [`work take ${work.id}`], waitingOn: null }
          : { mayRun: reviewing ? [`work accept ${work.id}`] : [], waitingOn: work.assigned_to };
      }
      return reviewing
        ? {
          mayRun: [
            `work accept ${work.id}`,
            `work note ${work.id} "<gap>" --rework`,
            `work accept ${work.id} --outcome cancelled`,
          ],
          waitingOn: null,
        }
        : { mayRun: [], waitingOn: reviewerName };
    default:
      return { mayRun: [], waitingOn: null };
  }
}

function nextLine(work: SlpWorkRow, step: SlpNextStep, lessonCandidate = false): string {
  if (work.state === "DONE") return `next: none (${work.acceptance_outcome ?? "done"})`;
  const optional = step.mayRun.length > 0 ? `; may run: ${step.mayRun.join(" | ")}` : "";
  const base = step.waitingOn === null
    ? `next: ${step.mayRun.join(" | ")}`
    : `next: waiting on ${step.waitingOn}${optional}`;
  if (!lessonCandidate) return base;
  // room d129 point 4: the reminder is addressed to whoever the next step is
  // addressed to, so the holder reads what its return must say and the
  // reviewer reads what it may accept on.
  const may = (verb: string) => step.mayRun.some((command) => command.startsWith(`${verb} ${work.id}`));
  const reminder = may("work accept")
    ? "accept only after the return states the lesson it filed or why none applies"
    : may("work return") || may("work take")
    ? "this item carries a lesson candidate: state in the return the lesson you filed (maestro lesson file) or why none applies"
    : "this item carries a lesson candidate";
  return `${base}; ${reminder}`;
}

// w707: DONE is the one state that means two opposite things - delivered, or
// cancelled without ever being worked - and the store already knows which, in
// acceptance_outcome. Print it on every DONE line and in the collapsed count
// rather than marking only the exception: an unmarked line would leave the
// reader to supply "accepted", which is exactly the join w706 stopped asking
// readers to make. No state is added; DONE is still DONE.
function doneOutcome(work: SlpWorkRow): string {
  return work.acceptance_outcome ?? "outcome unrecorded";
}

function doneSummary(done: SlpWorkRow[]): string {
  const tally = new Map<string, number>();
  for (const work of done) {
    const outcome = doneOutcome(work);
    tally.set(outcome, (tally.get(outcome) ?? 0) + 1);
  }
  const known = ["accepted", "cancelled"];
  const rank = (outcome: string) => {
    const at = known.indexOf(outcome);
    return at < 0 ? known.length : at;
  };
  const outcomes = [...tally.keys()].sort((left, right) =>
    rank(left) - rank(right) || left.localeCompare(right)
  );
  const first = outcomes[0];
  const summary = outcomes.length === 1 && first !== undefined
    ? first
    : outcomes.map((outcome) => `${tally.get(outcome)} ${outcome}`).join(", ");
  return `${done.length} DONE (${summary}); --all to list`;
}

function workLine(work: SlpWorkRow, step: SlpNextStep): string {
  const marker = work.state !== "DONE" && step.waitingOn === null ? "*" : " ";
  const outcome = work.state === "DONE" ? ` (${doneOutcome(work)})` : "";
  return `${marker} ${work.id} ${work.state}${outcome} ${work.created_by} -> ${work.assigned_to}: ${
    clipLine(work.objective, 72)
  }`;
}

function decisionLine(refs: Array<{ id: string; workId: string | null }>): string {
  if (refs.length === 0) return "decisions: none";
  return `decisions: ${
    refs.map((ref) => ref.workId ? `${ref.id} (${ref.workId})` : ref.id).join(", ")
  }`;
}

function teamDecisionRefs(
  context: PluginContext,
  actor: SlpActor,
): Array<{ id: string; workId: string | null }> {
  const query = (store: Store) =>
    tableExists(store, "slp_decisions")
      ? store.database
        .query<{ created_at: string; id: string; work_id: string | null }, [string, number]>(
          `SELECT id, work_id, created_at FROM slp_decisions
           WHERE team_id = ? AND generation = ? ORDER BY created_at, id`,
        )
        .all(actor.team.team_id, actor.team.generation)
      : [];
  const roomStore = new Store(actor.team.room_store_path, { readonly: true });
  let rows = query(context.store);
  try {
    rows = [...rows, ...query(roomStore)];
  } finally {
    roomStore.close();
  }
  return [...new Map(rows.map((row) => [row.id, row])).values()]
    .sort((left, right) =>
      left.created_at.localeCompare(right.created_at) || left.id.localeCompare(right.id)
    )
    .map((row) => ({ id: row.id, workId: row.work_id }));
}

// room d117: one work-detail read, shared by the seats and by the Hub Supervisor
// reviewing the Lead's work. Extracted verbatim from maybeHandleSlpStatus so
// the Hub sees exactly what a seat sees - same entries, same merged
// decisions, same next: line - rather than a second, drifting renderer.
async function readWorkDetail(
  context: PluginContext,
  actor: SlpReviewerActor,
  roles: SlpStatusRole[],
  workId: string,
): Promise<CliResult> {
  const work = requireSlpWork(context, actor, workId);
  if (actor.role === "peer" && work.assigned_to !== actor.name) {
    throw new CliError("ROLE_FORBIDDEN", `${actor.name} cannot inspect ${workId}`);
  }
  if (
    actor.role === "lead" &&
    work.assigned_to !== actor.name &&
    work.created_by !== actor.name
  ) {
    throw new CliError("ROLE_FORBIDDEN", `${actor.name} cannot inspect ${workId}`);
  }
  const entries = context.store.database
    .query<{
      actor: string;
      body: string;
      created_at: string;
      flag: string | null;
      kind: "NOTE" | "RETURN" | "ACCEPTANCE";
    }, [string]>(
      `SELECT kind, actor, body, flag, created_at FROM slp_work_entries
       WHERE work_id = ? ORDER BY id`,
    )
    .all(workId);
  const localDecisions = context.store.database
    .query<{
      actor: string;
      choice: string;
      created_at: string;
      id: string;
      replaces_id: string | null;
      scope: string;
      why: string;
    }, [string]>(
      `SELECT id, choice, why, scope, replaces_id, actor, created_at
       FROM slp_decisions WHERE work_id = ? ORDER BY created_at, id`,
    )
    .all(workId)
    .map((decision) => ({
      actor: decision.actor,
      choice: decision.choice,
      createdAt: decision.created_at,
      id: decision.id,
      replaces: decision.replaces_id,
      scope: decision.scope,
      why: decision.why,
    }));
  const roomStore = new Store(actor.team.room_store_path, { readonly: true });
  let hubDecisions: typeof localDecisions = [];
  try {
    if (tableExists(roomStore, "slp_decisions")) {
      hubDecisions = roomStore.database
        .query<{
          actor: string;
          choice: string;
          created_at: string;
          id: string;
          replaces_id: string | null;
          scope: string;
          why: string;
        }, [string, number, string]>(
          `SELECT id, choice, why, scope, replaces_id, actor, created_at
           FROM slp_decisions
           WHERE team_id = ? AND generation = ? AND work_id = ?
           ORDER BY created_at, id`,
        )
        .all(actor.team.team_id, actor.team.generation, workId)
        .map((decision) => ({
          actor: decision.actor,
          choice: decision.choice,
          createdAt: decision.created_at,
          id: decision.id,
          replaces: decision.replaces_id,
          scope: decision.scope,
          why: decision.why,
        }));
    }
  } finally {
    roomStore.close();
  }
  const decisions = [...new Map(
    [...localDecisions, ...hubDecisions].map((decision) => [decision.id, decision]),
  ).values()].sort((left, right) =>
    left.createdAt.localeCompare(right.createdAt) || left.id.localeCompare(right.id)
  );
  const latest = entries.at(-1) ?? null;
  const candidateSources = lessonCandidateSources(context.store, workId);
  // room d137: the assignee's compact count, read beside its seat and nowhere
  // else. It is evidence for the reviewer's judgment, never a gate: no branch
  // below it refuses, nudges or stops anything on the figure.
  const compacts = compactLine(
    compactsForPane(
      context.store,
      roles.find((role) => role.name === work.assigned_to)?.pane_id ?? null,
    ),
  );
  return {
    data: {
      acceptance: entries.findLast((entry) => entry.kind === "ACCEPTANCE") ?? null,
      decisions,
      lessonCandidate: candidateSources.length > 0 ? { sources: candidateSources } : null,
      notes: entries
        .filter((entry) => entry.kind === "NOTE")
        .map((entry) => ({
          actor: entry.actor,
          body: entry.body,
          createdAt: entry.created_at,
          flag: entry.flag,
        })),
      returns: entries
        .filter((entry) => entry.kind === "RETURN")
        .map((entry) => ({ actor: entry.actor, body: entry.body, createdAt: entry.created_at })),
      work: workData(work),
    },
    text: [
      `${work.id} ${work.state} ${work.created_by} -> ${work.assigned_to}${compacts ? ` ${compacts}` : ""}`,
      `revision: ${work.return_revision}`,
      ...objectiveLines(work.id, work.objective, 160),
      latest
        ? `${latest.kind.toLowerCase()}${latest.flag ? ` [${latest.flag}]` : ""} by ${latest.actor}: ${
          clipLine(latest.body, 160)
        }`
        : "entries: none",
      decisionLine(decisions.map((decision) => ({ id: decision.id, workId: null }))),
      ...(candidateSources.length > 0
        ? [`lesson candidate: yes, from ${candidateSources.join(", ")}`]
        : []),
      nextLine(
        work,
        nextStep(actor, roles, work, reworkGrantOpen(context, work)),
        candidateSources.length > 0,
      ),
    ].join("\n"),
  };
}

export async function maybeHandleSlpStatus(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  const requestedWork = invocation.positionals[0] ?? null;
  if (isRoom(context.store.database)) {
    if (requestedWork) {
      // w697: the Hub reads its own rulings by id; work still belongs to the seats.
      if (decisionIdPattern.test(requestedWork)) {
        return decisionResult(
          decisionRowsById(context.store, requestedWork, null)
            .map((row) => decisionData(row, "hub")),
          requestedWork,
        );
      }
      // room d117: the Hub reads the work it reviews.
      const bound = bindHubReviewer(context, requestedWork, "read");
      try {
        const roles = roleRows(
          bound.context.store,
          bound.actor.team.team_id,
          bound.actor.team.generation,
          true,
        );
        return await readWorkDetail(bound.context, bound.actor, roles, bound.workId);
      } finally {
        bound.close();
      }
    }
    const rows = tableExists(context.store, "slp_teams")
      ? context.store.database
      .query<{
        configuration_json: string;
        generation: number;
        pack_digest: string;
        pack_version: string;
        project_path: string;
        state: "RUNNING" | "STOPPED";
        team_id: string;
        workspace_id: string;
      }, []>(
        `SELECT team_id, generation, project_path, configuration_json, pack_version,
                pack_digest, state, workspace_id
         FROM slp_teams
         ORDER BY project_path, generation`,
      )
      .all()
      : [];
    const runtime = new HerdrSlpRuntime();
    const teams = [] as Array<Record<string, unknown>>;
    for (const row of rows) {
      const roles = roleRows(context.store, row.team_id, row.generation, false);
      let missingPanes: string[] = [];
      let runtimeState: "available" | "unavailable" | "not-running" = "not-running";
      let runtimePane = false;
      const plan = await planForTeam(row);
      const runtimePaneId = runtimePaneIdInProject(row.project_path, row.team_id, row.generation);
      try {
        const inspection = await runtime.inspect(
          plan,
          roles.map((role) => ({
            briefDigest: role.brief_digest,
            instanceId: role.instance_id,
            name: role.name,
            packDigest: role.pack_digest,
            paneId: role.pane_id,
            profile: role.profile,
            readyChallenge: role.ready_challenge,
            role: role.role,
            workspaceId: role.workspace_id,
          })),
          runtimePaneId,
        );
        if (row.state === "RUNNING" || inspection.workspace) {
          missingPanes = inspection.missingPanes;
          runtimeState = inspection.runtime;
          runtimePane = inspection.runtimePane;
        }
      } catch {
        runtimeState = "unavailable";
      }
      const counts: Record<WorkState, number> = { ACTIVE: 0, DONE: 0, OPEN: 0, RETURNED: 0 };
      let abandonedWorkCount = 0;
      let hubBlocked: Array<{ body: string; workId: string }> = [];
      const projectStore = new Store(resolveStoreLocation(row.project_path).path, { readonly: true });
      try {
        if (row.state === "RUNNING") hubBlocked = blockedForHub(context.store, projectStore, row, roles);
        if (tableExists(projectStore, "slp_work")) {
          for (const count of projectStore.database
            .query<{ count: number; state: WorkState }, [string, number]>(
              `SELECT state, COUNT(*) AS count FROM slp_work
               WHERE team_id = ? AND generation = ? GROUP BY state`,
            )
            .all(row.team_id, row.generation)) {
            counts[count.state] = count.count;
          }
          const hasAbandonment = projectStore.database
            .query<{ present: number }, []>(
              `SELECT 1 AS present FROM pragma_table_info('slp_work')
               WHERE name = 'abandoned_at'`,
            )
            .get();
          if (hasAbandonment) {
            abandonedWorkCount = projectStore.database
              .query<{ count: number }, [string, number]>(
                `SELECT COUNT(*) AS count FROM slp_work
                 WHERE team_id = ? AND generation = ? AND abandoned_at IS NOT NULL`,
              )
              .get(row.team_id, row.generation)?.count ?? 0;
          }
        }
      } finally {
        projectStore.close();
      }
      // d870's third clause (d873): a REPORT-ONLY orphaned verdict. A RUNNING
      // generation whose Lead is missing has no seat left to report on it, so
      // the Hub is told, and told the one command that closes it by hand. It
      // is deliberately not certainty: this branch reads through
      // HerdrSlpRuntime.inspect, whose zero-workspace path reports EVERY role
      // missing, so a Herdr that is down or has not restored its workspace is
      // indistinguishable from a real orphan here. That is exactly why it is
      // wired to no commit. A status read mutates nothing. The non-empty
      // guard keeps a generation with no rows at all from reading as orphaned
      // rather than as unknown.
      const leads = roles.filter((role) => role.role === "lead");
      const orphaned = row.state === "RUNNING" &&
        leads.length > 0 &&
        leads.every((role) => missingPanes.includes(role.name));
      const stopRecord = tableExists(context.store, "slp_lifecycle_operations")
        ? context.store.database
          .query<{ actor: string; emergency: number; reason: string }, [string, number]>(
            `SELECT actor, emergency, reason FROM slp_lifecycle_operations
             WHERE team_id = ? AND generation = ? AND operation = 'STOP' AND phase = 'COMMITTED'`,
          )
          .get(row.team_id, row.generation) ?? null
        : null;
      teams.push({
        abandonedWorkCount,
        generation: row.generation,
        hubBlocked,
        missingPanes,
        orphaned,
        packDigest: row.pack_digest,
        packVersion: row.pack_version,
        projectPath: row.project_path,
        roles: roles.map((role) => ({
          briefDigest: role.brief_digest,
          instanceId: role.instance_id,
          name: role.name,
          packDigest: role.pack_digest,
          paneId: role.pane_id,
          profile: role.profile,
          readyChallenge: role.ready_challenge,
          role: role.role,
        })),
        runtime: runtimeState,
        state: row.state,
        stop: stopRecord
          ? { actor: stopRecord.actor, emergency: stopRecord.emergency === 1, reason: stopRecord.reason }
          : null,
        runtimePane: runtimePane ? "on" : "off",
        teamId: row.team_id,
        workCounts: counts,
        workspaceId: row.workspace_id,
      });
    }
    const waiting = provisionalDecisions(context.store);
    return {
      data: { provisionalDecisions: waiting, teams },
      text: [
        ...(teams.length === 0
          ? ["no SLP teams"]
          : teams.flatMap((team) => [
            `${team.teamId} g${team.generation} ${team.state}${stopSuffix(team.stop as { emergency: boolean; reason: string } | null)}; runtime pane ${team.runtimePane}; missing ${(team.missingPanes as string[]).join(", ") || "none"}${orphanedSuffix(team)}`,
            ...(team.hubBlocked as Array<{ body: string; workId: string }>).map((blocked) =>
              `  blocked for the Hub: ${team.teamId}:${blocked.workId} ${clipLine(blocked.body, 160)}; answer: maestro decide "<choice>" --why "<reason>" --work ${team.teamId}:${blocked.workId}`
            ),
          ])),
        ...provisionalLines(waiting),
      ].join("\n"),
    };
  }

  if (!requireActiveOrLegacy(context)) return null;
  const actor = requireSlpActor(context, ["lead", "peer"]);
  const roles = roleRows(context.store, actor.team.team_id, actor.team.generation, true);
  if (requestedWork && decisionIdPattern.test(requestedWork)) {
    return readSlpDecision(context, actor, requestedWork);
  }
  if (requestedWork) {
    return readWorkDetail(context, actor, roles, requestedWork);
  }

  const allWork = context.store.database
    .query<SlpWorkRow, [string, number]>(
      `SELECT * FROM slp_work WHERE team_id = ? AND generation = ? ORDER BY created_at, id`,
    )
    .all(actor.team.team_id, actor.team.generation);
  const scoped = allWork.filter((work) => {
    if (actor.role === "peer") return work.assigned_to === actor.name && work.state !== "DONE";
    return (
      work.assigned_to === actor.name || work.created_by === actor.name
    ) && work.state !== "DONE";
  });
  let missingPanes: string[] = [];
  let runtimeState: "available" | "unavailable" = "available";
  let runtimePane = false;
  const plan = await planForTeam(actor.team);
  try {
    const inspection = await new HerdrSlpRuntime().inspect(
      plan,
      roles.map((role) => ({
        briefDigest: role.brief_digest,
        instanceId: role.instance_id,
        name: role.name,
        packDigest: role.pack_digest,
        paneId: role.pane_id,
        profile: role.profile,
        readyChallenge: role.ready_challenge,
        role: role.role,
        workspaceId: role.workspace_id,
      })),
      actor.team.runtime_pane_id,
    );
    missingPanes = inspection.missingPanes;
    runtimePane = inspection.runtimePane;
  } catch {
    runtimeState = "unavailable";
  }
  return {
    data: {
      generation: actor.team.generation,
      missingPanes,
      role: { name: actor.name, role: actor.role },
      roles: roles.map((role) => ({
        briefDigest: role.brief_digest,
        instanceId: role.instance_id,
        name: role.name,
        packDigest: role.pack_digest,
        paneId: role.pane_id,
        profile: role.profile,
        readyChallenge: role.ready_challenge,
        role: role.role,
      })),
      runtime: runtimeState,
      runtimePane: runtimePane ? "on" : "off",
      teamId: actor.team.team_id,
      work: scoped.map(workData),
    },
    text: teamStatusText(context, invocation, actor, roles, allWork, missingPanes),
  };
}

// d758: text lists the team's items; JSON keeps the relevance-scoped list.
function teamStatusText(
  context: PluginContext,
  invocation: CliInvocation,
  actor: SlpActor,
  roles: SlpStatusRole[],
  allWork: SlpWorkRow[],
  missingPanes: string[],
): string {
  const order: Record<WorkState, number> = { ACTIVE: 1, DONE: 3, OPEN: 0, RETURNED: 2 };
  const visible = actor.role === "peer"
    ? allWork.filter((work) => work.assigned_to === actor.name)
    : allWork;
  const pending = visible
    .filter((work) => work.state !== "DONE")
    .sort((left, right) => order[left.state] - order[right.state]);
  const done = visible.filter((work) => work.state === "DONE");
  const line = (work: SlpWorkRow) =>
    workLine(work, nextStep(actor, roles, work, reworkGrantOpen(context, work)));
  const pane = roles.find((role) => role.name === actor.name)?.pane_id ?? "";
  const missing = missingPanes.length > 0 ? `; missing ${missingPanes.join(", ")}` : "";
  return [
    `${actor.team.team_id} g${actor.team.generation} ${actor.role} ${actor.name} in ${pane}${missing}`,
    ...pending.map(line),
    ...(invocation.options.all === true
      ? done.map(line)
      : done.length > 0
      ? [doneSummary(done)]
      : []),
    decisionLine(teamDecisionRefs(context, actor)),
  ].join("\n");
}
