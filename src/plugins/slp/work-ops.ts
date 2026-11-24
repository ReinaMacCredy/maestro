import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { CliError, requiredPosition, stringOption, type CliInvocation, type CliResult } from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { Store, tableExists } from "../../kernel/store.ts";
import { resolveHomeDirectory } from "../home.ts";
import { isRoom } from "../room.ts";
import { resolveSeat } from "./seats.ts";
import { HerdrSlpRuntime } from "./runtime.ts";
import type { SlpRole, SlpRoleContract, SlpRolePlan, SlpTeamPlan } from "./runtime-plan.ts";
import {
  expectedReviewer,
  hubReviewerName,
  hubSupervisorTarget,
  markLessonCandidate,
  noticeSummary,
  pushNotice,
  readSlpWork,
  recordProjectActivity,
  requireActiveOrLegacy,
  requireRunningGeneration,
  requireSlpActor,
  requireSlpWork,
  requireWorkReviewer,
  requireWorkTransition,
  reviewerPushTarget,
  rolePaneName,
  type SlpActor,
  type SlpReviewerActor,
  type SlpWorkRow,
  workData,
} from "./context.ts";
import { bindHubReviewer, resolveHubWorkTarget } from "./decide.ts";
import { migrateProject, migrateRoom } from "./migrations.ts";
import { packConfiguration, planForTeam, requireSeatsUnchanged, roleContracts } from "./pack.ts";

function normalizedPeerName(teamId: string, raw: string, existing: readonly string[]): string {
  if (existing.includes(raw)) return raw;
  const candidate = raw.startsWith("peer-") ? raw.slice("peer-".length) : raw;
  const normalized = candidate
    .toLowerCase()
    .replaceAll(/[^a-z0-9-]+/g, "-")
    .replaceAll(/^-|-$/g, "");
  if (!/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(normalized)) {
    throw new CliError("INVALID_PEER", "--to must name a Peer using letters, numbers, or hyphens");
  }
  const readable = normalized.slice(0, 20).replace(/-$/, "");
  const identity = createHash("sha256")
    .update(`${teamId}\0${normalized}`)
    .digest("hex")
    .slice(0, 6);
  return `peer-${readable}-${identity}`;
}

// w701 (d859): --acceptance and --blocked-by are declared in the shared work add
// flag table, so the parser accepts them inside a team and the SLP branch, which
// reads only --to, --model and --fresh, then drops them without a word. Refuse
// instead of persisting: SLP has no acceptance column and no gating concept, so a
// stored blocker nothing enforces would read as a gate that is not one. Both flags
// keep working unchanged on the Hub path below this branch.
function refuseDiscardedWorkAddOptions(invocation: CliInvocation): void {
  if (invocation.options.start !== undefined) {
    throw new CliError("INVALID_OPTION", "--start does not apply to SLP work add; the assignee takes the item with maestro work take <id>");
  }
  const discarded = ["acceptance", "blocked-by"].filter(
    (flag) => invocation.options[flag] !== undefined,
  );
  if (discarded.length === 0) return;
  throw new CliError(
    "INVALID_OPTION",
    `${discarded.map((flag) => `--${flag}`).join(" and ")} ` +
      `${discarded.length > 1 ? "are" : "is"} not recorded by SLP work add; ` +
      "write the acceptance condition and any ordering into the objective text",
  );
}

