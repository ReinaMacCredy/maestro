import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { parseSeatFile, resolveSeat, seatDigest, seatLaunchPath, type SeatName } from "../src/plugins/slp-seats.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { fakeHerdrCommands, installFakeHerdr } from "./helpers-herdr.ts";
import { enableSlp, prepareInstallFixture, runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

const shippedRoot = join(import.meta.dir, "..", "src", "plugins", "resources");

function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

function failure(stderr: string): { code: string; message: string } {
  return (JSON.parse(stderr) as { error: { code: string; message: string } }).error;
}

const runtimePhaseLine =
  /^\S+: (?:starting (?:claude|codex) pane in \S+|waiting for acknowledgement \(up to \d+s\)|ready in \d+s|already (?:acknowledged|running) in \S+; left alone)$/;

function phaseFree(stderr: string): string {
  return stderr.split("\n").filter((line) => line !== "" && !runtimePhaseLine.test(line)).join("\n");
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

function sharedContract(pack: string): string {
  return (/<!-- slp:shared:begin -->([\s\S]*?)<!-- slp:shared:end -->/.exec(pack)?.[1] ?? "").trim();
}

async function claudeRender(fixture: Fixture, seat: SeatName): Promise<{ body: string; frontmatter: string }> {
  const text = await readFile(seatLaunchPath(fixture.home, "claude", seat), "utf8");
  const close = text.indexOf("\n---\n");
  return { body: text.slice(close + "\n---\n".length), frontmatter: text.slice(0, close) };
}

async function startTeam(fixture: Fixture, room: string, env: Record<string, string>) {
  const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Seats under test", "--json"], env);
  expect(phaseFree(started.stderr)).toBe("");
  expect(started.exitCode).toBe(0);
  return envelope<{
    team: { roles: Array<{ name: string; paneId: string; profile: string; role: string }>; teamId: string };
    work: { id: string };
  }>(started.stdout);
}

function startsOf(commands: string[][]): Map<string, string[]> {
  return new Map(
    commands
      .filter((command) => command[0] === "agent" && command[1] === "start")
      .map((command) => [command[2] ?? "", command.slice(command.indexOf("--") + 1)]),
  );
}

test("team-start-launch-args: each seat launches as maestro-<seat> with its autocompact and no --model when no model flag is given, the contract prompt is one line, and --peer-profile and --lead-profile are unknown flags (red 5; d906)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "peer.md"),
      "---\nharness: claude\nmodel: default\nautocompact: 250000\ndescription: claude peer\n---\n",
    );
    const fake = await installFakeHerdr(fixture);

    const data = await startTeam(fixture, room, fake.env);
    const commands = await fakeHerdrCommands(fake);
    expect(startsOf(commands).get(`lead-${data.team.teamId}`)).toEqual(["--profile", "maestro-lead"]);
    const prompts = commands.filter(
      (command) =>
        command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("slp team "),
    );
    expect(prompts).toHaveLength(1);
    for (const prompt of prompts) {
      expect(prompt[3]).toMatch(/^slp team \S+ generation 1 instance [0-9a-f-]{36}; reply [0-9a-f]{32}$/);
      expect(prompt[3]?.split("\n")).toHaveLength(1);
    }

    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "claude peer item", "--to", "x", "--json"], {
      ...fake.env,
      HERDR_PANE_ID: lead.paneId,
    });
    expect(phaseFree(added.stderr)).toBe("");
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; profile: string } }>(added.stdout).role;
    expect(peer.profile).toBe("peer");
    expect(startsOf(await fakeHerdrCommands(fake)).get(peer.name)).toEqual(["--agent", "maestro-peer", "--autocompact", "250000"]);
    for (const command of await fakeHerdrCommands(fake)) expect(command).not.toContain("--model");

    for (const flag of ["--peer-profile", "--lead-profile"]) {
      const unknown = await runCliAt(fixture, room, ["team", "start", fixture.repo, "x", flag, "peer", "--json"], fake.env);
      expect(unknown.exitCode).toBe(2);
      expect(failure(unknown.stderr).code).toBe("UNKNOWN_FLAG");
    }
    const help = (await runCliAt(fixture, room, ["help", "team"], fake.env)).stdout;
    for (const flag of ["--lead-model", "--peer-model"]) expect(help).toContain(flag);
    expect(help).not.toContain("profile");
  });
}, 30_000);

