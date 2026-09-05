import { expect, test } from "bun:test";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { parseProfile } from "../src/plugins/profiles.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { fakeHerdrCommands, installFakeHerdr } from "./fake-herdr.ts";
import { prepareInstallFixture, runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

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

async function claudeRender(fixture: Fixture, name: string): Promise<{ body: string; frontmatter: string }> {
  const text = await readFile(join(fixture.home, ".claude", "agents", `maestro-${name}.md`), "utf8");
  const close = text.indexOf("\n---\n");
  return { body: text.slice(close + "\n---\n".length), frontmatter: text.slice(0, close) };
}

test("profile-seat-body: maestro-peer renders shared contract + peer body; maestro-peer-refuter adds the node body under the node's frontmatter (red 3, items 3 and 7)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    const refuterBody = "Role: Refuter.\n\nAttack the strongest claim in the return first.";
    await writeFile(
      join(fixture.home, "maestro", "profiles", "refuter.md"),
      `---\nharness: claude\nmodel: sonnet\ndescription: refuter lens\n---\n${refuterBody}\n`,
    );
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const shared = sharedContract(await readFile(join(fixture.home, "maestro", "SLP.md"), "utf8"));
    const peerBody = parseProfile("peer.md", await readFile(join(shippedRoot, "profiles", "peer.md"), "utf8")).body;
    expect(shared.startsWith("## Shared contract")).toBe(true);
    expect((await claudeRender(fixture, "peer")).body).toBe(`\n${shared}\n\n${peerBody}\n`);

    const composed = await claudeRender(fixture, "peer-refuter");
    expect(composed.body).toBe(`\n${shared}\n\n${peerBody}\n\n${refuterBody}\n`);
    expect(composed.frontmatter).toContain("\nmodel: sonnet");
    expect(composed.frontmatter).toContain("\nname: maestro-peer-refuter\n");
    const codexSession = await readFile(join(fixture.home, ".codex", "maestro-peer-refuter.config.toml"), "utf8");
    expect(codexSession).toContain('model = "sonnet"\n');
    expect(codexSession).toContain("Role: Refuter.");
  });
}, 30_000);

test("pack-v3-markers: a version-2 pack is refused naming slp:profile, an unknown profile is refused by name, the fixture pack resolves three seats to three profiles (red 4)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture);
    const shipped = await readFile(join(shippedRoot, "SLP.md"), "utf8");
    const packPath = join(room, "SLP.md");

    const v2 = shipped
      .replace("<!-- slp:version=3 -->", "<!-- slp:version=2 -->")
      .replace(/<!-- slp:profile:(team-supervisor|lead|peer)=[a-z-]+ -->/g, "<!-- slp:model:$1=codex:default -->");
    await writeFile(packPath, v2);
    const refusedV2 = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Old pack", "--json"], fake.env);
    expect(refusedV2.exitCode).toBe(1);
    expect(failure(refusedV2.stderr).code).toBe("INVALID_SLP_PACK");
    expect(failure(refusedV2.stderr).message).toContain("slp:profile");
    expect(failure(refusedV2.stderr).message).toContain("maestro install");

    await writeFile(packPath, shipped.replace("<!-- slp:profile:lead=lead -->", "<!-- slp:profile:lead=ghost -->"));
    const refusedGhost = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Ghost lead", "--json"], fake.env);
    expect(refusedGhost.exitCode).toBe(1);
    expect(failure(refusedGhost.stderr).code).toBe("PROFILE_NOT_FOUND");
    expect(failure(refusedGhost.stderr).message).toContain("ghost");
    expect((await fakeHerdrCommands(fake)).filter((command) => command[0] === "agent" && command[1] === "start")).toEqual([]);

    await writeFile(packPath, shipped);
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Three seats", "--json"], fake.env);
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ name: string; paneId: string; profile: string; role: string }>; teamId: string };
    }>(started.stdout);
    expect(data.team.roles.map((role) => [role.role, role.profile])).toEqual([
      ["team-supervisor", "team-supervisor"],
      ["lead", "lead"],
    ]);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer item", "--to", "alpha", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead.paneId },
    );
    expect(added.exitCode).toBe(0);
    const status = envelope<{ roles: Array<{ profile: string; role: string }> }>(
      (await runCliAt(fixture, fixture.repo, ["status", "--json"], { ...fake.env, HERDR_PANE_ID: lead.paneId })).stdout,
    );
    expect(status.roles.map((role) => [role.role, role.profile])).toEqual([
      ["team-supervisor", "team-supervisor"],
      ["lead", "lead"],
      ["peer", "peer"],
    ]);
    await rm(packPath, { force: true });
  });
}, 30_000);

