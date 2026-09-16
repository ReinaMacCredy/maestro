import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { profileDigest, profileDirectories, renderedProfilePath, resolveProfile } from "../src/plugins/profiles.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { fakeHerdrCommands, installFakeHerdr } from "./helpers-herdr.ts";
import { runCliAt, withFixture, type Fixture } from "./helpers.ts";

// room d128 (w728): a seat's model is a launch-time choice. team start gains
// --lead-model, --peer-model and --supervisor-model, recorded on the team row
// like --peer-profile; work add --to gains --model for one Peer. The model
// rides the launch line after --agent/--profile the way --autocompact does, so
// the rendered profile, its digest and the pinned pack are untouched. The
// frontmatter model stays the default a generation uses when no flag is given.

function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

function failure(stderr: string): { code: string; message: string } {
  return (JSON.parse(stderr) as { error: { code: string; message: string } }).error;
}

const runtimePhaseLine =
  /^\S+: (?:starting (?:claude|codex) pane in \S+|waiting for acknowledgement \(up to \d+s\)|ready in \d+s|already (?:acknowledged|running) in \S+; left alone)$/;

function phaseFree(stderr: string): string[] {
  return stderr.split("\n").filter((line) => line !== "" && !runtimePhaseLine.test(line));
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

interface StartedTeam {
  team: { generation: number; roles: Array<{ name: string; paneId: string; profile: string; role: string }>; teamId: string };
  work: { id: string };
}

function startsOf(commands: string[][]): Map<string, string[]> {
  return new Map(
    commands
      .filter((command) => command[0] === "agent" && command[1] === "start")
      .map((command) => [command[2] ?? "", command.slice(command.indexOf("--") + 1)]),
  );
}

function teamConfiguration(fixture: Fixture, teamId: string, generation: number): {
  models?: Record<string, string>;
  profileDigests: Record<string, string>;
} {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
  try {
    const row = database
      .query<{ configuration_json: string }, [string, number]>(
        "SELECT configuration_json FROM slp_local_teams WHERE team_id = ? AND generation = ?",
      )
      .get(teamId, generation);
    expect(row).not.toBeNull();
    return JSON.parse(row!.configuration_json);
  } finally {
    database.close();
  }
}

test("seat-model: team start --lead-model/--supervisor-model/--peer-model ride the launch line after --agent/--profile (and --autocompact), are recorded on the team row, leave the rendered seat files and digests byte-identical, and print one models line (room d128 a, c, d, f, h)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "team-supervisor.md"),
      "---\nharness: claude\nmodel: opus\nautocompact: 250000\ndescription: shadowed supervisor\n---\nRole: Team Supervisor (shadow).\n",
    );
    const fake = await installFakeHerdr(fixture);
    // The shipped lead and peer are codex, the shadowed supervisor is claude,
    // so one start covers both launch-line shapes.
    const supervisorRender = renderedProfilePath(fixture.home, "claude", "team-supervisor");
    const leadRender = renderedProfilePath(fixture.home, "codex", "lead");
    const before = { lead: await readFile(leadRender, "utf8"), supervisor: await readFile(supervisorRender, "utf8") };
    const directories = profileDirectories(fixture.repo, fixture.home);
    const expectedDigests: Record<string, string> = {};
    for (const name of ["team-supervisor", "lead", "peer"]) {
      const profile = await resolveProfile(name, directories);
      expect(profile).not.toBeNull();
      expectedDigests[name] = profileDigest(profile!);
    }

    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Models under test", "--lead-model", "gpt-5.6-sol", "--supervisor-model", "fable", "--peer-model", "gpt-5.6-luna", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    // (f): the chosen models are printed on one stderr line beside the phases.
    expect(phaseFree(started.stderr)).toEqual([
      "lead model: gpt-5.6-sol; peer model: gpt-5.6-luna; supervisor model: fable",
    ]);
    const data = envelope<StartedTeam>(started.stdout);
    const starts = startsOf(await fakeHerdrCommands(fake));
    expect(starts.get(`supervisor-${data.team.teamId}`)).toEqual([
      "--agent", "maestro-team-supervisor", "--autocompact", "250000", "--model", "fable",
    ]);
    expect(starts.get(`lead-${data.team.teamId}`)).toEqual(["--profile", "maestro-lead", "--model", "gpt-5.6-sol"]);

    // (a): recorded on the team row next to the profiles; (c): the digests and
    // the rendered files are exactly what a start without the flags pins.
    const configuration = teamConfiguration(fixture, data.team.teamId, data.team.generation);
    expect(configuration.models).toEqual({ lead: "gpt-5.6-sol", peer: "gpt-5.6-luna", teamSupervisor: "fable" });
    expect(configuration.profileDigests).toEqual(expectedDigests);
    expect(await readFile(supervisorRender, "utf8")).toBe(before.supervisor);
    expect(await readFile(leadRender, "utf8")).toBe(before.lead);
    expect(await readFile(supervisorRender, "utf8")).toContain("\nmodel: opus\n");
    expect(await readFile(supervisorRender, "utf8")).not.toContain("fable");

    // (h): --peer-model applies to a Peer opened by work add --to, and
    // work add --to --model overrides it for that pane only.
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const inherited = await runCliAt(fixture, fixture.repo, ["work", "add", "team peer model", "--to", "alpha", "--json"], leadEnvironment);
    expect(inherited.exitCode).toBe(0);
    const alpha = envelope<{ role: { name: string } }>(inherited.stdout).role.name;
    const overridden = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "own model", "--to", "beta", "--model", "gpt-5.6-sol", "--json"],
      leadEnvironment,
    );
    expect(overridden.exitCode).toBe(0);
    const beta = envelope<{ role: { name: string } }>(overridden.stdout).role.name;
    const claudePeer = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "claude peer", "--to", "gamma", "--profile", "peer-opus", "--model", "fable", "--json"],
      leadEnvironment,
    );
    expect(claudePeer.exitCode).toBe(0);
    const gamma = envelope<{ role: { name: string } }>(claudePeer.stdout).role.name;
    const peerStarts = startsOf(await fakeHerdrCommands(fake));
    expect(peerStarts.get(alpha)).toEqual(["--profile", "maestro-peer", "--model", "gpt-5.6-luna"]);
    expect(peerStarts.get(beta)).toEqual(["--profile", "maestro-peer", "--model", "gpt-5.6-sol"]);
    expect(peerStarts.get(gamma)).toEqual(["--agent", "maestro-peer-opus", "--model", "fable"]);
    // The override was for that one pane: the team row still says the team's peer model.
    expect(teamConfiguration(fixture, data.team.teamId, data.team.generation).models?.peer).toBe("gpt-5.6-luna");

    // Restore: an identical start of the running generation reads the models
    // back from the row (the same line is printed), and a start naming another
    // model is a configuration change and refused like --peer-profile.
    const restored = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Models under test", "--lead-model", "gpt-5.6-sol", "--supervisor-model", "fable", "--peer-model", "gpt-5.6-luna", "--json"],
      fake.env,
    );
    expect(restored.exitCode).toBe(0);
    expect(phaseFree(restored.stderr)).toEqual([
      "lead model: gpt-5.6-sol; peer model: gpt-5.6-luna; supervisor model: fable",
    ]);
    const changed = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Models under test", "--lead-model", "opus", "--json"],
      fake.env,
    );
    expect(changed.exitCode).toBe(1);
    expect(failure(changed.stderr).code).toBe("TEAM_RUNNING");
  });
}, 30_000);

