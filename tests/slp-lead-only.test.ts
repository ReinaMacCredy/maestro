import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { slpRuntimeDirectory } from "../src/plugins/slp-process.ts";
import {
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
} from "./helpers-herdr.ts";
import { runCliAt, withFixture, type Fixture } from "./helpers.ts";

// room d117 (store at ~/maestro): `maestro team start` gains a lead-only shape
// that opens exactly one Lead pane plus the team runtime pane and no Team
// Supervisor, and in that shape the Hub Supervisor is the Lead's reviewer - it
// receives the RETURNED push, may read the item, and runs work accept or
// work note --rework. d72's "Team Supervisor accepts Lead work" and "Hub
// Supervisor never manages Lead directly" continue to govern every team that
// HAS a Team Supervisor, so both shapes are asserted here side by side: the
// change must not loosen the supervised boundary it is carving an exception to.

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

interface Started {
  data: StartedTeam;
  fake: Awaited<ReturnType<typeof installFakeHerdr>>;
  room: string;
}

async function startTeam(fixture: Fixture, leadOnly: boolean): Promise<Started> {
  const room = await markedRoom(fixture);
  const fake = await installFakeHerdr(fixture, { runtimePane: "spawn" });
  const argv = ["team", "start", fixture.repo, "Shape check", "--json"];
  if (leadOnly) argv.splice(4, 0, "--lead-only");
  const started = await runCliAt(fixture, room, argv, fake.env);
  expect(started.stderr).not.toContain("error");
  expect(started.exitCode).toBe(0);
  return { data: envelope<StartedTeam>(started.stdout), fake, room };
}

test("lead-only: team start --lead-only opens one Lead pane and the runtime pane and no Team Supervisor, while the default shape still opens both seats (room d117)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startTeam(fixture, true);
    const roles = data.team.roles.map((role) => role.role);
    expect(roles).toContain("lead");
    expect(roles).not.toContain("team-supervisor");
    expect(roles.filter((role) => role === "lead")).toHaveLength(1);

    // d97: the runtime pane is the model-free attention layer and stays in
    // both shapes; a lead-only team without it would have no stall detection.
    expect(data.team.runtimePaneId).not.toBe("");

    // The Herdr side agrees: no supervisor agent was ever spawned.
    const agents = (await readFakeHerdrState(fake)).agents as Array<
      { name: string; workspace_id: string }
    >;
    const inTeam = agents.filter((agent) => agent.workspace_id === data.team.workspaceId);
    expect(inTeam.some((agent) => agent.name === `lead-${data.team.teamId}`)).toBe(true);
    expect(inTeam.some((agent) => agent.name === `supervisor-${data.team.teamId}`)).toBe(false);
  });
});

test("lead-only: the default shape is unchanged and still opens a Team Supervisor alongside the Lead (d72)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startTeam(fixture, false);
    const roles = data.team.roles.map((role) => role.role);
    expect(roles).toContain("lead");
    expect(roles).toContain("team-supervisor");

    const agents = (await readFakeHerdrState(fake)).agents as Array<
      { name: string; workspace_id: string }
    >;
    const inTeam = agents.filter((agent) => agent.workspace_id === data.team.workspaceId);
    expect(inTeam.some((agent) => agent.name === `supervisor-${data.team.teamId}`)).toBe(true);
  });
});

test("lead-only: a Lead's returned work waits on hub-supervisor, the Hub reads it and accepts it, and no seat inside the team can accept it (room d117)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;

    // room d118: the Lead takes and works its own item rather than opening a Peer.
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);
    const returned = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", workId, "done, with the verify story"],
      leadEnvironment,
    );
    expect(returned.exitCode).toBe(0);

    // The Lead's own status names the Hub as the reviewer it is waiting on -
    // not a team-supervisor pane, which does not exist in this shape.
    const leadStatus = await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment);
    expect(leadStatus.exitCode).toBe(0);
    expect(leadStatus.stdout).toContain("waiting on hub-supervisor");

    // The Lead cannot review itself: independent acceptance still holds at the
    // Lead boundary, it has simply moved up to the Hub.
    const selfAccept = await runCliAt(fixture, fixture.repo, ["work", "accept", workId], leadEnvironment);
    expect(selfAccept.exitCode).not.toBe(0);
    expect(`${selfAccept.stdout}${selfAccept.stderr}`).toContain("lead-only");

    // The Hub reads the item from ~/maestro, which d72 alone would refuse.
    const hubRead = await runCliAt(fixture, room, ["status", workId], fake.env);
    expect(hubRead.exitCode).toBe(0);
    expect(hubRead.stdout).toContain(workId);
    expect(hubRead.stdout).toContain("RETURNED");

    // And accepts it.
    const accepted = await runCliAt(fixture, room, ["work", "accept", workId, "--json"], fake.env);
    expect(accepted.exitCode).toBe(0);
    const after = envelope<{ work: { acceptedBy: string; state: string } }>(accepted.stdout);
    expect(after.work.state).toBe("DONE");
    expect(after.work.acceptedBy).toBe("hub-supervisor");
  });
});