async function startTeam(fixture: Fixture, room: string, env: Record<string, string>, extra: string[] = []) {
  const started = await runCliAt(
    fixture,
    room,
    ["team", "start", fixture.repo, "Profiles under test", ...extra, "--json"],
    env,
  );
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

test("team-start-launch-args: --agent/--profile with autocompact and no --model, a one-line prompt, --peer-profile recorded and applied, retired flags refused by name (red 5, items 5 and 6)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "team-supervisor.md"),
      "---\nharness: claude\nmodel: default\nautocompact: 250000\ndescription: shadowed supervisor\n---\nRole: Team Supervisor (shadow).\n",
    );
    const fake = await installFakeHerdr(fixture);

    const data = await startTeam(fixture, room, fake.env, ["--peer-profile", "peer-opus"]);
    const commands = await fakeHerdrCommands(fake);
    const starts = startsOf(commands);
    expect(starts.get(`supervisor-${data.team.teamId}`)).toEqual(["--agent", "maestro-team-supervisor", "--autocompact", "250000"]);
    expect(starts.get(`lead-${data.team.teamId}`)).toEqual(["--profile", "maestro-lead"]);
    for (const command of commands) expect(command).not.toContain("--model");
    const prompts = commands.filter((command) => command[0] === "agent" && command[1] === "prompt");
    expect(prompts).toHaveLength(2);
    for (const prompt of prompts) {
      expect(prompt[3]).toMatch(/^slp team \S+ generation 1 instance [0-9a-f-]{36}; reply [0-9a-f]{32}$/);
      expect(prompt[3]).not.toContain("Shared contract");
      expect(prompt[3]?.split("\n")).toHaveLength(1);
    }

    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "opus peer item", "--to", "x", "--json"], leadEnvironment);
    expect(phaseFree(added.stderr)).toBe("");
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; profile: string } }>(added.stdout).role;
    expect(peer.profile).toBe("peer-opus");
    expect(startsOf(await fakeHerdrCommands(fake)).get(peer.name)).toEqual(["--agent", "maestro-peer-opus"]);
    const hub = envelope<{ teams: Array<{ roles: Array<{ profile: string; role: string }> }> }>(
      (await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout,
    );
    expect(hub.teams[0]?.roles.map((role) => role.profile)).toEqual(["team-supervisor", "lead", "peer-opus"]);

    const retired = await runCliAt(fixture, room, ["team", "start", fixture.repo, "x", "--lead-model", "opus", "--json"], fake.env);
    expect(retired.exitCode).toBe(1);
    expect(failure(retired.stderr).code).toBe("RETIRED_FLAG");
    expect(failure(retired.stderr).message).toContain("--peer-profile");
    expect(failure(retired.stderr).message).toContain("profiles/lead.md");
    const unknown = await runCliAt(fixture, room, ["team", "start", fixture.repo, "x", "--lead-profile", "lead", "--json"], fake.env);
    expect(unknown.exitCode).toBe(2);
    expect(failure(unknown.stderr).code).toBe("UNKNOWN_FLAG");
    expect((await runCliAt(fixture, room, ["help", "team"], fake.env)).stdout).not.toContain("--lead-model");
  });
}, 30_000);

test("launch-refuses-uninstalled: a missing render fails team start with PROFILE_NOT_INSTALLED before any Herdr call (red 6, A2)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture);
    await rm(join(fixture.home, ".claude", "agents", "maestro-team-supervisor.md"));

    const refused = await runCliAt(fixture, room, ["team", "start", fixture.repo, "No render", "--json"], fake.env);
    expect(refused.exitCode).toBe(1);
    expect(failure(refused.stderr).code).toBe("PROFILE_NOT_INSTALLED");
    expect(failure(refused.stderr).message).toContain("maestro install");
    expect(failure(refused.stderr).message).toContain("maestro-team-supervisor");
    const commands = await fakeHerdrCommands(fake);
    expect(commands.filter((command) => command[0] === "agent" && command[1] === "start")).toEqual([]);
    expect(commands.filter((command) => command[1] === "create")).toEqual([]);
  });
}, 30_000);

