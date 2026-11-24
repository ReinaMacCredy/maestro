import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { seatLaunchPath, seatTokenPath } from "../src/plugins/slp-seats.ts";
import { fakeHerdrCommands, installFakeHerdr, readFakeHerdrState } from "./helpers-herdr.ts";
import { enableSlp, prepareInstallFixture, runCli, runCliAt, withFixture } from "./helpers.ts";
import { envelope, phaseFree } from "./helpers-slp.ts";

const offHint = "is part of the slp plugin, which is off; run: maestro plugin enable slp";

test("slp is off by default: ordinary verbs create no slp_ table, its verbs are refused with the enable hint, and work add --to is refused (d879)", async () => {
  await withFixture(async (fixture) => {
    for (const args of [
      ["work", "add", "plain item"],
      ["work", "note", "w1", "a note"],
      ["work", "list"],
      ["decision", "show", "d1"],
      ["search", "plain"],
      ["hook", "record", "--event", "SessionStart", "--harness", "claude"],
    ]) {
      await runCli(fixture, args);
    }
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    try {
      const slpTables = database
        .query<{ name: string }, []>("SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'slp\\_%' ESCAPE '\\'")
        .all();
      expect(slpTables).toEqual([]);
    } finally {
      database.close();
    }

    for (const [verb, args] of [
      ["team", ["team", "start", fixture.repo, "objective"]],
      ["slp", ["slp", "status"]],
      ["decide", ["decide", "a choice", "--why", "a reason"]],
      ["work take", ["work", "take", "w1"]],
      ["work return", ["work", "return", "w1", "body"]],
      ["work accept", ["work", "accept", "w1"]],
    ] as const) {
      const refused = await runCli(fixture, [...args]);
      expect({ verb, exitCode: refused.exitCode, message: JSON.parse(refused.stderr).error.message }).toEqual({
        verb,
        exitCode: 1,
        message: `${verb} ${offHint}`,
      });
    }
    const status = await runCli(fixture, ["status"]);
    expect(status.exitCode).toBe(1);
    expect(JSON.parse(status.stderr).error.message).toBe(
      "status left core (d896): for tracked work and who holds it run: maestro work list; the team readout is part of the slp plugin, which is off; run: maestro plugin enable slp",
    );
    const assigned = await runCli(fixture, ["work", "add", "for a peer", "--to", "peer-1"]);
    expect(assigned.exitCode).toBe(2);
    expect(JSON.parse(assigned.stderr).error).toMatchObject({ code: "UNKNOWN_FLAG", flag: "--to" });
    expect((await runCli(fixture, ["recipe", "list"])).stdout).not.toContain("slp\t");

    await enableSlp(fixture);
    expect((await runCli(fixture, ["help", "team"])).exitCode).toBe(0);
    const outsideRoom = await runCli(fixture, ["team", "start", fixture.repo, "objective"]);
    expect(JSON.parse(outsideRoom.stderr).error.code).toBe("ROLE_FORBIDDEN");
    expect((await runCli(fixture, ["recipe", "list"])).stdout).toContain("slp\t");
  });
}, 60_000);

test("install with slp off removes the Herdr plugin link and the rendered launch files an earlier install wrote, says so once, and keeps the seat token (d879)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const fake = await installFakeHerdr(fixture);
    const env = { HERDR_SOCKET_PATH: fake.env.HERDR_SOCKET_PATH, PATH: path };
    const earlier = await runCli(fixture, ["install"], env);
    expect(earlier.exitCode).toBe(0);
    expect(earlier.stdout).toContain("herdr plugin: linked");
    const launchFiles = (["lead", "peer"] as const).flatMap((seat) =>
      (["claude", "codex"] as const).map((harness) => seatLaunchPath(fixture.home, harness, seat))
    );
    const manifest = join(fixture.home, "maestro", "herdr-plugin.toml");
    for (const file of [...launchFiles, manifest]) expect({ file, exists: existsSync(file) }).toEqual({ file, exists: true });

    await rm(join(fixture.home, ".maestro", "config"));
    const off = await runCli(fixture, ["install"], env);
    expect(off.exitCode).toBe(0);
    expect(off.stdout.match(/the slp plugin is off, so install removed/g)).toEqual(["the slp plugin is off, so install removed"]);
    expect(off.stdout).not.toContain("seat dirs:");
    expect(off.stdout).not.toContain("herdr plugin:");
    for (const file of [...launchFiles, manifest]) expect({ file, exists: existsSync(file) }).toEqual({ file, exists: false });
    expect(existsSync(seatTokenPath(fixture.home))).toBe(true);
    expect((await readFakeHerdrState(fake)).plugins).toEqual([]);

    const callsBefore = (await fakeHerdrCommands(fake)).length;
    const again = await runCli(fixture, ["install"], env);
    expect(again.exitCode).toBe(0);
    expect(again.stdout).not.toContain("slp plugin is off");
    expect(again.stderr).toBe("");
    expect((await fakeHerdrCommands(fake)).length).toBe(callsBefore);
  });
}, 120_000);

test("team start from a room that enabled slp turns the plugin on in the project config (d879)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    await runCliAt(fixture, room, ["room", "mark"], { MAESTRO_ROOM_SCAFFOLD: "1", MAESTRO_SESSION_NONE: "1" });
    const fake = await installFakeHerdr(fixture);
    await rm(join(fixture.home, ".maestro", "config"));
    await mkdir(join(room, ".maestro"), { recursive: true });
    await writeFile(join(room, ".maestro", "config"), `${JSON.stringify({ plugins: [{ name: "slp", disabled: false }] })}\n`);
    expect((await runCli(fixture, ["help", "work", "take"])).exitCode).not.toBe(0);

    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Implement the change", "--json"], fake.env);
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    expect(envelope<{ team: { state: string } }>(started.stdout).team.state).toBe("RUNNING");
    const config = JSON.parse(await readFile(join(fixture.repo, ".maestro", "config"), "utf8")) as {
      plugins: Array<{ disabled?: boolean; name: string }>;
    };
    expect(config.plugins).toContainEqual({ name: "slp", disabled: false });
    expect((await runCli(fixture, ["help", "work", "take"])).exitCode).toBe(0);
  });
}, 120_000);