test("lead-only: the Hub grants rework on a lead-only team's returned work, and the Lead may retake it once (room d117)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;

    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "return", workId, "first pass"], leadEnvironment))
        .exitCode,
    ).toBe(0);

    const rework = await runCliAt(
      fixture,
      room,
      ["work", "note", workId, "the verify story is missing", "--rework"],
      fake.env,
    );
    expect(rework.exitCode).toBe(0);
    expect(rework.stdout).toContain("rework grant");

    // The grant is what lets the assignee retake RETURNED work exactly once.
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);
  });
});

test("lead-only: a supervised team still refuses the Hub Supervisor and is still accepted by its Team Supervisor (d72 survives room d117)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, false);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;

    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "return", workId, "done"], leadEnvironment)).exitCode,
    ).toBe(0);

    // The exception room d117 carves is exactly one shape wide.
    const hubAccept = await runCliAt(fixture, room, ["work", "accept", workId], fake.env);
    expect(hubAccept.exitCode).not.toBe(0);
    expect(`${hubAccept.stdout}${hubAccept.stderr}`).toContain("Team Supervisor");

    const hubRead = await runCliAt(fixture, room, ["status", workId], fake.env);
    expect(hubRead.exitCode).not.toBe(0);

    const accepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", workId, "--json"],
      { ...fake.env, HERDR_PANE_ID: supervisor.paneId },
    );
    expect(accepted.exitCode).toBe(0);
    const after = envelope<{ work: { acceptedBy: string; state: string } }>(accepted.stdout);
    expect(after.work.state).toBe("DONE");
    expect(after.work.acceptedBy).toBe(supervisor.name);
  });
});

test("lead-only: a Lead's blocked note escalates to the Hub rather than to a Team Supervisor pane that does not exist (d761 under room d117)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;

    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);
    const blocked = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", workId, "which decision ids govern this?", "--blocked"],
      leadEnvironment,
    );
    expect(blocked.exitCode).toBe(0);

    // The escalation is only real if the seat above can see it; the Hub is
    // that seat here, and it reads the blocked note back off the record.
    const hubRead = await runCliAt(fixture, room, ["status", workId], fake.env);
    expect(hubRead.exitCode).toBe(0);
    expect(hubRead.stdout).toContain("blocked");
    expect(hubRead.stdout).toContain("which decision ids govern this?");
  });
});

test("lead-only: the shape is pinned for the generation, so re-running team start without the flag does not reshape a live lead-only team (room d117)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, true);

    const again = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Shape check", "--json"],
      fake.env,
    );
    expect(again.exitCode).toBe(0);
    const restored = envelope<StartedTeam>(again.stdout);
    expect(restored.team.generation).toBe(data.team.generation);
    expect(restored.team.roles.map((role) => role.role)).not.toContain("team-supervisor");

    const agents = (await readFakeHerdrState(fake)).agents as Array<
      { name: string; workspace_id: string }
    >;
    expect(
      agents
        .filter((agent) => agent.workspace_id === data.team.workspaceId)
        .some((agent) => agent.name === `supervisor-${data.team.teamId}`),
    ).toBe(false);
  });
});

test("lead-only: the pinned shape is readable from the generation's own configuration and defaults to supervised for a row written before the shape existed (room d117)", async () => {
  await withFixture(async (fixture) => {
    const { data } = await startTeam(fixture, true);
    const { Database } = await import("bun:sqlite");
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    try {
      const row = database
        .query<{ configuration_json: string }, [string, number]>(
          `SELECT configuration_json FROM slp_local_teams WHERE team_id = ? AND generation = ?`,
        )
        .get(data.team.teamId, data.team.generation);
      expect(row).not.toBeNull();
      const configuration = JSON.parse(row!.configuration_json) as {
        profileDigests: Record<string, string>;
        shape?: string;
      };
      expect(configuration.shape).toBe("lead-only");
      // A lead-only generation never launches the Team Supervisor, so it must
      // not pin that profile's bytes either - otherwise an edit to a profile
      // this team never uses would trip requireProfilesUnchanged on it.
      expect(Object.keys(configuration.profileDigests)).not.toContain("team-supervisor");
    } finally {
      database.close();
    }
  });
});

