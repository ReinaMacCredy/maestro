import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { HerdrClient } from "../src/plugins/herdr-client.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { runtimeLockHolder } from "../src/plugins/slp-attention.ts";
import { slpRuntimeDirectory } from "../src/plugins/slp-process.ts";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
  tripwireInvocations,
  waitForFakeHerdr,
} from "./helpers-herdr.ts";
import { runCliAt, withFixture, type CliResult, type Fixture } from "./helpers.ts";

// w711: an orphan team. A generation is supervised by its Lead and its Team
// Supervisor; when both of those panes are gone there is nobody left to run
// `maestro team stop`, and the Peers keep holding work for a team that can no
// longer review or close it. Every observation point that could notice the
// loss - the attention runtime's own pane-loss handler, the `maestro slp
// event` hook Herdr runs when no runtime is subscribed, and the `maestro slp
// restore` startup hook - is driven here in one scenario, and only the
// outcome is asserted: the generation is no longer RUNNING and a COMMITTED
// STOP is on the record. Which of the three notices, and how, is the fix's
// choice, not this test's.

interface StartedTeam {
  team: {
    generation: number;
    roles: Array<{ name: string; paneId: string; role: string }>;
    runtimePaneId: string;
    teamId: string;
    workspaceId: string;
  };
  work: { id: string };
}

interface AddedPeer {
  role: { name: string; paneId: string };
  work: { id: string };
}

function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

async function markedRoom(fixture: Fixture): Promise<string> {
  const room = await scaffoldRoom(fixture.home);
  const marked = await runCliAt(fixture, room, ["room", "mark"], {
    MAESTRO_ROOM_SCAFFOLD: "1",
    MAESTRO_SESSION_NONE: "1",
  });
  expect(marked.exitCode).toBe(0);
  return room;
}

// The two facts the Hub reads about a generation it can no longer reach.
interface OrphanOutcome {
  committedStops: number;
  state: string;
}

function orphanOutcome(room: string, team: StartedTeam["team"]): OrphanOutcome {
  const database = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
  try {
    const state = database
      .query<{ state: string }, [string, number]>(
        `SELECT state FROM slp_teams WHERE team_id = ? AND generation = ?`,
      )
      .get(team.teamId, team.generation)?.state ?? "missing";
    const committedStops = database
      .query<{ count: number }, [string, number]>(
        `SELECT COUNT(*) AS count FROM slp_lifecycle_operations
         WHERE team_id = ? AND generation = ? AND operation = 'STOP' AND phase = 'COMMITTED'`,
      )
      .get(team.teamId, team.generation)?.count ?? 0;
    return { committedStops, state };
  } finally {
    database.close();
  }
}

// Polls without throwing: a timeout here is the reproducer's subject, so it
// must reach the expect below rather than fail as a helper error.
async function settle(condition: () => boolean, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (condition()) return;
    await Bun.sleep(50);
  }
}