export async function maybeHandleSlpWorkAdd(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  if (isRoom(context.store.database) && invocation.options.to !== undefined) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      "Hub Supervisor cannot assign team work directly; communicate through the Lead",
    );
  }
  if (!requireActiveOrLegacy(context)) return null;
  const actor = requireSlpActor(context, ["lead"]);
  requireRunningGeneration(context.store, actor.team);
  refuseDiscardedWorkAddOptions(invocation);
  const objective = requiredPosition(invocation, 0, "work objective");
  const requestedTarget = stringOption(invocation, "to");
  // room d128: --model launches this one Peer with that model, over the
  // generation's --peer-model.
  const modelOption = stringOption(invocation, "model");
  if (modelOption !== undefined && modelOption.trim() === "") {
    throw new CliError("INVALID_OPTION", "--model needs a model name; the harness validates it, maestro passes it through");
  }
  const fresh = invocation.options.fresh === true;
  const roles = context.store.database
    .query<{
      brief_digest: string;
      instance_id: string;
      name: string;
      pack_digest: string;
      pane_id: string;
      profile: string;
      ready_challenge: string;
      role: SlpRole;
      workspace_id: string;
    }, [string, number]>(
      `SELECT name, pane_id, role, workspace_id, instance_id, pack_digest,
              brief_digest, ready_challenge, profile FROM slp_local_roles
       WHERE team_id = ? AND generation = ?`,
    )
    .all(actor.team.team_id, actor.team.generation);
  let assignee = roles.find((role) => role.role === "lead") ?? null;
  let createdPeerTab: string | null = null;
  let startedPeerPane: string | null = null;
  let plan: SlpTeamPlan | null = null;
  const runtime = new HerdrSlpRuntime();
  if (!requestedTarget) {
    // room d118 (w725): a Lead opens its own item with no --to. The row is the
    // one team start writes for Lead work - the Lead is its own assignee - so
    // no Peer pane is opened, no wake line goes out, and its reviewer is the
    // seat above exactly as for Lead work today.
    if (!assignee) throw new CliError("RUNTIME_INCOMPLETE", "the team has no Lead");
    if (modelOption !== undefined) {
      throw new CliError("INVALID_OPTION", "--model applies to Lead work add --to <peer>");
    }
    if (fresh) throw new CliError("INVALID_OPTION", "--fresh applies to Lead work add --to <peer>");
  } else {
    const peerName = normalizedPeerName(
      actor.team.team_id,
      requestedTarget,
      roles.filter((role) => role.role === "peer").map((role) => role.name),
    );
    const configuration = packConfiguration(actor.team.configuration_json);
    const snapshotPath = join(actor.team.project_path, ".maestro", "SLP.md");
    const snapshot = new Uint8Array(await readFile(snapshotPath));
    const digest = createHash("sha256").update(snapshot).digest("hex");
    if (digest !== actor.team.pack_digest) {
      throw new CliError(
        "SLP_SNAPSHOT_CHANGED",
        `running generation ${actor.team.team_id}:g${actor.team.generation} must keep its pinned Workspace Pack`,
      );
    }
    await requireSeatsUnchanged(configuration, actor.team);
    const peerSeat = await resolveSeat("peer", actor.team.project_path, resolveHomeDirectory());
    const knownPeer = roles.find((role) => role.name === peerName) ?? null;
    // room d128 (w728 rework): the launch line is the only carrier for the
    // model, and an acknowledged pane is never relaunched by work add - not
    // even by --fresh, whose /clear keeps the process. The role row does not
    // keep the launch model, so nothing could prove a repeated value matches
    // either; any --model on a reused pane is refused before a pane is
    // ensured and before any store write.
    if (modelOption !== undefined && knownPeer && knownPeer.ready_challenge !== "") {
      throw new CliError(
        "INVALID_OPTION",
        `--model rides the launch line only, and ${peerName} is already running in ${knownPeer.pane_id}; drop --model to reuse it as it is, or pick another Peer name to open a new pane with --model ${modelOption}`,
        { model: modelOption, pane: knownPeer.pane_id, peer: peerName },
      );
    }
    // Hub d101: a reset mid-item would drop the Peer's working context, so
    // --fresh is refused before anything is written while it holds ACTIVE work.
    if (fresh) {
      const held = context.store.database
        .query<{ id: string }, [string, number, string, string]>(
          `SELECT id FROM slp_work
           WHERE team_id = ? AND generation = ? AND state = 'ACTIVE'
             AND (owner = ? OR assigned_to = ?)
           ORDER BY id LIMIT 1`,
        )
        .get(actor.team.team_id, actor.team.generation, peerName, peerName);
      if (held) {
        throw new CliError(
          "PEER_ACTIVE",
          `${peerName} holds ${held.id} ACTIVE; --fresh resets a Peer pane only between items, wait for its return or add without --fresh`,
          { peer: peerName, work: held.id },
        );
      }
    }
    plan = await planForTeam(actor.team);
    const peerPlan: SlpRolePlan = {
      autocompact: peerSeat.config.autocompact ?? null,
      kind: peerSeat.config.harness,
      label: `slp:${actor.team.team_id}:g${actor.team.generation}:peer:${peerName}`,
      model: modelOption ?? configuration.models?.peer ?? null,
      name: peerName,
      profile: "peer",
      role: "peer",
    };
    const ensured = await runtime.ensurePeer(
      plan,
      peerPlan,
      roleContracts(
        actor.team.team_id,
        actor.team.generation,
        digest,
      ).get("peer") as SlpRoleContract,
      knownPeer && knownPeer.ready_challenge !== ""
        ? {
          briefDigest: knownPeer.brief_digest,
          instanceId: knownPeer.instance_id,
          packDigest: knownPeer.pack_digest,
          paneId: knownPeer.pane_id,
          readyChallenge: knownPeer.ready_challenge,
        }
        : null,
    );
    createdPeerTab = ensured.createdTabId;
    startedPeerPane = ensured.startedPaneId;
    let identity = ensured.role;
    // Hub d101: an acknowledged pane is reset and re-challenged before the
    // store write (d757 order: proven, then written); a reset that is not
    // proven refuses the whole add, nothing is written and no wake line goes
    // out (live g22 2026-09-05: an unproven reset went OPEN on stale context).
    if (fresh && ensured.reused) {
      const contract = roleContracts(
        actor.team.team_id,
        actor.team.generation,
        digest,
        { peer: identity.instanceId },
      ).get("peer") as SlpRoleContract;
      try {
        await runtime.resetPeer(plan, peerPlan, identity.paneId, contract);
      } catch (error) {
        throw new CliError(
          "PEER_RESET_FAILED",
          `could not reset ${peerName} in ${identity.paneId} before adding work: ${error instanceof Error ? error.message : String(error)}; nothing was added, retry --fresh or add without it`,
          { pane: identity.paneId, peer: peerName },
        );
      }
      identity = {
        ...identity,
        briefDigest: contract.briefDigest,
        readyChallenge: contract.readyChallenge,
      };
    }
    assignee = {
      name: identity.name,
      pane_id: identity.paneId,
      profile: identity.profile,
      role: identity.role,
      workspace_id: identity.workspaceId,
      instance_id: identity.instanceId,
      pack_digest: identity.packDigest,
      brief_digest: identity.briefDigest,
      ready_challenge: identity.readyChallenge,
    };
  }
  if (!assignee) throw new CliError("RUNTIME_INCOMPLETE", "work assignee is unavailable");
  const opensPeer = assignee.role === "peer";

  const now = new Date().toISOString();
  let attachedRoom = false;
  let id = "";
  try {
    if (opensPeer) {
      const roomStore = new Store(actor.team.room_store_path);
      try {
        migrateRoom(roomStore);
      } finally {
        roomStore.close();
      }
      context.store.database.query("ATTACH DATABASE ? AS slp_room").run(actor.team.room_store_path);
      attachedRoom = true;
    }
    context.store.database.exec("BEGIN IMMEDIATE");
    requireRunningGeneration(context.store, actor.team);
    if (opensPeer) {
      for (const table of ["slp_local_roles", "slp_room.slp_team_roles"]) {
        context.store.database
          .query(
            `INSERT INTO ${table}
              (team_id, generation, role, name, pane_id, workspace_id, instance_id,
               pack_digest, brief_digest, ready_challenge, profile, created_at)
             VALUES (?, ?, 'peer', ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(team_id, generation, name) DO UPDATE SET
               pane_id = excluded.pane_id,
               workspace_id = excluded.workspace_id,
               instance_id = excluded.instance_id,
               pack_digest = excluded.pack_digest,
               brief_digest = excluded.brief_digest,
               ready_challenge = excluded.ready_challenge,
               profile = excluded.profile`,
          )
          .run(
            actor.team.team_id,
            actor.team.generation,
            assignee.name,
            assignee.pane_id,
            assignee.workspace_id,
            assignee.instance_id,
            assignee.pack_digest,
            assignee.brief_digest,
            assignee.ready_challenge,
            assignee.profile,
            now,
          );
      }
    }
    id = context.store.nextCountedId("w", ["work", "slp_work", "slp_lifecycle_operations.work_id"]);
    context.store.database
      .query(
        `INSERT INTO slp_work
          (id, team_id, generation, objective, created_by, assigned_to, owner,
           state, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, NULL, 'OPEN', ?, ?)`,
      )
      .run(
        id,
        actor.team.team_id,
        actor.team.generation,
        objective,
        actor.name,
        assignee.name,
        now,
        now,
      );
    recordProjectActivity(context, actor, "work.add", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    let cleanupError: unknown = null;
    if ((startedPeerPane || createdPeerTab) && plan) {
      try {
        if (startedPeerPane) await runtime.closeStartedPane(plan, startedPeerPane);
        else if (createdPeerTab) await runtime.closeCreatedTab(plan, createdPeerTab);
      } catch (caught) {
        cleanupError = caught;
      }
    }
    if (cleanupError) throw cleanupError;
    throw error;
  } finally {
    if (attachedRoom) context.store.database.exec("DETACH DATABASE slp_room");
  }
  context.sessions.record("work.add");
  // Live row 17 (g18): an acknowledged Peer pane never learned about its next
  // item and a fresh one sat idle after READY, so the assignee is woken here
  // in both cases, the same d753 line as return and accept. A Lead's own item
  // (room d118) has no counterpart to wake.
  if (assignee.name !== actor.name) {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      assignee.name,
      `${id} OPEN`,
      objective,
      `maestro status ${id}`,
    );
  }
  const work = readSlpWork(context, actor, id);
  return {
    data: {
      role: {
        name: assignee.name,
        paneId: assignee.pane_id,
        profile: assignee.profile,
        role: assignee.role,
        workspaceId: assignee.workspace_id,
      },
      work: workData(work),
    },
    text: `${id} OPEN for ${assignee.name}`,
  };
}