test("lead-only: room d117 moves only the boundary above the Lead - a Peer's work in a lead-only team is still reviewed by the Lead, and the Hub is refused without being told the team has a Team Supervisor it does not have (room d117)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };

    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "A genuinely parallel part", "--to", "peer-parallel", "--json"],
      leadEnvironment,
    );
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(
      added.stdout,
    );
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id], peerEnvironment)).exitCode)
      .toBe(0);
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "return", peer.work.id, "done"], peerEnvironment))
        .exitCode,
    ).toBe(0);

    // The Hub reviews the Lead, not the Lead's Peers.
    const hubAccept = await runCliAt(fixture, room, ["work", "accept", peer.work.id], fake.env);
    expect(hubAccept.exitCode).not.toBe(0);
    const refusal = `${hubAccept.stdout}${hubAccept.stderr}`;
    expect(refusal).toContain("lead");
    // The old message asserted a Team Supervisor exists; in this shape none does.
    expect(refusal).not.toContain("this generation has a Team Supervisor");

    // The Lead still accepts its Peer's work, exactly as in a supervised team.
    const accepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", peer.work.id, "--json"],
      leadEnvironment,
    );
    expect(accepted.exitCode).toBe(0);
    expect(envelope<{ work: { state: string } }>(accepted.stdout).work.state).toBe("DONE");
  });
});

// w722 DEFECT 1: a lead-only generation has no Team Supervisor, so nothing
// could run the normal stop; the Hub Supervisor, which room d117 makes the
// seat above the Lead, was left with --emergency as its only way to close a
// finished team. From ~/maestro, `team stop <id>` without --emergency now
// performs the Team Supervisor's normal stop for that shape: the same phases,
// the same non-emergency STOPPED recording, refused while work is unfinished.

async function finishLeadWork(
  fixture: Fixture,
  started: Started,
): Promise<void> {
  const lead = started.data.team.roles.find((role) => role.role === "lead")!;
  const leadEnvironment = { ...started.fake.env, HERDR_PANE_ID: lead.paneId };
  const workId = started.data.work.id;
  expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
    .toBe(0);
  expect(
    (await runCliAt(fixture, fixture.repo, ["work", "return", workId, "done"], leadEnvironment))
      .exitCode,
  ).toBe(0);
}

// The runtime pane child may still be releasing the project store when the
// stop returns, so a read-only assertion waits on the lock instead of
// reporting SQLITE_BUSY as a test failure.
function openReadonly(path: string): Database {
  const database = new Database(path, { readonly: true });
  database.exec("PRAGMA busy_timeout = 5000");
  return database;
}

function localTeamState(fixture: Fixture, teamId: string, generation: number): string | undefined {
  const database = openReadonly(join(fixture.repo, ".maestro", "maestro.db"));
  try {
    return database
      .query<{ state: string }, [string, number]>(
        "SELECT state FROM slp_local_teams WHERE team_id = ? AND generation = ?",
      )
      .get(teamId, generation)?.state;
  } finally {
    database.close();
  }
}