test("orphan-team: a RUNNING generation whose Lead and Team Supervisor panes are both gone is left RUNNING with no COMMITTED STOP, by the attention runtime, the slp event hook and slp restore alike, while its Peer still holds work (w711)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { runtimePane: "spawn" });
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Orphan me", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;

    // The runtime discards Herdr's replay after every subscribe; the panes go
    // only once its latest subscription covers every role pane, drained.
    const logPath = join(
      slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation),
      "runtime.log",
    );
    const subscribed = async (): Promise<void> => {
      await waitForFakeHerdr(async () => {
        const state = await readFakeHerdrState(fake);
        const roles = (state.agents as Array<{ pane_id: string; workspace_id: string }>)
          .filter((agent) => agent.workspace_id === data.team.workspaceId)
          .map((agent) => agent.pane_id);
        if (!existsSync(logPath)) return false;
        const lines = (await readFile(logPath, "utf8")).split("\n");
        const subscribedLines = lines.filter((line) => line.includes(" subscribed: "));
        const drained = lines.filter((line) => line.includes(" replay drained: ")).length;
        const latest = subscribedLines.at(-1) ?? "";
        return subscribedLines.length > 0 && drained === subscribedLines.length &&
          roles.every((pane) => latest.includes(`pane.agent_status_changed:${pane}`));
      }, 15_000, "the runtime subscription to every role pane, replay drained");
    };
    await subscribed();

    // A Peer holding delivery work is what makes the loss matter: it survives
    // the two panes above it and keeps an ACTIVE item nobody can accept.
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer item", "--to", "peer-orphaned", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead.paneId },
    );
    expect(added.exitCode).toBe(0);
    const peer = envelope<AddedPeer>(added.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id], peerEnvironment)).exitCode).toBe(0);
    await subscribed();

    // Observation point one: both supervising panes close under a live,
    // subscribed attention runtime. The Peer pane stays.
    const client = new HerdrClient(fake.env);
    await client.paneClose(supervisor.paneId);
    await client.paneClose(lead.paneId);
    const survivors = (await readFakeHerdrState(fake)).agents as Array<{ name: string; pane_id: string }>;
    expect(survivors.map((agent) => agent.pane_id)).toContain(peer.role.paneId);
    expect(survivors.map((agent) => agent.pane_id)).not.toContain(lead.paneId);
    expect(survivors.map((agent) => agent.pane_id)).not.toContain(supervisor.paneId);
    await settle(() => orphanOutcome(room, data.team).state !== "RUNNING", 3_000);

    // Observation points two and three run without a runtime holding the
    // lock, the way they do after Herdr has restarted.
    await client.paneClose(data.team.runtimePaneId);
    const runtimeDirectory = slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation);
    await waitForFakeHerdr(
      async () => (await runtimeLockHolder(runtimeDirectory)) === null,
      10_000,
      "the runtime lock to clear",
    );
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    for (const paneId of [supervisor.paneId, lead.paneId]) {
      const hooked = await runCliAt(fixture, room, ["slp", "event", "--json"], {
        ...fake.env,
        HERDR_PLUGIN_EVENT: "pane.closed",
        HERDR_PLUGIN_EVENT_JSON: JSON.stringify({
          event: "pane_closed",
          data: { pane_id: paneId, workspace_id: data.team.workspaceId },
        }),
      });
      expect(hooked.exitCode).toBe(0);
    }
    const restored = await runCliAt(fixture, room, ["slp", "restore", "--json"], fake.env);
    expect(restored.exitCode).toBe(0);
    await settle(() => orphanOutcome(room, data.team).state !== "RUNNING", 3_000);

    // The outcome, and only the outcome: with no Lead and no Team Supervisor
    // the generation must not still be RUNNING, and the STOP must be on the
    // record. On current main it is neither - the Peer's ACTIVE item below is
    // held for a team nobody can close.
    const peerState = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const heldByPeer = peerState
      .query<{ state: string }, [string]>(`SELECT state FROM slp_work WHERE id = ?`)
      .get(peer.work.id)?.state;
    peerState.close();
    expect(heldByPeer).toBe("ACTIVE");
    expect(orphanOutcome(room, data.team)).toEqual({ committedStops: 1, state: "STOPPED" });
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 120_000);

// w713: the auto-close itself. Every test below drives the Herdr [[events]]
// hook (`maestro slp event`) with no runtime subscribed, which is the only
// point d870 lets commit a STOP.

interface StopFacts {
  abandoned: Array<{ abandonment_reason: string; abandoned_by: string; id: string; state: string }>;
  emergencyActivity: number;
  localState: string;
  roomEmergencyActivity: number;
  roomState: string;
  stop: { actor: string; emergency: number; phase: string } | null;
  unabandoned: number;
}

function stopFacts(repo: string, room: string, team: StartedTeam["team"]): StopFacts {
  const project = new Database(join(repo, ".maestro", "maestro.db"), { readonly: true });
  const hub = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
  try {
    const localState = project
      .query<{ state: string }, [string, number]>(
        `SELECT state FROM slp_local_teams WHERE team_id = ? AND generation = ?`,
      )
      .get(team.teamId, team.generation)?.state ?? "missing";
    const roomState = hub
      .query<{ state: string }, [string, number]>(
        `SELECT state FROM slp_teams WHERE team_id = ? AND generation = ?`,
      )
      .get(team.teamId, team.generation)?.state ?? "missing";
    const stop = project
      .query<{ actor: string; emergency: number; phase: string }, [string, number]>(
        `SELECT phase, actor, emergency FROM slp_lifecycle_operations
         WHERE team_id = ? AND generation = ? AND operation = 'STOP'`,
      )
      .get(team.teamId, team.generation) ?? null;
    const abandoned = project
      .query<{ abandonment_reason: string; abandoned_by: string; id: string; state: string }, [string, number]>(
        `SELECT id, state, abandoned_by, abandonment_reason FROM slp_work
         WHERE team_id = ? AND generation = ? AND abandoned_at IS NOT NULL ORDER BY id`,
      )
      .all(team.teamId, team.generation);
    const unabandoned = project
      .query<{ count: number }, [string, number]>(
        `SELECT COUNT(*) AS count FROM slp_work
         WHERE team_id = ? AND generation = ? AND state <> 'DONE' AND abandoned_at IS NULL`,
      )
      .get(team.teamId, team.generation)?.count ?? 0;
    const emergency = (database: Database) =>
      database
        .query<{ count: number }, [string, number]>(
          `SELECT COUNT(*) AS count FROM slp_activity
           WHERE team_id = ? AND generation = ? AND operation = 'team.stop.emergency'`,
        )
        .get(team.teamId, team.generation)?.count ?? 0;
    return {
      abandoned,
      emergencyActivity: emergency(project),
      localState,
      roomEmergencyActivity: emergency(hub),
      roomState,
      stop,
      unabandoned,
    };
  } finally {
    project.close();
    hub.close();
  }
}