export async function maybeHandleSlpWorkNote(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  // room d117: the Hub grants rework on the Lead's work. It is a
  // reviewer here and nothing more - a Hub --blocked note would be the Hub
  // escalating to itself, so it is refused rather than quietly recorded.
  // w736 F1: the plain note joins the grant. A reviewer answer is not always a
  // ruling - a fact, a commit hash, a pointer to a decision has no
  // `maestro decide` shape - and the room's plain note used to fall through to
  // the non-SLP work table, whose "work not found; run: maestro work list"
  // sent the reader to the wrong store. The room's OWN work items keep that
  // table and that verb; only an id it does not hold is resolved as SLP work,
  // so the failure names the SLP resolver.
  if (isRoom(context.store.database)) {
    const id = requiredPosition(invocation, 0, "work id");
    const flags = [
      ...(invocation.options.blocked === true ? ["--blocked"] : []),
      ...(invocation.options.rework === true ? ["--rework"] : []),
      ...(invocation.options.owner === true ? ["--owner"] : []),
    ];
    if (flags.length > 1) {
      throw new CliError("INVALID_OPTION", `${flags.join(" and ")} are separate notes`);
    }
    if (flags.length === 0 && roomHoldsLocalWork(context.store, id)) {
      // Two id spaces meet on this verb, and both are live: the room's own
      // `work` sequence and every team's. They do not overlap today, and the
      // room's d118/d125 collision in `decisions` is what one looks like when
      // it happens, so when both hold the id neither wins silently - the Hub
      // qualifies the team's item the way it already qualifies an ambiguous
      // one (w736 F1).
      let team: string | null = null;
      try {
        team = resolveHubWorkTarget(context, id, "read").teamId;
      } catch {
        team = null;
      }
      if (team === null) return null;
      throw new CliError(
        "AMBIGUOUS_WORK",
        `${id} names both a Hub work item and SLP work in ${team}; qualify the team's item as ${team}:${id}`,
        { candidates: [id, `${team}:${id}`] },
      );
    }
    if (invocation.options.blocked === true) {
      throw new CliError(
        "ROLE_FORBIDDEN",
        "the Hub Supervisor has no seat above it inside SLP, so --blocked from the room would be the" +
          " Hub escalating to itself (room d117); a question for the owner goes to the owner, and an" +
          " answer that settles a fork is maestro decide",
      );
    }
    if (invocation.options.owner === true) {
      throw new CliError(
        "ROLE_FORBIDDEN",
        "--owner is a seat's record of an owner walk-in into its own pane before it acts (room d123);" +
          " an owner instruction in the Hub pane is recorded as maestro decide --scope owner",
      );
    }
    const bound = bindHubReviewer(context, id);
    try {
      return await noteWorkAs(bound.context, bound.actor, invocation, bound.workId);
    } finally {
      bound.close();
    }
  }
  if (!requireActiveOrLegacy(context)) return null;
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["lead", "peer"]);
  return noteWorkAs(context, actor, invocation);
}