test("lead-only: the Hub Supervisor closes a finished lead-only team with a normal team stop, committed STOPPED without emergency and with the supervisor's closing-report semantics (w722, room d117)", async () => {
  await withFixture(async (fixture) => {
    const started = await startTeam(fixture, true);
    const { data, fake, room } = started;
    await finishLeadWork(fixture, started);
    expect((await runCliAt(fixture, room, ["work", "accept", data.work.id], fake.env)).exitCode).toBe(0);

    const reason = "all green: the one item is accepted";
    const stopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--reason", reason, "--json"],
      fake.env,
    );
    expect(stopped.exitCode).toBe(0);
    const result = envelope<{ emergency: boolean; team: { state: string } }>(stopped.stdout);
    expect(result.emergency).toBe(false);
    expect(result.team.state).toBe("STOPPED");
    expect(localTeamState(fixture, data.team.teamId, data.team.generation)).toBe("STOPPED");

    // The committed STOP row is the normal one: the Hub Supervisor as actor,
    // emergency 0, the closing report as its reason - what a Team Supervisor's
    // normal stop records in a supervised team, less the seat that is absent.
    const hub = openReadonly(join(room, ".maestro", "maestro.db"));
    try {
      expect(
        hub
          .query<{ actor: string; emergency: number; phase: string; reason: string }, [string]>(
            `SELECT actor, emergency, phase, reason FROM slp_lifecycle_operations
             WHERE operation = 'STOP' AND team_id = ?`,
          )
          .get(data.team.teamId),
      ).toEqual({ actor: "hub-supervisor", emergency: 0, phase: "COMMITTED", reason });
      expect(
        hub
          .query<{ operation: string }, [string]>(
            "SELECT operation FROM slp_activity WHERE team_id = ? AND target_type = 'team'",
          )
          .all(data.team.teamId)
          .map((row) => row.operation),
      ).toEqual(["team.start", "team.stop"]);
    } finally {
      hub.close();
    }

    // Nothing was abandoned: this was not an emergency.
    const project = openReadonly(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      expect(
        project
          .query<{ abandoned_at: string | null }, [string]>(
            "SELECT abandoned_at FROM slp_work WHERE id = ?",
          )
          .get(data.work.id)?.abandoned_at,
      ).toBeNull();
    } finally {
      project.close();
    }

    const status = await runCliAt(fixture, room, ["status"], fake.env);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain(`STOPPED (supervisor): ${reason}`);
    expect(status.stdout).not.toContain("(emergency)");
  });
});

test("lead-only: the Hub's normal stop is refused while the team holds unfinished work, exactly as the supervised normal stop is, and --emergency keeps its meaning (w722)", async () => {
  await withFixture(async (fixture) => {
    const started = await startTeam(fixture, true);
    const { data, fake, room } = started;
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", data.work.id], leadEnvironment)).exitCode)
      .toBe(0);

    const refused = await runCliAt(fixture, room, ["team", "stop", data.team.teamId, "--json"], fake.env);
    expect(refused.exitCode).toBe(1);
    expect(refused.stderr).toContain("TEAM_UNFINISHED");
    expect(refused.stderr).toContain(`${data.work.id} [ACTIVE]`);
    expect(localTeamState(fixture, data.team.teamId, data.team.generation)).toBe("RUNNING");

    // The refusal wrote no STOP row: the next stop starts clean.
    const hub = openReadonly(join(room, ".maestro", "maestro.db"));
    try {
      expect(
        hub
          .query<{ count: number }, [string]>(
            "SELECT count(*) AS count FROM slp_lifecycle_operations WHERE operation = 'STOP' AND team_id = ?",
          )
          .get(data.team.teamId)?.count,
      ).toBe(0);
    } finally {
      hub.close();
    }

    const emergency = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--emergency", "--reason", "abandoning w722 check", "--json"],
      fake.env,
    );
    expect(emergency.exitCode).toBe(0);
    expect(envelope<{ emergency: boolean; team: { state: string } }>(emergency.stdout)).toMatchObject({
      emergency: true,
      team: { state: "STOPPED" },
    });
    const stopped = openReadonly(join(room, ".maestro", "maestro.db"));
    try {
      expect(
        stopped
          .query<{ actor: string; emergency: number; phase: string }, [string]>(
            "SELECT actor, emergency, phase FROM slp_lifecycle_operations WHERE operation = 'STOP' AND team_id = ?",
          )
          .get(data.team.teamId),
      ).toEqual({ actor: "hub-supervisor", emergency: 1, phase: "COMMITTED" });
    } finally {
      stopped.close();
    }
    const project = openReadonly(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      expect(
        project
          .query<{ abandonment_reason: string | null }, [string]>(
            "SELECT abandonment_reason FROM slp_work WHERE id = ?",
          )
          .get(data.work.id)?.abandonment_reason,
      ).toBe("abandoning w722 check");
    } finally {
      project.close();
    }
  });
});