function paneLossEntries(repo: string): Array<{ body: string; flag: string | null }> {
  const database = new Database(join(repo, ".maestro", "maestro.db"), { readonly: true });
  try {
    return database
      .query<{ body: string; flag: string | null }, []>(
        `SELECT body, flag FROM slp_work_entries WHERE actor = 'runtime' ORDER BY id`,
      )
      .all();
  } finally {
    database.close();
  }
}

interface HookFixture {
  data: StartedTeam;
  fake: Awaited<ReturnType<typeof installFakeHerdr>>;
  room: string;
}

// A started generation with no runtime process, its project in the Hub
// registry so the hooks can find it.
async function startForHook(
  fixture: Fixture,
  objective: string,
  behavior: Parameters<typeof installFakeHerdr>[1] = {},
): Promise<HookFixture> {
  const room = await markedRoom(fixture);
  const fake = await installFakeHerdr(fixture, { runtimePane: "record", ...behavior });
  const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, objective, "--json"], fake.env);
  expect(started.exitCode).toBe(0);
  await mkdir(join(fixture.home, "maestro"), { recursive: true });
  await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
  return { data: envelope<StartedTeam>(started.stdout), fake, room };
}

function paneEvent(
  fixture: Fixture,
  hook: HookFixture,
  event: "pane_closed" | "pane_exited",
  paneId: string,
): Promise<CliResult> {
  return runCliAt(fixture, hook.room, ["slp", "event", "--json"], {
    ...hook.fake.env,
    HERDR_PLUGIN_EVENT: event.replace("_", "."),
    HERDR_PLUGIN_EVENT_JSON: JSON.stringify({
      event,
      data: { pane_id: paneId, workspace_id: hook.data.team.workspaceId },
    }),
  });
}

// Herdr lost these panes: they leave agent.list, pane.list and the process
// table together, the way a closed pane does.
async function dropPanes(hook: HookFixture, paneIds: string[]): Promise<void> {
  await editFakeHerdrState(hook.fake, (state) => {
    state.agents = state.agents.filter((agent: { pane_id: string }) => !paneIds.includes(agent.pane_id));
    state.panes = state.panes.filter((pane: { pane_id: string }) => !paneIds.includes(pane.pane_id));
    for (const paneId of paneIds) delete state.processes[paneId];
  });
}

function seats(hook: HookFixture): { lead: StartedTeam["team"]["roles"][number]; supervisor: StartedTeam["team"]["roles"][number] } {
  return {
    lead: hook.data.team.roles.find((role) => role.role === "lead")!,
    supervisor: hook.data.team.roles.find((role) => role.role === "team-supervisor")!,
  };
}

test("orphan-close: a pane.closed for the Lead of a generation whose Team Supervisor pane is already gone commits the STOP - both stores STOPPED, the STOP row COMMITTED with the orphan actor and emergency 1, every non-DONE item abandoned with the orphan evidence, and the Hub Supervisor notified (w713, d869, d870, d872)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Orphan close");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    const outcome = envelope<{ abandonedWorkCount: number; orphaned: boolean; stopped: boolean }>(closed.stdout);
    expect(outcome.orphaned).toBe(true);
    expect(outcome.stopped).toBe(true);

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("STOPPED");
    expect(facts.roomState).toBe("STOPPED");
    expect(facts.stop).toEqual({ actor: "runtime-orphan", emergency: 1, phase: "COMMITTED" });
    // d869: the same abandonment shape an emergency stop writes.
    expect(facts.unabandoned).toBe(0);
    expect(facts.abandoned.length).toBeGreaterThan(0);
    expect(outcome.abandonedWorkCount).toBe(facts.abandoned.length);
    for (const item of facts.abandoned) {
      expect(item.abandoned_by).toBe("runtime-orphan");
      expect(item.abandonment_reason).toContain("orphaned:");
      expect(item.abandonment_reason).toContain(`lead ${lead.name}`);
      expect(item.abandonment_reason).toContain(`team-supervisor ${supervisor.name}`);
    }

    // d872: the seat that would normally report the stop is the one that died.
    const notices = (await fakeHerdrCommands(hook.fake))
      .filter((command) => command[0] === "agent" && command[1] === "prompt")
      .filter((command) => (command[3] ?? "").includes("orphan auto-close"));
    expect(notices).toHaveLength(1);
    expect(notices[0]?.[3]).toContain(`${hook.data.team.teamId} g${hook.data.team.generation} STOPPED`);
    expect(notices[0]?.[3]).toContain("runtime-orphan");
    expect(notices[0]?.[3]).toContain(`lead ${lead.name}`);
    expect(notices[0]?.[3]).toContain(`team-supervisor ${supervisor.name}`);
    expect(notices[0]?.[3]).toContain(`${facts.abandoned.length} unfinished work item`);
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