test("launch-refuses-uninstalled: a missing seat launch file fails team start with SEAT_NOT_INSTALLED before any Herdr call (red 6, A2)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture);
    await rm(seatLaunchPath(fixture.home, "codex", "lead"));

    const refused = await runCliAt(fixture, room, ["team", "start", fixture.repo, "No seat file", "--json"], fake.env);
    expect(refused.exitCode).toBe(1);
    expect(failure(refused.stderr).code).toBe("SEAT_NOT_INSTALLED");
    expect(failure(refused.stderr).message).toContain("maestro install");
    expect(failure(refused.stderr).message).toContain("maestro-lead");
    const commands = await fakeHerdrCommands(fake);
    expect(commands.filter((command) => command[0] === "agent" && command[1] === "start")).toEqual([]);
    expect(commands.filter((command) => command[1] === "create")).toEqual([]);
  });
}, 30_000);

test("pin-seats: editing a pinned seat file fails the next work add naming every file whose bytes decide it - one path for a replaced mandate, both for a frontmatter-only owner file - and names the two ways out (red 7, A4, w705)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const owner = join(fixture.home, "maestro", "profiles");
    await mkdir(owner, { recursive: true });
    const fake = await installFakeHerdr(fixture);
    const data = await startTeam(fixture, room, fake.env);
    const teamId = data.team.teamId;
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const add = (node: string) =>
      runCliAt(fixture, fixture.repo, ["work", "add", "item", "--to", node, "--json"], { ...fake.env, HERDR_PANE_ID: lead.paneId });
    expect((await add("alpha")).exitCode).toBe(0);
    const ownerPeer = join(owner, "peer.md");
    const shippedPeer = join(shippedRoot, "slp", "peer.md");

    await writeFile(ownerPeer, "---\nharness: codex\nmodel: default\n---\nRole: Peer, edited mid-generation.\n");
    const replaced = failure((await add("beta")).stderr);
    expect(replaced.code).toBe("SLP_SNAPSHOT_CHANGED");
    expect(replaced.message).toContain(`pinned peer seat file, whose bytes are decided by ${ownerPeer};`);
    expect(replaced.message).toContain("put those bytes back as they were pinned");
    expect(replaced.message).toContain(`stop the generation with maestro team stop ${teamId}`);

    await writeFile(ownerPeer, "---\nharness: codex\nmodel: default\n---\n");
    const inheriting = failure((await add("beta")).stderr);
    expect(inheriting.code).toBe("SLP_SNAPSHOT_CHANGED");
    expect(inheriting.message).toContain(
      `pinned peer seat file, whose bytes are decided by ${ownerPeer} (frontmatter) and ${shippedPeer} (mandate);`,
    );
    await rm(ownerPeer);
    expect((await add("beta")).exitCode).toBe(0);
  });
}, 30_000);

// d906: a generation started before the profile system went pinned a peer
// variant by name and recorded it on the role row. Status and stop always read
// it; a work add goes through while the pinned lead bytes still match and is
// refused once they do not, as d906 itself changed the shipped lead mandate.
test("pre-d906-generation: a team row pinning profiles {lead, peer-opus} with a peer-opus digest and a peer-opus role reads in status and stops, takes a work add while the lead pin matches and refuses one with SLP_SNAPSHOT_CHANGED when it does not", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture);
    const data = await startTeam(fixture, room, fake.env);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const opened = await runCliAt(fixture, fixture.repo, ["work", "add", "old peer", "--to", "alpha", "--json"], leadEnvironment);
    expect(opened.exitCode).toBe(0);
    const peerName = envelope<{ role: { name: string } }>(opened.stdout).role.name;

    const pinOld = (leadDigest: string) => {
      const old = JSON.stringify({ profileDigests: { lead: leadDigest, "peer-opus": "f".repeat(64) }, profiles: { lead: "lead", peer: "peer-opus" } });
      for (const [path, table] of [
        [join(fixture.repo, ".maestro", "maestro.db"), "slp_local_teams"],
        [join(room, ".maestro", "maestro.db"), "slp_teams"],
      ] as const) {
        const database = new Database(path);
        database.query(`UPDATE ${table} SET configuration_json = ? WHERE team_id = ?`).run(old, data.team.teamId);
        database.close();
      }
    };
    pinOld(seatDigest(await resolveSeat("lead", fixture.repo, fixture.home)));
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    project.query("UPDATE slp_local_roles SET profile = 'peer-opus' WHERE name = ?").run(peerName);
    project.close();

    const reads = async (peerProfile: string) => {
      const status = await runCliAt(fixture, fixture.repo, ["status", "--json"], leadEnvironment);
      expect(status.exitCode).toBe(0);
      expect(envelope<{ roles: Array<{ profile: string; role: string }> }>(status.stdout).roles.map((role) => [role.role, role.profile]))
        .toEqual([["lead", "lead"], ["peer", peerProfile]]);
      expect((await runCliAt(fixture, room, ["status", "--json"], fake.env)).exitCode).toBe(0);
    };
    await reads("peer-opus");
    const reused = await runCliAt(fixture, fixture.repo, ["work", "add", "second item", "--to", "alpha", "--json"], leadEnvironment);
    expect(phaseFree(reused.stderr)).toBe("");
    expect(reused.exitCode).toBe(0);

    pinOld("e".repeat(64));
    await reads("peer");
    const refused = await runCliAt(fixture, fixture.repo, ["work", "add", "third item", "--to", "alpha", "--json"], leadEnvironment);
    expect(refused.exitCode).toBe(1);
    expect(failure(refused.stderr).code).toBe("SLP_SNAPSHOT_CHANGED");
    expect(failure(refused.stderr).message).toContain(`maestro team stop ${data.team.teamId}`);
    const stopped = await runCliAt(fixture, room, ["team", "stop", data.team.teamId, "--emergency", "--json"], fake.env);
    expect(stopped.exitCode).toBe(0);
  });
}, 30_000);