test("lead-only: a supervised team's Hub stop stays --emergency only, even once its work is DONE (d72 survives w722)", async () => {
  await withFixture(async (fixture) => {
    const started = await startTeam(fixture, false);
    const { data, fake, room } = started;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    await finishLeadWork(fixture, started);
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "accept", data.work.id], {
        ...fake.env,
        HERDR_PANE_ID: supervisor.paneId,
      })).exitCode,
    ).toBe(0);

    const refused = await runCliAt(fixture, room, ["team", "stop", data.team.teamId, "--json"], fake.env);
    expect(refused.exitCode).toBe(1);
    expect(refused.stderr).toContain("ROLE_FORBIDDEN");
    expect(refused.stderr).toContain("Team Supervisor");
    expect(localTeamState(fixture, data.team.teamId, data.team.generation)).toBe("RUNNING");

    // --reason without --emergency is still not the Hub's option in this shape.
    const reasoned = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--reason", "not the Hub's call", "--json"],
      fake.env,
    );
    expect(reasoned.exitCode).toBe(1);
    expect(reasoned.stderr).toContain("INVALID_OPTION");
  });
});

// w722 DEFECT 2: in a lead-only team no Team Supervisor pane keeps the
// workspace alive, so Herdr auto-closes it when the last seat tab closes
// (herdr-server.log 2026-09-15T17:24:23: three tab.close, then
// workspace.close), and the remaining-pane check that follows saw
// workspace_not_found before the store commit. The panes were dead, the store
// still said RUNNING, and status recommended the very command that had just
// failed. A workspace that disappears during teardown is a workspace that is
// torn down: the stop proceeds to its commit on the first call.
test("lead-only: a workspace that auto-closes with its last tab lets team stop commit STOPPED on the first call, transcript cleaned (w722)", async () => {
  await withFixture(async (fixture) => {
    const started = await startTeam(fixture, true);
    const { data, fake, room } = started;
    await finishLeadWork(fixture, started);
    expect((await runCliAt(fixture, room, ["work", "accept", data.work.id], fake.env)).exitCode).toBe(0);

    const runtimeDirectory = slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation);
    await mkdir(runtimeDirectory, { recursive: true });
    await writeFile(join(runtimeDirectory, "state.json"), "{}\n");
    await setFakeHerdrBehavior(fake, { closeWorkspaceWithLastTab: true });
    const commandsBefore = (await fakeHerdrCommands(fake)).length;

    const stopped = await runCliAt(fixture, room, ["team", "stop", data.team.teamId, "--json"], fake.env);
    expect(stopped.exitCode, stopped.stderr).toBe(0);
    expect(envelope<{ emergency: boolean; team: { state: string } }>(stopped.stdout)).toMatchObject({
      emergency: false,
      team: { state: "STOPPED" },
    });
    expect(localTeamState(fixture, data.team.teamId, data.team.generation)).toBe("STOPPED");
    expect(existsSync(runtimeDirectory)).toBe(false);

    // The workspace really did vanish under the stop, and the stop noticed
    // rather than assuming: it asked Herdr, got workspace_not_found, and went on.
    const after = await readFakeHerdrState(fake);
    expect(
      (after.workspaces as Array<{ workspace_id: string }>)
        .some((workspace) => workspace.workspace_id === data.team.workspaceId),
    ).toBe(false);
    const stopCommands = (await fakeHerdrCommands(fake)).slice(commandsBefore);
    expect(stopCommands.filter((command) => command[0] === "tab" && command[1] === "close").length)
      .toBeGreaterThan(0);

    const status = await runCliAt(fixture, room, ["status"], fake.env);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain("STOPPED");
    expect(status.stdout).not.toContain("ORPHANED?");
  });
});