test("orphan-herdr-down: with agent.list answered by a Herdr error the same event commits nothing - both stores still RUNNING, no STOP row, no abandonment - because an outage must never read as total loss; but the ordinary pane-loss entry IS still recorded, because the event is Herdr's own evidence that that pane died and rests on nothing agent.list could contradict (w715, d874 superseding d871)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Herdr down");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);
    await setFakeHerdrBehavior(hook.fake, { failMethods: ["agent.list"] });

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    // w717 F3, d875, changed rather than deleted: this asserted a NON-zero
    // exit, which was the throw escaping the hook. The abort is the same -
    // nothing committed - but it is now caught at the hook boundary and
    // reported, because the predicate runs on every role-pane loss including
    // the ones a subscribed runtime owns, and failing those hooks is a cost
    // d874 never asked for. Aborting is still the opposite of concluding loss.
    expect(closed.exitCode).toBe(0);
    const aborted = envelope<{ aborted?: string; orphaned?: boolean; reason?: string }>(closed.stdout);
    expect(aborted.aborted).toBe("orphan-check");
    expect(aborted.orphaned).toBeUndefined();
    expect(aborted.reason).toContain("agent.list");

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("RUNNING");
    expect(facts.roomState).toBe("RUNNING");
    expect(facts.stop).toBeNull();
    expect(facts.abandoned).toEqual([]);
    // d874, the half that changed: the ordinary pane-loss entry survives the
    // outage. This hook is the safety net that records a pane death when no
    // runtime is subscribed; losing that record exactly when Herdr is flaky
    // would defeat it in the case it exists for. Only the orphan conclusion
    // needed agent.list, and only the orphan conclusion was abandoned.
    const entries = paneLossEntries(fixture.repo);
    expect(entries.map((entry) => entry.flag)).toEqual(["pane:closed"]);
    expect(entries[0]?.body).toContain(`closed: ${lead.name} pane ${lead.paneId}`);
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

test("orphan-no-workspace-is-not-orphan: agent.list answers successfully and still lists both seats while no workspace carries the plan label; nothing is committed, pinning that the decision never reads plan.workspaceLabel (w713, d871)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "No workspace");
    const { lead } = seats(hook);
    // Herdr is back but its workspaces are not: the seats are still listed.
    await editFakeHerdrState(hook.fake, (state) => {
      state.workspaces = [];
    });

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    expect(envelope<{ handled: boolean; orphaned?: boolean }>(closed.stdout).orphaned).toBeUndefined();

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("RUNNING");
    expect(facts.roomState).toBe("RUNNING");
    expect(facts.stop).toBeNull();
    expect(facts.abandoned).toEqual([]);
    // The ordinary pane-loss record still happens: only the stop is withheld.
    expect(paneLossEntries(fixture.repo).map((entry) => entry.flag)).toEqual(["pane:closed"]);
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

test("orphan-repair-fenced: while a runtime repair holds the START row with a live owner the event commits nothing and reports wait, because a one-shot hook must abort rather than spin on a reservation another process owns (w713)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Repair fenced");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);
    // A `team start` repair in flight: the START row is owned by a live pid.
    for (const path of [join(fixture.repo, ".maestro", "maestro.db"), join(hook.room, ".maestro", "maestro.db")]) {
      const database = new Database(path);
      database
        .query(
          `UPDATE slp_lifecycle_operations SET owner_token = ?, owner_pid = ?
           WHERE team_id = ? AND generation = ? AND operation = 'START'`,
        )
        .run("repair-in-flight", process.pid, hook.data.team.teamId, hook.data.team.generation);
      database.close();
    }

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    expect(envelope<{ orphaned: boolean; outcome: string }>(closed.stdout)).toMatchObject({
      orphaned: true,
      outcome: "wait",
    });

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("RUNNING");
    expect(facts.roomState).toBe("RUNNING");
    expect(facts.stop).toBeNull();
    expect(facts.abandoned).toEqual([]);
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