// The Hub room keeps its own non-SLP work items, whose ids run a separate
// sequence from any team's. A plain note on one of those is still the plain
// verb; anything else is SLP work and answers from the SLP resolver (w736 F1).
function roomHoldsLocalWork(store: Store, id: string): boolean {
  if (!tableExists(store, "work")) return false;
  return store.database
    .query<{ present: number }, [string]>("SELECT 1 AS present FROM work WHERE id = ?")
    .get(id) !== null;
}

// room d117: reviewer-shaped like acceptWorkAs, so the Hub's --rework grant is
// the same grant the Lead records, not a parallel implementation.
async function noteWorkAs(
  context: PluginContext,
  actor: SlpReviewerActor,
  invocation: CliInvocation,
  // The Hub may address work as `<team-id>:<work-id>`; seats always pass none.
  resolvedId?: string,
): Promise<CliResult> {
  requireRunningGeneration(context.store, actor.team);
  const id = resolvedId ?? requiredPosition(invocation, 0, "work id");
  const body = requiredPosition(invocation, 1, "note body");
  const work = requireSlpWork(context, actor, id);
  const rework = invocation.options.rework === true;
  const blocked = invocation.options.blocked === true;
  // room d123: --owner is provenance, not a state. It marks the note as an
  // owner walk-in recorded by the seat before acting, so the seat above can
  // tell an owner instruction from the seat changing course on its own.
  const owner = invocation.options.owner === true;
  const separate = [
    ...(blocked ? ["--blocked"] : []),
    ...(rework ? ["--rework"] : []),
    ...(owner ? ["--owner"] : []),
  ];
  if (separate.length > 1) {
    throw new CliError("INVALID_OPTION", `${separate.join(" and ")} are separate notes`);
  }
  if (actor.role === "peer" && work.assigned_to !== actor.name) {
    throw new CliError("ROLE_FORBIDDEN", `${actor.name} may note only its assigned work`);
  }
  if (rework) {
    requireWorkReviewer(context, actor, work, "grant rework");
    if (work.state !== "RETURNED") {
      throw new CliError(
        "INVALID_STATE",
        `${id} must be RETURNED before its reviewer grants rework`,
      );
    }
  }
  const now = new Date().toISOString();
  const flag = blocked ? "blocked" : owner ? "owner" : null;
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningGeneration(context.store, actor.team);
    const current = requireSlpWork(context, actor, id);
    if (rework) {
      requireWorkReviewer(context, actor, current, "grant rework");
      if (current.state !== "RETURNED" || current.return_revision <= 0) {
        throw new CliError(
          "INVALID_STATE",
          `${id} changed before its reviewer could grant rework`,
        );
      }
      const existing = context.store.database
        .query<{ present: number }, [string, number]>(
          `SELECT 1 AS present FROM slp_rework_grants
           WHERE work_id = ? AND return_revision = ?`,
        )
        .get(id, current.return_revision);
      if (existing) {
        throw new CliError(
          "REWORK_ALREADY_GRANTED",
          `${id} return revision ${current.return_revision} already has a rework grant`,
        );
      }
      context.store.database
        .query(
          `INSERT INTO slp_rework_grants
            (work_id, return_revision, reviewer, granted_at)
           VALUES (?, ?, ?, ?)`,
        )
        .run(id, current.return_revision, actor.name, now);
      markLessonCandidate(
        context.store,
        id,
        `rework note on revision ${current.return_revision}`,
        now,
      );
    }
    context.store.database
      .query(
        `INSERT INTO slp_work_entries (work_id, kind, actor, body, flag, created_at)
         VALUES (?, 'NOTE', ?, ?, ?, ?)`,
      )
      .run(id, actor.name, body, flag, now);
    recordProjectActivity(context, actor, "work.note", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  context.sessions.record("work.note");
  if (rework) {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      work.assigned_to,
      `${id} RETURNED`,
      `rework granted: ${body}`,
      `maestro status ${id}`,
    );
  }
  // w736 F1: a seat's plain note wakes nobody - the shared contract is explicit
  // that an instruction whose value expires travels on a waking operation and
  // never on a plain note. The Hub is not a seat in the team: it holds no
  // `work add`, no pane, and no other line down, so its plain note is the one
  // it has and it is delivered. The exception is exactly one seat wide.
  if (!rework && actor.role === hubReviewerName) {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      work.assigned_to,
      `${id} NOTE`,
      body,
      `maestro status ${id}`,
    );
  }
  // d761: the blocked note wakes the seat above the actor, never the reviewer
  // of the item, because the one who is stuck is the one escalating; the seat
  // above the Lead is the Hub (room d117).
  // room d123: the owner walk-in rides the same path with its own subject; it
  // is one line up for the record, never a stall and never the blocked flag.
  if (blocked || owner) {
    await pushOneSeatUp(context, actor, id, blocked ? "BLOCKED" : "OWNER", body);
  }
  const kind = rework ? "rework grant" : blocked ? "blocked note" : owner ? "owner note" : "note";
  return {
    data: {
      note: { actor: actor.name, body, createdAt: now, flag, rework },
      work: workData(readSlpWork(context, actor, id)),
    },
    text: `${id} ${kind} by ${actor.name}: ${body}`,
  };
}