test("pin-profiles: editing a referenced profile fails the next work add with the pack-digest error naming it; an unreferenced profile does not; a profile first used by work add joins the pin (red 7, A4)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const profiles = join(fixture.home, "maestro", "profiles");
    await mkdir(profiles, { recursive: true });
    await writeFile(join(profiles, "refuter.md"), "---\nharness: claude\nmodel: sonnet\n---\nRole: Refuter.\n");
    const fake = await installFakeHerdr(fixture);
    const data = await startTeam(fixture, room, fake.env);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const add = (args: string[]) =>
      runCliAt(fixture, fixture.repo, ["work", "add", "item", "--to", ...args, "--json"], leadEnvironment);

    await writeFile(join(profiles, "unrelated.md"), "---\nharness: claude\nmodel: default\n---\nRole: unrelated.\n");
    expect((await add(["alpha"])).exitCode).toBe(0);

    // A home peer.md now shadows the shipped peer the generation pinned.
    await writeFile(join(profiles, "peer.md"), "---\nharness: codex\nmodel: default\n---\nRole: Peer, edited mid-generation.\n");
    const refused = await add(["beta"]);
    expect(refused.exitCode).toBe(1);
    expect(failure(refused.stderr).code).toBe("SLP_SNAPSHOT_CHANGED");
    expect(failure(refused.stderr).message).toContain("pinned profile peer");
    await rm(join(profiles, "peer.md"));
    expect((await add(["beta"])).exitCode).toBe(0);

    await writeFile(join(profiles, "refuter.md"), "---\nharness: claude\nmodel: sonnet\n---\nRole: Refuter, edited before first use.\n");
    expect((await add(["gamma"])).exitCode).toBe(0);
    expect((await add(["peer-refuter"])).exitCode).toBe(0);
    await writeFile(join(profiles, "refuter.md"), "---\nharness: claude\nmodel: sonnet\n---\nRole: Refuter, edited after first use.\n");
    const refusedRefuter = await add(["delta"]);
    expect(refusedRefuter.exitCode).toBe(1);
    expect(failure(refusedRefuter.stderr).code).toBe("SLP_SNAPSHOT_CHANGED");
    expect(failure(refusedRefuter.stderr).message).toContain("pinned profile refuter");
  });
}, 30_000);

test("work-add-profile: peer-<node> and --profile pick the render, a profile switch on an existing Peer is refused, a missing render is refused and never rendered (red 8, items 6 and 7)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const profiles = join(fixture.home, "maestro", "profiles");
    await mkdir(profiles, { recursive: true });
    await writeFile(join(profiles, "refuter.md"), "---\nharness: claude\nmodel: sonnet\n---\nRole: Refuter.\n");
    const fake = await installFakeHerdr(fixture);
    const data = await startTeam(fixture, room, fake.env);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const add = (args: string[]) =>
      runCliAt(fixture, fixture.repo, ["work", "add", "item", ...args, "--json"], leadEnvironment);

    const refuter = await add(["--to", "peer-refuter"]);
    expect(refuter.exitCode).toBe(0);
    const refuterRole = envelope<{ role: { name: string; profile: string } }>(refuter.stdout).role;
    expect(refuterRole.profile).toBe("peer-refuter");
    expect(startsOf(await fakeHerdrCommands(fake)).get(refuterRole.name)).toEqual(["--agent", "maestro-peer-refuter"]);

    const alpha = await add(["--to", "alpha", "--profile", "peer-opus"]);
    expect(alpha.exitCode).toBe(0);
    const alphaRole = envelope<{ role: { name: string; profile: string } }>(alpha.stdout).role;
    expect(alphaRole.profile).toBe("peer-opus");
    expect(startsOf(await fakeHerdrCommands(fake)).get(alphaRole.name)).toEqual(["--agent", "maestro-peer-opus"]);

    const mismatch = await add(["--to", "alpha", "--profile", "peer"]);
    expect(mismatch.exitCode).toBe(1);
    expect(failure(mismatch.stderr).code).toBe("PEER_PROFILE_MISMATCH");
    expect((await add(["--to", "alpha", "--profile", "peer-opus"])).exitCode).toBe(0);

    const render = join(fixture.home, ".claude", "agents", "maestro-peer-refuter.md");
    await rm(render);
    const uninstalled = await add(["--to", "peer-refuter"]);
    expect(uninstalled.exitCode).toBe(1);
    expect(failure(uninstalled.stderr).code).toBe("PROFILE_NOT_INSTALLED");
    expect(failure(uninstalled.stderr).message).toContain("maestro install");
    expect(await Bun.file(render).exists()).toBe(false);
  });
}, 30_000);