test("orphan-idempotent: a pane.exited and then a pane.closed for the same dead generation commit once - one COMMITTED STOP row and one team.stop.emergency in each store - and the second call is exit 0 with handled false (w713)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Idempotent");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);

    const first = await paneEvent(fixture, hook, "pane_exited", lead.paneId);
    expect(first.exitCode).toBe(0);
    expect(envelope<{ stopped: boolean }>(first.stdout).stopped).toBe(true);

    const second = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(second.exitCode).toBe(0);
    expect(envelope<{ handled: boolean }>(second.stdout).handled).toBe(false);

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.stop).toEqual({ actor: "runtime-orphan", emergency: 1, phase: "COMMITTED" });
    expect(facts.emergencyActivity).toBe(1);
    expect(facts.roomEmergencyActivity).toBe(1);
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

test("orphan-peers-alive: a Peer pane alive in agent.list does not save the generation - the Lead and the Team Supervisor both being gone is the orphan, and the Peer's held item is abandoned with the rest (w713, d871)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Peers alive");
    const { lead, supervisor } = seats(hook);
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer item", "--to", "peer-survivor", "--json"],
      { ...hook.fake.env, HERDR_PANE_ID: lead.paneId },
    );
    expect(added.exitCode).toBe(0);
    const peer = envelope<AddedPeer>(added.stdout);
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id], {
        ...hook.fake.env,
        HERDR_PANE_ID: peer.role.paneId,
      })).exitCode,
    ).toBe(0);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);
    // The surviving Peer is still listed; `alive.length === 0` would miss this.
    const listed = (await readFakeHerdrState(hook.fake)).agents as Array<{ pane_id: string }>;
    expect(listed.map((agent) => agent.pane_id)).toContain(peer.role.paneId);

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    expect(envelope<{ stopped: boolean }>(closed.stdout).stopped).toBe(true);

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("STOPPED");
    expect(facts.roomState).toBe("STOPPED");
    expect(facts.unabandoned).toBe(0);
    expect(facts.abandoned.map((item) => item.id)).toContain(peer.work.id);
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

test("orphan-status-verdict: Hub status reports a RUNNING generation with neither a lead nor a team-supervisor pane as ORPHANED? and names maestro team stop --emergency, reports a healthy one and a lead-only loss as not orphaned, and commits nothing on any of those reads (w715, d870, d873)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Status verdict");
    const { lead, supervisor } = seats(hook);
    // The verdict has to land in both renderings: --json carries the data the
    // brief asks for, the plain read is what a person actually sees.
    const hubStatus = async () => {
      const shown = await runCliAt(fixture, hook.room, ["status", "--json"], hook.fake.env);
      expect(shown.exitCode).toBe(0);
      const plain = await runCliAt(fixture, hook.room, ["status"], hook.fake.env);
      expect(plain.exitCode).toBe(0);
      return {
        team: envelope<{ teams: Array<{ missingPanes: string[]; orphaned: boolean; teamId: string }> }>(shown.stdout).teams[0]!,
        text: plain.stdout,
      };
    };

    // Healthy: no verdict.
    const healthy = await hubStatus();
    expect(healthy.team.orphaned).toBe(false);
    expect(healthy.text).not.toContain("ORPHANED?");

    // Only the Lead is gone: the Team Supervisor can still stop the team, so
    // this is degraded, not orphaned.
    await dropPanes(hook, [lead.paneId]);
    const leadOnly = await hubStatus();
    expect(leadOnly.team.missingPanes).toEqual([lead.name]);
    expect(leadOnly.team.orphaned).toBe(false);
    expect(leadOnly.text).not.toContain("ORPHANED?");

    // Both supervising seats gone: the verdict, as a question, with the
    // command that closes it by hand.
    await dropPanes(hook, [supervisor.paneId]);
    const orphaned = await hubStatus();
    expect(orphaned.team.orphaned).toBe(true);
    expect(orphaned.team.missingPanes.sort()).toEqual([lead.name, supervisor.name].sort());
    expect(orphaned.text).toContain("ORPHANED?");
    expect(orphaned.text).toContain("no lead or team-supervisor pane");
    expect(orphaned.text).toContain(`maestro team stop ${hook.data.team.teamId} --emergency`);
    // Not presented as certainty: the read cannot tell a downed Herdr apart.
    expect(orphaned.text).toContain("a Herdr that is down reads the same from here");

    // d870: a status read never stops anything. Three reads later the
    // generation is untouched - still RUNNING in both stores, no STOP row,
    // nothing abandoned.
    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("RUNNING");
    expect(facts.roomState).toBe("RUNNING");
    expect(facts.stop).toBeNull();
    expect(facts.abandoned).toEqual([]);
    expect(facts.emergencyActivity).toBe(0);
    expect(facts.roomEmergencyActivity).toBe(0);
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

