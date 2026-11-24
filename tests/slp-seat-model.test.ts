import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { resolveSeat, seatDigest, seatLaunchPath } from "../src/plugins/slp-seats.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { fakeHerdrCommands, installFakeHerdr } from "./helpers-herdr.ts";
import { enableSlp, runCliAt, withFixture, type Fixture } from "./helpers.ts";

// room d128 (w728): a seat's model is a launch-time choice. team start gains
// --lead-model and --peer-model, recorded on the team row; work add --to
// gains --model for one Peer. The model
// rides the launch line after --agent/--profile the way --autocompact does, so
// the seat file, its digest and the pinned pack are untouched. The
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

test("seat-model: team start --lead-model/--peer-model ride the launch line after --agent/--profile (and --autocompact), are recorded on the team row, leave the rendered seat files and digests byte-identical, and print one models line (room d128 a, c, d, f, h)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "lead.md"),
      "---\nharness: claude\nmodel: opus\nautocompact: 250000\ndescription: shadowed lead\n---\nRole: Lead (shadow).\n",
    );
    const fake = await installFakeHerdr(fixture);
    // The shipped peer is codex and the shadowed lead is claude, so one start
    // covers both launch-line shapes.
    const leadRender = seatLaunchPath(fixture.home, "claude", "lead");
    const before = { lead: await readFile(leadRender, "utf8") };
    const expectedDigests: Record<string, string> = {};
    for (const name of ["lead", "peer"] as const) {
      expectedDigests[name] = seatDigest(await resolveSeat(name, fixture.repo, fixture.home));
    }

    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Models under test", "--lead-model", "fable", "--peer-model", "gpt-5.6-luna", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    // (f): the chosen models are printed on one stderr line beside the phases.
    expect(phaseFree(started.stderr)).toEqual([
      "lead model: fable; peer model: gpt-5.6-luna",
    ]);
    const data = envelope<StartedTeam>(started.stdout);
    const starts = startsOf(await fakeHerdrCommands(fake));
    expect(starts.get(`lead-${data.team.teamId}`)).toEqual([
      "--agent", "maestro-lead", "--autocompact", "250000", "--model", "fable",
    ]);

    // (a): recorded on the team row next to the digests; (c): the digests and
    // the rendered files are exactly what a start without the flags pins.
    const configuration = teamConfiguration(fixture, data.team.teamId, data.team.generation);
    expect(configuration.models).toEqual({ lead: "fable", peer: "gpt-5.6-luna" });
    expect(configuration.profileDigests).toEqual(expectedDigests);
    expect(await readFile(leadRender, "utf8")).toBe(before.lead);
    expect(await readFile(leadRender, "utf8")).toContain("\nmodel: opus\n");
    expect(await readFile(leadRender, "utf8")).not.toContain("fable");

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
    const peerStarts = startsOf(await fakeHerdrCommands(fake));
    expect(peerStarts.get(alpha)).toEqual(["--profile", "maestro-peer", "--model", "gpt-5.6-luna"]);
    expect(peerStarts.get(beta)).toEqual(["--profile", "maestro-peer", "--model", "gpt-5.6-sol"]);
    // The override was for that one pane: the team row still says the team's peer model.
    expect(teamConfiguration(fixture, data.team.teamId, data.team.generation).models?.peer).toBe("gpt-5.6-luna");

    // Restore: an identical start of the running generation reads the models
    // back from the row (the same line is printed), and a start naming another
    // model is a configuration change and refused.
    const restored = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Models under test", "--lead-model", "fable", "--peer-model", "gpt-5.6-luna", "--json"],
      fake.env,
    );
    expect(restored.exitCode).toBe(0);
    expect(phaseFree(restored.stderr)).toEqual([
      "lead model: fable; peer model: gpt-5.6-luna",
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

test("seat-model: an empty model and work add --model without --to (the Lead's own item) are INVALID_OPTION (room d128 a, b)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { runtimePane: "spawn" });
    const empty = await runCliAt(fixture, room, ["team", "start", fixture.repo, "x", "--lead-model", "", "--json"], fake.env);
    expect(empty.exitCode).toBe(1);
    expect(failure(empty.stderr).code).toBe("INVALID_OPTION");
    expect(failure(empty.stderr).message).toContain("--lead-model");
    expect((await fakeHerdrCommands(fake)).filter((command) => command[0] === "agent" && command[1] === "start")).toEqual([]);

    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Refusals", "--lead-model", "gpt-5.6-sol", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    const leadStart = (await fakeHerdrCommands(fake)).find(
      (command) => command[0] === "agent" && command[1] === "start" && command[2] === `lead-${data.team.teamId}`,
    )!;
    expect(leadStart[leadStart.indexOf("--kind") + 1]).toBe("codex");
    expect(leadStart.slice(leadStart.indexOf("--model"))).toEqual(["--model", "gpt-5.6-sol"]);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const ownItem = await runCliAt(fixture, fixture.repo, ["work", "add", "mine", "--model", "fable", "--json"], {
      ...fake.env,
      HERDR_PANE_ID: lead.paneId,
    });
    expect(ownItem.exitCode).toBe(1);
    expect(failure(ownItem.stderr).code).toBe("INVALID_OPTION");
    expect(failure(ownItem.stderr).message).toContain("--model applies to Lead work add --to <peer>");
    const emptyPeer = await runCliAt(fixture, fixture.repo, ["work", "add", "peer item", "--to", "alpha", "--model", "", "--json"], {
      ...fake.env,
      HERDR_PANE_ID: lead.paneId,
    });
    expect(emptyPeer.exitCode).toBe(1);
    expect(failure(emptyPeer.stderr).code).toBe("INVALID_OPTION");
    expect((await fakeHerdrCommands(fake)).filter((command) => command[0] === "agent" && command[1] === "start")).toHaveLength(1);
  });
}, 30_000);