test("seat-file-inherits: an owner seat file carrying frontmatter and no body renders the shipped mandate under its frontmatter; its digest covers both files; the shipped layer still refuses an empty body (w700, d862)", async () => {
  await withFixture(async (fixture) => {
    await enableSlp(fixture);
    const { path } = await prepareInstallFixture(fixture);
    const owner = join(fixture.home, "maestro", "profiles");
    await mkdir(owner, { recursive: true });
    await writeFile(
      join(owner, "lead.md"),
      "---\nharness: claude\nmodel: opus\neffort: high\ndescription: SLP Lead seat on Claude Opus\n---\n",
    );
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const shared = sharedContract(await readFile(join(fixture.home, "maestro", "SLP.md"), "utf8"));
    const shippedLeadPath = join(shippedRoot, "slp", "lead.md");
    const shippedLead = parseSeatFile("lead.md", await readFile(shippedLeadPath, "utf8"), "shipped").body;
    const rendered = await claudeRender(fixture, "lead");
    expect(rendered.body).toBe(`\n${shared}\n\n${shippedLead}\n`);
    expect(rendered.frontmatter).toContain("\nmodel: opus");
    expect(rendered.frontmatter).toContain("\neffort: high");

    const resolved = await resolveSeat("lead", fixture.repo, fixture.home);
    expect(resolved.paths).toEqual([join(owner, "lead.md"), shippedLeadPath]);
    expect(resolved.body).toBe(shippedLead);
    expect(resolved.config.harness).toBe("claude");

    const before = seatDigest(resolved);
    await writeFile(join(owner, "lead.md"), "---\nharness: claude\nmodel: opus\neffort: high\ndescription: edited\n---\n");
    expect(seatDigest(await resolveSeat("lead", fixture.repo, fixture.home))).not.toBe(before);

    expect(() => parseSeatFile(shippedLeadPath, "---\nharness: codex\ndescription: x\n---\n", "shipped"))
      .toThrow("missing body: the mandate below the frontmatter is empty");
  });
}, 40_000);

test("lead-mandate-rendered: the installed maestro-lead seat carries room d118's self-work default and room d117's Hub reviewer line from the shared contract", async () => {
  await withFixture(async (fixture) => {
    await enableSlp(fixture);
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const unwrap = (text: string) => text.replaceAll(/\s+/g, " ");
    const rendered = unwrap((await claudeRender(fixture, "lead")).body);
    expect(rendered).toContain("Working your own items is the default path");
    expect(rendered).toContain("Open a Peer the way a harness opens a sub-agent");
    expect(rendered).toContain("The Lead's reviewer is the Hub Supervisor");
    expect(rendered).not.toContain("never implement a Peer's item yourself");

    const codexSeat = unwrap(await readFile(seatLaunchPath(fixture.home, "codex", "lead"), "utf8"));
    expect(codexSeat).toContain("Working your own items is the default path");
  });
}, 30_000);