// w717: the w716 review's findings, each staged as the scenario it named.
// d875 governs all of them - every ambiguity on the commit path resolves to
// ABORT, never to stop - so each of these asserts the same negative shape:
// both stores still RUNNING, no STOP row, nothing abandoned, exit 0.

// The reason string the hook builds for a pane_closed on the Lead. Mirrored
// here because the EMERGENCY_REASON_CHANGED guard compares reasons literally,
// and the precedence case below only exists when they match exactly. If the
// template in runSlpEvent drifts, this stops matching and that test fails on
// its abandoned_by assertion rather than passing for the wrong reason.
function orphanReasonForLead(hook: HookFixture): string {
  const { lead, supervisor } = seats(hook);
  return `orphaned: ${hook.data.team.teamId}:g${hook.data.team.generation} ` +
    `lost lead ${lead.name} and team-supervisor ${supervisor.name} ` +
    `(pane_closed on ${lead.name} pane ${lead.paneId}); ` +
    `store: no lead or team-supervisor pane in agent.list`;
}

// A pending emergency STOP nobody owns, cloned from the generation's own
// START row so every NOT NULL column carries a real value.
function pendingEmergencyStop(repo: string, room: string, hook: HookFixture, reason: string): void {
  for (const path of [join(repo, ".maestro", "maestro.db"), join(room, ".maestro", "maestro.db")]) {
    const database = new Database(path);
    database
      .query(
        `INSERT INTO slp_lifecycle_operations
         SELECT team_id, generation, 'STOP', 'RESERVED', 1, project_path, objective,
                configuration_json, pack_version, pack_digest, work_id, workspace_id,
                runtime_json, 'hub-supervisor', ?, 1, NULL, NULL, created_at, updated_at
         FROM slp_lifecycle_operations
         WHERE team_id = ? AND generation = ? AND operation = 'START'`,
      )
      .run(reason, hook.data.team.teamId, hook.data.team.generation);
    database.close();
  }
}

test("orphan-malformed-agent-list: a SUCCESSFUL agent.list whose agents field is absent, and one where it is not an array, each abort the orphan commit instead of reading as zero agents - both stores still RUNNING, no STOP row, no abandonment - while the ordinary pane-loss entry is still recorded per d874 (w717, F1, d875)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Malformed list");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);

    // Shape one: a 200-shaped answer with no agents field at all. Before
    // d875 this reached the predicate as [] - every seat unmatched, both
    // read dead, and a healthy team emergency-abandoned on a response
    // maestro simply did not understand.
    await setFakeHerdrBehavior(hook.fake, { malformedResults: { "agent.list": {} } });
    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    const first = envelope<{ aborted?: string; orphaned?: boolean; reason?: string }>(closed.stdout);
    expect(first.aborted).toBe("orphan-check");
    expect(first.orphaned).toBeUndefined();
    expect(first.reason).toContain("no agents field");

    // Shape two: a field of the wrong type, the shape a rename or a protocol
    // drift produces. checkProtocol only warns, so this is the layer that
    // has to notice.
    await setFakeHerdrBehavior(hook.fake, { malformedResults: { "agent.list": { agents: "none" } } });
    const exited = await paneEvent(fixture, hook, "pane_exited", supervisor.paneId);
    expect(exited.exitCode).toBe(0);
    const second = envelope<{ aborted?: string; reason?: string }>(exited.stdout);
    expect(second.aborted).toBe("orphan-check");
    expect(second.reason).toContain("agents as string");

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("RUNNING");
    expect(facts.roomState).toBe("RUNNING");
    expect(facts.stop).toBeNull();
    expect(facts.abandoned).toEqual([]);
    // d874 again: the malformed read costs the orphan conclusion, not the
    // record of the two panes Herdr told us had died.
    const entries = paneLossEntries(fixture.repo);
    expect(entries.map((entry) => entry.flag)).toEqual(["pane:closed", "pane:exited"]);
    expect(entries[0]?.body).toContain(`closed: ${lead.name} pane ${lead.paneId}`);
    expect(entries[1]?.body).toContain(`exited: ${supervisor.name} pane ${supervisor.paneId}`);
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