// The one upward push a seat has: Peer to Lead, and the Lead to the Hub pane
// (room d117). The Hub's line carries the team and points at the room's own
// status, because the Hub cannot read a team item by bare id.
async function pushOneSeatUp(
  context: PluginContext,
  actor: SlpReviewerActor,
  id: string,
  subject: "BLOCKED" | "OWNER",
  body: string,
): Promise<void> {
  // The Hub reviewer never notes --blocked or --owner (refused at the room
  // path), and it has no seat above to push to.
  if (actor.role === hubReviewerName) return;
  const toHub = actor.role === "lead";
  await pushNotice(
    actor.team.project_path,
    actor.role,
    toHub ? hubSupervisorTarget(actor.team) : rolePaneName(context, actor, "lead"),
    `${id} ${subject}`,
    toHub ? `${noticeSummary(body)} in ${actor.team.team_id} g${actor.team.generation}` : body,
    toHub ? "maestro status" : `maestro status ${id}`,
  );
}

// w740 (d898): opening a Full-tier bundle on an item is the store's signal
// that a design phase has begun, and it reaches the reviewer above the one
// designing - the Hub for the Lead, the Lead for a Peer.
// Every failure here is silent: a bundle open outside a running team, from a
// pane with no role, or on an id no SLP item holds, is an ordinary bundle open.
export async function pushDesignOpened(
  context: PluginContext,
  bundleId: string,
  workIds: readonly string[],
): Promise<void> {
  if (workIds.length === 0) return;
  let actor: SlpActor;
  try {
    actor = requireSlpActor(context, ["lead", "peer"]);
  } catch {
    return;
  }
  for (const workId of workIds) {
    const work = context.store.database
      .query<SlpWorkRow, [string, string, number]>(
        "SELECT * FROM slp_work WHERE id = ? AND team_id = ? AND generation = ?",
      )
      .get(workId, actor.team.team_id, actor.team.generation);
    if (!work) continue;
    const toHub = expectedReviewer(context, actor, work).kind === "hub";
    await pushNotice(
      actor.team.project_path,
      actor.role,
      reviewerPushTarget(context, actor, work),
      `${workId} DESIGN`,
      toHub
        ? `design bundle ${bundleId} in ${actor.team.team_id} g${actor.team.generation}`
        : `design bundle ${bundleId}`,
      toHub ? "maestro status" : `maestro status ${workId}`,
    );
  }
}

