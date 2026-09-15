import { expect, test } from "bun:test";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { installFakeHerdr, readFakeHerdrState } from "./helpers-herdr.ts";
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