test("orphan-check-abort-under-a-live-runtime: a failed agent.list on a generation whose runtime holds the lock exits 0 reporting the abort instead of failing the hook, and the runtime still owns the pane-loss record (w717, F3, d875)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Runtime holds lock");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);
    // A subscribed runtime owns this generation: before w713 the hook
    // returned here without calling Herdr at all, so the predicate running on
    // this path is the new cost, and an uncaught throw would turn an exit-0
    // hook into a failing one. This pid is alive - it is the test's own.
    const directory = slpRuntimeDirectory(fixture.repo, hook.data.team.teamId, hook.data.team.generation);
    await mkdir(directory, { recursive: true });
    await writeFile(join(directory, "runtime.lock"), `${process.pid}\n`);
    try {
      expect(await runtimeLockHolder(directory)).toBe(process.pid);
      await setFakeHerdrBehavior(hook.fake, { failMethods: ["agent.list"] });

      const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
      expect(closed.exitCode).toBe(0);
      expect(envelope<{ aborted?: string; handled: boolean; runtime?: boolean }>(closed.stdout)).toMatchObject({
        aborted: "orphan-check",
        handled: false,
        runtime: true,
      });

      const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
      expect(facts.localState).toBe("RUNNING");
      expect(facts.roomState).toBe("RUNNING");
      expect(facts.stop).toBeNull();
      expect(facts.abandoned).toEqual([]);
      // Unchanged on this path: the hook does not write the entry when a
      // runtime is subscribed, because the runtime records it itself.
      expect(paneLossEntries(fixture.repo)).toEqual([]);
    } finally {
      await rm(join(directory, "runtime.lock"), { force: true });
    }
  });
}, 60_000);

test("orphan-repair-fenced-dead-owner: a committed START repair row whose owner pid is DEAD still fences the orphan commit - it reports wait, writes nothing and leaves the claim in place - while an interactive team stop --emergency on the same tree still clears that claim and stops, so only the automatic path became stricter (w717, F2, d875)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Repair crashed");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);
    // A pid that cannot be running, verified rather than assumed: macOS
    // caps pids well below this. The repairing process died mid-window -
    // and runtime.start closes and recreates each seat pane in turn, so
    // that window looks exactly like an orphan from agent.list.
    const deadPid = 999_999;
    expect(() => process.kill(deadPid, 0)).toThrow();
    for (const path of [join(fixture.repo, ".maestro", "maestro.db"), join(hook.room, ".maestro", "maestro.db")]) {
      const database = new Database(path);
      database
        .query(
          `UPDATE slp_lifecycle_operations SET owner_token = ?, owner_pid = ?
           WHERE team_id = ? AND generation = ? AND operation = 'START'`,
        )
        .run("repair-crashed", deadPid, hook.data.team.teamId, hook.data.team.generation);
      database.close();
    }

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    expect(envelope<{ orphaned: boolean; outcome: string }>(closed.stdout)).toMatchObject({
      orphaned: true,
      outcome: "wait",
    });

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("RUNNING");
    expect(facts.roomState).toBe("RUNNING");
    expect(facts.stop).toBeNull();
    expect(facts.abandoned).toEqual([]);
    // The claim is untouched: the automatic path waits, it does not take
    // the repair away from a `team start` that may still finish it.
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const claim = project
      .query<{ owner_token: string | null }, [string, number]>(
        `SELECT owner_token FROM slp_lifecycle_operations
         WHERE team_id = ? AND generation = ? AND operation = 'START'`,
      )
      .get(hook.data.team.teamId, hook.data.team.generation);
    project.close();
    expect(claim?.owner_token).toBe("repair-crashed");

    // The other half of d875: the INTERACTIVE stop is unchanged. A human at
    // the Hub has decided, so a dead claim is cleared and the stop proceeds.
    const stopped = await runCliAt(
      fixture,
      hook.room,
      ["team", "stop", hook.data.team.teamId, "--emergency", "--json"],
      hook.fake.env,
    );
    expect(stopped.exitCode).toBe(0);
    const after = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(after.localState).toBe("STOPPED");
    expect(after.roomState).toBe("STOPPED");
    expect(after.stop?.phase).toBe("COMMITTED");
    expect(after.stop?.actor).toBe("hub-supervisor");
  });
}, 60_000);