export function takeWork(context: PluginContext, id: string): CliResult {
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["lead", "peer"]);
  requireRunningGeneration(context.store, actor.team);
  const work = requireSlpWork(context, actor, id);
  if (work.assigned_to !== actor.name) {
    throw new CliError("ROLE_FORBIDDEN", `${id} is assigned to ${work.assigned_to}`, {
      actor: actor.name,
      assignedTo: work.assigned_to,
    });
  }
  if (work.state !== "OPEN" && work.state !== "RETURNED") {
    throw new CliError("INVALID_STATE", `${id} must be OPEN or RETURNED before work take`);
  }
  const now = new Date().toISOString();
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningGeneration(context.store, actor.team);
    const current = requireSlpWork(context, actor, id);
    if (current.assigned_to !== actor.name) {
      throw new CliError("ROLE_FORBIDDEN", `${id} is assigned to ${current.assigned_to}`);
    }
    if (current.state !== "OPEN" && current.state !== "RETURNED") {
      throw new CliError("INVALID_STATE", `${id} changed before work take`);
    }
    if (current.state === "RETURNED") {
      const grant = context.store.database
        .query<{ consumed_at: string | null }, [string, number, string]>(
          `SELECT consumed_at FROM slp_rework_grants
           WHERE work_id = ? AND return_revision = ? AND reviewer <> ?`,
        )
        .get(id, current.return_revision, actor.name);
      if (!grant || grant.consumed_at !== null) {
        throw new CliError(
          "REWORK_REQUIRED",
          `${id} return revision ${current.return_revision} requires an unused grant from its reviewer`,
        );
      }
    }
    const transition = context.store.database
      .query(
        `UPDATE slp_work
         SET state = 'ACTIVE', owner = ?, current_return = NULL, updated_at = ?
         WHERE id = ? AND state = ?`,
      )
      .run(actor.name, now, id, current.state);
    requireWorkTransition(transition.changes, id, "work take");
    if (current.state === "RETURNED") {
      const consumed = context.store.database
        .query(
          `UPDATE slp_rework_grants SET consumed_at = ?
           WHERE work_id = ? AND return_revision = ? AND consumed_at IS NULL`,
        )
        .run(now, id, current.return_revision);
      requireWorkTransition(consumed.changes, id, "rework grant consumption");
    }
    recordProjectActivity(context, actor, "work.take", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  context.sessions.record("work.take");
  const current = readSlpWork(context, actor, id);
  return { data: { work: workData(current) }, text: `${id} ACTIVE by ${actor.name}` };
}