// room d118 mechanism (w725 Finding A): a Lead opens its own item with
// `work add` and no --to. The row is the same one team start writes for Lead
// work - the Lead is its own assignee, no Peer pane is opened, nothing is
// pushed because there is no counterpart to wake - so its reviewer falls out
// of the existing boundary: the Hub in this shape, the Team Supervisor in the
// supervised one (asserted in tests/slp-v2.test.ts).
test("lead-only: a Lead opens its own item with work add and no --to, takes it, and the Hub reviews it; a Peer still cannot add work (room d118, w725)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const promptsBefore = (await fakeHerdrCommands(fake))
      .filter((command) => command[0] === "agent" && command[1] === "prompt").length;

    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "A follow-up the Lead will do itself", "--json"],
      leadEnvironment,
    );
    expect(added.stderr).not.toContain("error");
    expect(added.exitCode).toBe(0);
    const own = envelope<{
      role: { name: string; paneId: string; role: string };
      work: { assignedTo: string; createdBy: string; id: string; owner: string | null; state: string };
    }>(added.stdout);
    expect(own.work).toMatchObject({ assignedTo: lead.name, createdBy: lead.name, owner: null, state: "OPEN" });
    expect(own.role).toMatchObject({ name: lead.name, paneId: lead.paneId, role: "lead" });
    // No Peer pane and no wake line: the Lead is the only party to this item.
    const promptsAfter = (await fakeHerdrCommands(fake))
      .filter((command) => command[0] === "agent" && command[1] === "prompt").length;
    expect(promptsAfter).toBe(promptsBefore);
    expect(JSON.stringify(await readFakeHerdrState(fake))).not.toContain(":peer:");

    // The list marks it as waiting on the Lead while OPEN.
    const listed = await runCliAt(fixture, fixture.repo, ["status"], leadEnvironment);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain(`* ${own.work.id} OPEN`);

    expect((await runCliAt(fixture, fixture.repo, ["work", "take", own.work.id], leadEnvironment)).exitCode)
      .toBe(0);
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "return", own.work.id, "done"], leadEnvironment))
        .exitCode,
    ).toBe(0);
    const waiting = await runCliAt(fixture, fixture.repo, ["status", own.work.id], leadEnvironment);
    expect(waiting.stdout).toContain("waiting on hub-supervisor");
    const accepted = await runCliAt(fixture, room, ["work", "accept", own.work.id, "--json"], fake.env);
    expect(accepted.exitCode).toBe(0);
    expect(envelope<{ work: { acceptedBy: string; state: string } }>(accepted.stdout).work)
      .toMatchObject({ acceptedBy: "hub-supervisor", state: "DONE" });

    // A Peer has no work add, with or without --to.
    const peerAdded = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "A genuinely parallel part", "--to", "peer-parallel", "--json"],
      leadEnvironment,
    );
    expect(peerAdded.exitCode).toBe(0);
    const peer = envelope<{ role: { paneId: string } }>(peerAdded.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    const refused = await runCliAt(fixture, fixture.repo, ["work", "add", "Peer self-work"], peerEnvironment);
    expect(refused.exitCode).not.toBe(0);
    expect(`${refused.stdout}${refused.stderr}`).toContain("ROLE_FORBIDDEN");
  });
});

// room d129 point 4: the natural capture points are recorded, never inferred
// (d705). A reviewer's `work note --rework` and a decision recorded with
// --replaces that also names --work mark that item as carrying a lesson
// candidate; a stored row, not a reading of the text. `maestro status
// <work-id>` prints the mark and its next: line reminds the holder that the
// return must state the lesson it filed or why none applies.
test("665 a rework grant marks its item as carrying a lesson candidate, and status reminds the holder and the reviewer (room d129 point 4)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;

    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);

    // Before the correction there is nothing to capture.
    const clean = await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment);
    expect(clean.exitCode).toBe(0);
    expect(clean.stdout).not.toContain("lesson candidate");

    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "return", workId, "first pass"], leadEnvironment))
        .exitCode,
    ).toBe(0);
    const rework = await runCliAt(
      fixture,
      room,
      ["work", "note", workId, "the verify story is missing", "--rework"],
      fake.env,
    );
    expect(rework.exitCode).toBe(0);

    // The reviewer's own read carries the mark and the acceptance condition.
    const reviewerRead = await runCliAt(fixture, room, ["status", workId], fake.env);
    expect(reviewerRead.exitCode).toBe(0);
    expect(reviewerRead.stdout).toContain("lesson candidate: yes, from rework note on revision 1");
    expect(reviewerRead.stdout).toContain(
      "accept only after the return states the lesson it filed or why none applies",
    );

    const holderJson = await runCliAt(fixture, fixture.repo, ["status", workId, "--json"], leadEnvironment);
    expect(holderJson.exitCode).toBe(0);
    expect(envelope<{ lessonCandidate: { sources: string[] } | null }>(holderJson.stdout).lessonCandidate)
      .toEqual({ sources: ["rework note on revision 1"] });
    const holderRead = await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment);
    expect(holderRead.exitCode).toBe(0);
    expect(holderRead.stdout).toContain("lesson candidate: yes, from rework note on revision 1");
    expect(holderRead.stdout).toContain(
      "this item carries a lesson candidate: state in the return the lesson you filed (maestro lesson file) or why none applies",
    );
  });
});