test("orphan-emergency-pinned: with a Hub emergency stop already pending under a different reason the hook reports outcome pinned and writes nothing, instead of letting EMERGENCY_REASON_CHANGED escape a per-event hook as a non-zero exit (w717, F4, d875)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Reason pinned");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);
    pendingEmergencyStop(fixture.repo, hook.room, hook, "hub: pulling the plug on this generation");

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    expect(envelope<{ orphaned: boolean; outcome: string }>(closed.stdout)).toMatchObject({
      orphaned: true,
      outcome: "pinned",
    });

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("RUNNING");
    expect(facts.roomState).toBe("RUNNING");
    expect(facts.abandoned).toEqual([]);
    // The pending reservation is left exactly as the Hub left it: someone
    // else has already decided how this generation dies.
    expect(facts.stop).toEqual({ actor: "hub-supervisor", emergency: 1, phase: "RESERVED" });
    expect(await tripwireInvocations(hook.fake)).toEqual([]);
  });
}, 60_000);

test("orphan-emergency-precedence: when the pending emergency reason is the orphan's own, the stop commits under the PENDING actor - abandoned_by and the stop row say hub-supervisor, not runtime-orphan - which is correct precedence and is pinned here so nobody later reads it as a bug (w717, F4, d875)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Precedence");
    const { lead, supervisor } = seats(hook);
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);
    const reason = orphanReasonForLead(hook);
    pendingEmergencyStop(fixture.repo, hook.room, hook, reason);

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    expect(envelope<{ orphaned: boolean; stopped: boolean }>(closed.stdout)).toMatchObject({
      orphaned: true,
      stopped: true,
    });

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("STOPPED");
    expect(facts.roomState).toBe("STOPPED");
    // reserveStop keeps the pending emergency's actor and reason rather than
    // overwriting them: an emergency already declared is not re-attributed by
    // whoever happens to finish it. The generation dies once, under the name
    // of whoever first said it should.
    expect(facts.stop).toEqual({ actor: "hub-supervisor", emergency: 1, phase: "COMMITTED" });
    expect(facts.unabandoned).toBe(0);
    expect(facts.abandoned.length).toBeGreaterThan(0);
    for (const item of facts.abandoned) expect(item.abandoned_by).toBe("hub-supervisor");
    // The d872 notice still names the mechanism that fired, which is the
    // Hub's only evidence that this was closed automatically.
    const notices = (await fakeHerdrCommands(hook.fake))
      .filter((command) => command[0] === "agent" && command[1] === "prompt")
      .filter((command) => (command[3] ?? "").includes("orphan auto-close"));
    expect(notices).toHaveLength(1);
    expect(notices[0]?.[3]).toContain("runtime-orphan");
  });
}, 60_000);

test("orphan-notice-counts-what-it-stamped: an item an earlier partial stop already abandoned is not counted again - the Hub notice and abandonedWorkCount report the rows this stop actually stamped, not the generation's unfinished total (w717, F5, d875)", async () => {
  await withFixture(async (fixture) => {
    const hook = await startForHook(fixture, "Count stamped");
    const { lead, supervisor } = seats(hook);
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer item", "--to", "peer-survivor", "--json"],
      { ...hook.fake.env, HERDR_PANE_ID: lead.paneId },
    );
    expect(added.exitCode).toBe(0);
    const peer = envelope<AddedPeer>(added.stdout);
    // An earlier partial stop already abandoned the Lead's item and then
    // failed before it could transition the team.
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    project
      .query(
        `UPDATE slp_work SET abandoned_at = ?, abandoned_by = ?, abandonment_reason = ?
         WHERE id = ?`,
      )
      .run("2026-09-01T00:00:00.000Z", "hub-supervisor", "earlier partial stop", hook.data.work.id);
    project.close();
    await dropPanes(hook, [supervisor.paneId, lead.paneId]);

    const closed = await paneEvent(fixture, hook, "pane_closed", lead.paneId);
    expect(closed.exitCode).toBe(0);
    expect(envelope<{ abandonedWorkCount: number }>(closed.stdout).abandonedWorkCount).toBe(1);

    const facts = stopFacts(fixture.repo, hook.room, hook.data.team);
    expect(facts.localState).toBe("STOPPED");
    expect(facts.unabandoned).toBe(0);
    // Two rows carry abandonment, but only one of them was stamped here, and
    // the earlier one keeps its own actor and reason.
    expect(facts.abandoned.length).toBe(2);
    const earlier = facts.abandoned.find((item) => item.id === hook.data.work.id);
    expect(earlier?.abandonment_reason).toBe("earlier partial stop");
    const stamped = facts.abandoned.find((item) => item.id === peer.work.id);
    expect(stamped?.abandoned_by).toBe("runtime-orphan");
    const notices = (await fakeHerdrCommands(hook.fake))
      .filter((command) => command[0] === "agent" && command[1] === "prompt")
      .filter((command) => (command[3] ?? "").includes("orphan auto-close"));
    expect(notices[0]?.[3]).toContain("1 unfinished work item abandoned");
  });
}, 60_000);