export async function returnWork(context: PluginContext, id: string, body: string): Promise<CliResult> {
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["lead", "peer"]);
  requireRunningGeneration(context.store, actor.team);
  const work = requireSlpWork(context, actor, id);
  if (work.state !== "ACTIVE" || work.owner !== actor.name) {
    throw new CliError(
      "INVALID_STATE",
      `${id} must be ACTIVE and owned by ${actor.name} before work return`,
    );
  }
  const now = new Date().toISOString();
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningGeneration(context.store, actor.team);
    const transition = context.store.database
      .query(
        `UPDATE slp_work
         SET state = 'RETURNED', owner = NULL, current_return = ?,
             return_revision = return_revision + 1, updated_at = ?
         WHERE id = ? AND state = 'ACTIVE' AND owner = ?`,
      )
      .run(body, now, id, actor.name);
    requireWorkTransition(transition.changes, id, "work return");
    context.store.database
      .query(
        `INSERT INTO slp_work_entries (work_id, kind, actor, body, created_at)
         VALUES (?, 'RETURN', ?, ?, ?)`,
      )
      .run(id, actor.name, body, now);
    recordProjectActivity(context, actor, "work.return", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  context.sessions.record("work.return");
  const current = readSlpWork(context, actor, id);
  await pushNotice(
    actor.team.project_path,
    actor.role,
    reviewerPushTarget(context, actor, current),
    `${id} RETURNED`,
    body,
    `maestro status ${id}`,
  );
  return { data: { work: workData(current) }, text: `${id} RETURNED by ${actor.name}` };
}