test("666 a team decision with --replaces and --work marks that item, and one without --work marks nothing (room d129 point 4)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;

    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);
    const first = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "keep the adapter", "--why", "it is the cheapest seam", "--json"],
      leadEnvironment,
    );
    expect(first.exitCode).toBe(0);
    const firstId = envelope<{ decision: { id: string } }>(first.stdout).decision.id;

    // A plain decision on the item is not a correction and marks nothing.
    const plain = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "name the seam", "--why", "readers ask", "--work", workId],
      leadEnvironment,
    );
    expect(plain.exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment)).stdout)
      .not.toContain("lesson candidate");

    // A replacement without --work has no item to mark.
    const unlinked = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "drop the adapter", "--why", "the seam moved", "--replaces", firstId],
      leadEnvironment,
    );
    expect(unlinked.exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment)).stdout)
      .not.toContain("lesson candidate");

    const second = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "use the port", "--why", "the adapter leaked", "--replaces", firstId, "--work", workId, "--json"],
      leadEnvironment,
    );
    expect(second.exitCode).toBe(0);
    const secondId = envelope<{ decision: { id: string } }>(second.stdout).decision.id;

    const read = await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment);
    expect(read.exitCode).toBe(0);
    expect(read.stdout).toContain(`lesson candidate: yes, from decision ${secondId}`);
  });
});

// room d129 point 4, the Hub half: a Hub ruling is recorded in the Hub store,
// but the item it corrects lives in the project store, so the mark is written
// there or it reaches nobody.
test("667 a Hub decision with --replaces on a lead-only team's work marks that item in the project store (room d129 point 4)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;

    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);
    const first = await runCliAt(
      fixture,
      room,
      ["decide", "wait for the owner", "--why", "the fork is hers", "--work", workId, "--json"],
      fake.env,
    );
    expect(first.exitCode).toBe(0);
    const firstId = envelope<{ decision: { id: string } }>(first.stdout).decision.id;
    expect((await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment)).stdout)
      .not.toContain("lesson candidate");

    const replacement = await runCliAt(
      fixture,
      room,
      [
        "decide",
        "proceed without her",
        "--why",
        "she delegated it",
        "--replaces",
        firstId,
        "--work",
        workId,
        "--json",
      ],
      fake.env,
    );
    expect(replacement.exitCode).toBe(0);
    const replacementId = envelope<{ decision: { id: string } }>(replacement.stdout).decision.id;

    const read = await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment);
    expect(read.exitCode).toBe(0);
    expect(read.stdout).toContain(`lesson candidate: yes, from decision ${replacementId}`);
    expect(read.stdout).toContain(
      "this item carries a lesson candidate: state in the return the lesson you filed",
    );
  });
});

// w736 F3: after a team stop the Hub could no longer read the items it had
// just accepted - `maestro status w732` from ~/maestro answered "SLP work not
// found from Hub", because the resolver only ever looked at RUNNING teams. The
// record is the point of the record, and reading it changes nothing; the write
// verbs still refuse, and now they say why instead of pretending the id does
// not exist.
test("w736 a stopped generation's work is still readable from the Hub, while every write verb refuses it by naming the team's state", async () => {
  await withFixture(async (fixture) => {
    const started = await startTeam(fixture, true);
    const { data, fake, room } = started;
    const workId = data.work.id;
    await finishLeadWork(fixture, started);
    expect((await runCliAt(fixture, room, ["work", "accept", workId], fake.env)).exitCode).toBe(0);
    const stopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--reason", "one item accepted", "--json"],
      fake.env,
    );
    expect(stopped.exitCode).toBe(0);
    expect(localTeamState(fixture, data.team.teamId, data.team.generation)).toBe("STOPPED");

    // The read still answers, with the same detail it answered while running.
    const read = await runCliAt(fixture, room, ["status", workId], fake.env);
    expect(read.exitCode).toBe(0);
    expect(read.stdout).toContain(`${workId} DONE`);
    expect(read.stdout).toContain("acceptance by hub-supervisor: accepted");
    expect(read.stdout).toContain("next: none (accepted)");

    // Qualifying the id by team reads the same row.
    const qualified = await runCliAt(
      fixture,
      room,
      ["status", `${data.team.teamId}:${workId}`],
      fake.env,
    );
    expect(qualified.exitCode).toBe(0);
    expect(qualified.stdout).toContain(`${workId} DONE`);

    // Writes are refused, and the refusal names the state rather than claiming
    // the id is unknown - the message that sent the Hub to the wrong table.
    for (const argv of [
      ["work", "accept", workId],
      ["work", "note", workId, "one more thing", "--rework"],
      ["decide", "A late ruling", "--why", "because", "--work", workId],
    ]) {
      const refused = await runCliAt(fixture, room, [...argv, "--json"], fake.env);
      expect(refused.exitCode).not.toBe(0);
      const message = `${refused.stdout}${refused.stderr}`;
      expect(message).toContain("STOPPED");
      expect(message).toContain(data.team.teamId);
      expect(message).not.toContain("SLP work not found from Hub");
    }

    // An id no team ever held is still simply not found.
    const unknown = await runCliAt(fixture, room, ["status", "w9999"], fake.env);
    expect(unknown.exitCode).not.toBe(0);
    expect(`${unknown.stdout}${unknown.stderr}`).toContain("SLP work not found from Hub: w9999");
  });
});