// Rework on w728 (part b): the launch line is the only carrier for --model, so
// a Peer pane that is already acknowledged cannot take one, with or without
// --fresh (a /clear is not a relaunch). Refused before any pane is ensured and
// before any store write.
test("seat-model: work add --to <peer> --model on an already-acknowledged Peer pane is refused with INVALID_OPTION naming the launch line and the way out, with or without --fresh, and writes nothing (room d128 b, rework)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Reused peer", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const opened = await runCliAt(fixture, fixture.repo, ["work", "add", "first item", "--to", "alpha", "--model", "gpt-5.6-sol", "--json"], leadEnvironment);
    expect(opened.exitCode).toBe(0);
    const alpha = envelope<{ role: { name: string } }>(opened.stdout).role.name;
    expect(startsOf(await fakeHerdrCommands(fake)).get(alpha)).toEqual(["--profile", "maestro-peer", "--model", "gpt-5.6-sol"]);
    const startsBefore = (await fakeHerdrCommands(fake)).filter((command) => command[0] === "agent" && command[1] === "start").length;
    const workCount = (): number => {
      const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
      try {
        return database.query<{ n: number }, []>("SELECT COUNT(*) AS n FROM slp_work").get()!.n;
      } finally {
        database.close();
      }
    };
    const workBefore = workCount();

    for (const extra of [[], ["--fresh"]]) {
      const refused = await runCliAt(
        fixture,
        fixture.repo,
        ["work", "add", "second item", "--to", "alpha", "--model", "fable", ...extra, "--json"],
        leadEnvironment,
      );
      expect(refused.exitCode).toBe(1);
      expect(failure(refused.stderr).code).toBe("INVALID_OPTION");
      expect(failure(refused.stderr).message).toContain("launch line");
      expect(failure(refused.stderr).message).toContain("another Peer name");
    }
    // The same model as the pane was opened with is refused too: the row does
    // not keep the launch model, so nothing can prove it matches.
    const same = await runCliAt(fixture, fixture.repo, ["work", "add", "third item", "--to", "alpha", "--model", "gpt-5.6-sol", "--json"], leadEnvironment);
    expect(same.exitCode).toBe(1);
    expect(failure(same.stderr).code).toBe("INVALID_OPTION");
    expect((await fakeHerdrCommands(fake)).filter((command) => command[0] === "agent" && command[1] === "start")).toHaveLength(startsBefore);
    expect(workCount()).toBe(workBefore);

    // Without --model the reused pane still takes work, and a new Peer name
    // with --model opens a new pane carrying it.
    const plain = await runCliAt(fixture, fixture.repo, ["work", "add", "plain reuse", "--to", "alpha", "--json"], leadEnvironment);
    expect(plain.exitCode).toBe(0);
    const fresh = await runCliAt(fixture, fixture.repo, ["work", "add", "new peer", "--to", "beta", "--model", "fable", "--json"], leadEnvironment);
    expect(fresh.exitCode).toBe(0);
    const beta = envelope<{ role: { name: string } }>(fresh.stdout).role.name;
    expect(startsOf(await fakeHerdrCommands(fake)).get(beta)).toEqual(["--profile", "maestro-peer", "--model", "fable"]);
  });
}, 30_000);

// Hub finding F1 (g7, w734): outside SLP - a plain checkout or the Hub room,
// where maybeHandleSlpWorkAdd does not take the add - work add used to ignore
// --model and --fresh in silence, so an option that only a Lead's Peer add can
// honour looked accepted. Both refuse the same way, before any store write.
test("seat-model: work add outside SLP refuses --model and --fresh alike and writes nothing (Hub finding F1)", async () => {
  await withFixture(async (fixture) => {
    await enableSlp(fixture);
    const listIds = async (): Promise<string[]> => {
      const listed = await runCliAt(fixture, fixture.repo, ["work", "list", "--json"]);
      expect(listed.exitCode).toBe(0);
      return envelope<{ works: { id: string }[] }>(listed.stdout).works.map((row) => row.id);
    };
    const before = await listIds();
    for (const option of [["--model", "fable"], ["--fresh"]]) {
      const refused = await runCliAt(fixture, fixture.repo, ["work", "add", "plain item", ...option, "--json"]);
      expect(refused.exitCode).toBe(1);
      expect(failure(refused.stderr).code).toBe("INVALID_OPTION");
      expect(failure(refused.stderr).message).toContain(`${option[0]} is available only for SLP work add --to <peer>`);
      expect(await listIds()).toEqual(before);
    }
    const plain = await runCliAt(fixture, fixture.repo, ["work", "add", "plain item", "--json"]);
    expect(plain.exitCode).toBe(0);
    expect((await listIds()).length).toBe(before.length + 1);
  });
}, 30_000);