export async function acceptWork(context: PluginContext, id: string, outcome: string): Promise<CliResult> {
  // room d117: from the Hub room this is the reviewer boundary of the Lead's
  // work; from a team workspace it is the seat boundary it has always been.
  if (isRoom(context.store.database)) {
    const bound = bindHubReviewer(context, id);
    try {
      return await acceptWorkAs(bound.context, bound.actor, bound.workId, outcome);
    } finally {
      bound.close();
    }
  }
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["lead"]);
  return acceptWorkAs(context, actor, id, outcome);
}

// room d117: the body below is reviewer-shaped rather than seat-shaped, so the
// Hub Supervisor runs exactly the same transition, entry and activity row the
// Lead does - one accept path, two callers, no second code path to drift.
// `context.store` is the PROJECT store in both cases.
async function acceptWorkAs(
  context: PluginContext,
  actor: SlpReviewerActor,
  id: string,
  outcome: string,
): Promise<CliResult> {
  requireRunningGeneration(context.store, actor.team);
  const work = requireSlpWork(context, actor, id);
  requireWorkReviewer(context, actor, work, "accept");
  if (outcome !== "accepted" && outcome !== "cancelled") {
    throw new CliError("INVALID_VALUE", "--outcome must be accepted or cancelled");
  }
  if (outcome === "cancelled" && work.state === "ACTIVE") {
    throw new CliError(
      "INVALID_STATE",
      `${id} is ACTIVE; its assignee must work return before cancellation`,
    );
  }
  if (
    (outcome === "accepted" && work.state !== "RETURNED") ||
    (outcome === "cancelled" && work.state !== "OPEN" && work.state !== "RETURNED")
  ) {
    throw new CliError(
      "INVALID_STATE",
      `${id} cannot be accepted with outcome ${outcome} from ${work.state}`,
    );
  }
  const now = new Date().toISOString();
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningGeneration(context.store, actor.team);
    const transition = outcome === "accepted"
      ? context.store.database
        .query(
          `UPDATE slp_work
           SET state = 'DONE', acceptance_outcome = ?, accepted_by = ?, updated_at = ?
           WHERE id = ? AND state = 'RETURNED'`,
        )
        .run(outcome, actor.name, now, id)
      : context.store.database
        .query(
          `UPDATE slp_work
           SET state = 'DONE', acceptance_outcome = ?, accepted_by = ?, updated_at = ?
           WHERE id = ? AND state IN ('OPEN', 'RETURNED')`,
        )
        .run(outcome, actor.name, now, id);
    requireWorkTransition(transition.changes, id, "work accept");
    context.store.database
      .query(
        `INSERT INTO slp_work_entries (work_id, kind, actor, body, created_at)
         VALUES (?, 'ACCEPTANCE', ?, ?, ?)`,
      )
      .run(id, actor.name, outcome, now);
    recordProjectActivity(context, actor, "work.accept", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  context.sessions.record("work.accept");
  const current = readSlpWork(context, actor, id);
  await pushNotice(
    actor.team.project_path,
    actor.role,
    current.assigned_to,
    `${id} DONE`,
    outcome,
    `maestro status ${id}`,
  );
  if (current.created_by === "hub-supervisor") {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      hubSupervisorTarget(actor.team),
      `${id} DONE`,
      `${outcome} in ${actor.team.team_id} g${actor.team.generation}`,
      "maestro status",
    );
  }
  return {
    data: { work: workData(current) },
    text: `${id} DONE (${outcome}) by ${actor.name}`,
  };
}