// w736 F1: the Hub is the reviewer of a lead-only team's Lead item, and a
// reviewer answer is not always a ruling. A fact, a commit hash, a pointer to
// a decision has no `maestro decide` shape, and the Hub holds no other channel
// down into the team. `maestro work note <id> "<text>"` from the room answered
// NOT_FOUND from the non-SLP table, which sent the reader to `maestro work
// list` and the wrong store entirely.
test("w736 the Hub records a plain note on a lead-only team's Lead item, wakes the Lead with it, and is still refused --blocked and --owner", async () => {
  await withFixture(async (fixture) => {
    const started = await startTeam(fixture, true);
    const { data, fake, room } = started;
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);

    const body = "the g12 head is 50ef3cdf; d141 is the grant, not a new rule";
    const noted = await runCliAt(fixture, room, ["work", "note", workId, body, "--json"], fake.env);
    expect(noted.exitCode).toBe(0);
    expect(noted.stdout).toContain("hub-supervisor");

    // The Lead reads it off the record, and the item is untouched: a note is
    // not a state change and the Hub is not the assignee.
    const leadRead = await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment);
    expect(leadRead.exitCode).toBe(0);
    expect(leadRead.stdout).toContain(body);
    expect(leadRead.stdout).toContain(`${workId} ACTIVE`);

    // And it is a wake-up, not a message left on a shelf: the Hub has no other
    // way to reach the Lead between one operation and the next.
    const pushed = (await fakeHerdrCommands(fake))
      .filter((command) => command[0] === "agent" && command[1] === "prompt")
      .map((command) => command[3] ?? "")
      .filter((line) => line.startsWith(`[from hub-supervisor][${workId} NOTE]`));
    expect(pushed).toHaveLength(1);
    expect(pushed[0]).toContain(body);
    expect(pushed[0]).toContain(`read: maestro status ${workId}`);

    // The Hub has no seat above it, so it cannot escalate to itself; and an
    // owner walk-in in the Hub pane is a Hub ruling, not a seat's provenance
    // note on somebody else's item.
    for (const flag of ["--blocked", "--owner"]) {
      const refused = await runCliAt(
        fixture,
        room,
        ["work", "note", workId, "why?", flag, "--json"],
        fake.env,
      );
      expect(refused.exitCode).not.toBe(0);
      const message = `${refused.stdout}${refused.stderr}`;
      expect(message).toContain("ROLE_FORBIDDEN");
      expect(message).not.toContain("run: maestro work list");
    }

    // An id no SLP team holds answers from the SLP resolver, not from the room's
    // own work table, which is the sentence that sent the Hub to the wrong place.
    const unknown = await runCliAt(fixture, room, ["work", "note", "w9999", "hello", "--json"], fake.env);
    expect(unknown.exitCode).not.toBe(0);
    expect(`${unknown.stdout}${unknown.stderr}`).toContain("SLP work not found from Hub: w9999");

    // The room's own work items keep their own note verb.
    const own = await runCliAt(fixture, room, ["work", "add", "A Hub-local reminder", "--json"], fake.env);
    expect(own.exitCode).toBe(0);
    const ownId = (JSON.parse(own.stdout) as { data: { work: { id: string } } }).data.work.id;
    const ownNote = await runCliAt(fixture, room, ["work", "note", ownId, "still local", "--json"], fake.env);
    expect(ownNote.exitCode).toBe(0);
    expect(ownNote.stdout).toContain("still local");
  });
});