test("seat-model: no flag means no --model on any launch line and no models on the row; the frontmatter model stays the default (room d128 d)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Defaults", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    expect(phaseFree(started.stderr)).toEqual([]);
    const data = envelope<StartedTeam>(started.stdout);
    for (const command of await fakeHerdrCommands(fake)) expect(command).not.toContain("--model");
    expect(teamConfiguration(fixture, data.team.teamId, data.team.generation).models).toBeUndefined();
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "plain peer", "--to", "alpha", "--json"], {
      ...fake.env,
      HERDR_PANE_ID: lead.paneId,
    });
    expect(added.exitCode).toBe(0);
    const alpha = envelope<{ role: { name: string } }>(added.stdout).role.name;
    expect(startsOf(await fakeHerdrCommands(fake)).get(alpha)).toEqual(["--profile", "maestro-peer"]);
    expect((await runCliAt(fixture, room, ["help", "team"], fake.env)).stdout).toContain("--lead-model");
  });
}, 30_000);

test("seat-model: an empty model, --supervisor-model on a lead-only team, and work add --model without --to (Lead's own item, Team Supervisor's item) are INVALID_OPTION (room d128 a, b)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { runtimePane: "spawn" });
    const empty = await runCliAt(fixture, room, ["team", "start", fixture.repo, "x", "--lead-model", "", "--json"], fake.env);
    expect(empty.exitCode).toBe(1);
    expect(failure(empty.stderr).code).toBe("INVALID_OPTION");
    expect(failure(empty.stderr).message).toContain("--lead-model");
    const supervisorless = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "x", "--lead-only", "--supervisor-model", "fable", "--json"],
      fake.env,
    );
    expect(supervisorless.exitCode).toBe(1);
    expect(failure(supervisorless.stderr).code).toBe("INVALID_OPTION");
    expect(failure(supervisorless.stderr).message).toContain("--supervisor-model");
    expect(failure(supervisorless.stderr).message).toContain("--lead-only");
    expect((await fakeHerdrCommands(fake)).filter((command) => command[0] === "agent" && command[1] === "start")).toEqual([]);

    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Refusals", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const ownItem = await runCliAt(fixture, fixture.repo, ["work", "add", "mine", "--model", "fable", "--json"], {
      ...fake.env,
      HERDR_PANE_ID: lead.paneId,
    });
    expect(ownItem.exitCode).toBe(1);
    expect(failure(ownItem.stderr).code).toBe("INVALID_OPTION");
    expect(failure(ownItem.stderr).message).toContain("--model applies to Lead work add --to <peer>");
    const supervisorItem = await runCliAt(fixture, fixture.repo, ["work", "add", "lead item", "--model", "fable", "--json"], {
      ...fake.env,
      HERDR_PANE_ID: supervisor.paneId,
    });
    expect(supervisorItem.exitCode).toBe(1);
    expect(failure(supervisorItem.stderr).code).toBe("INVALID_OPTION");
    const emptyPeer = await runCliAt(fixture, fixture.repo, ["work", "add", "peer item", "--to", "alpha", "--model", "", "--json"], {
      ...fake.env,
      HERDR_PANE_ID: lead.paneId,
    });
    expect(emptyPeer.exitCode).toBe(1);
    expect(failure(emptyPeer.stderr).code).toBe("INVALID_OPTION");
    expect((await fakeHerdrCommands(fake)).filter((command) => command[0] === "agent" && command[1] === "start")).toHaveLength(2);
  });
}, 30_000);
