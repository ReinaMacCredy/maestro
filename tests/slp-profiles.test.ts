import { expect, test } from "bun:test";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  parseProfile,
  profileDigest,
  profileDirectories,
  renderedProfilePath,
  resolveProfile,
} from "../src/plugins/profiles.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { fakeHerdrCommands, installFakeHerdr } from "./helpers-herdr.ts";
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
  const text = await readFile(renderedProfilePath(fixture.home, "claude", name), "utf8");
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
    // Contract prompts only; the w696 wake-up for the initial item rides along.
    const prompts = commands.filter(
      (command) =>
        command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("slp team "),
    );
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
    await rm(renderedProfilePath(fixture.home, "claude", "team-supervisor"));

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

test("snapshot-changed-names-files: the refusal names every file whose bytes decide the pinned profile - both paths when a frontmatter-only shadow inherits, the searched directories when it is gone - and names the two ways out (w705)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const profiles = join(fixture.home, "maestro", "profiles");
    await mkdir(profiles, { recursive: true });
    await writeFile(join(profiles, "housekeeper.md"), "---\nharness: claude\nmodel: sonnet\n---\nRole: Housekeeper.\n");
    const fake = await installFakeHerdr(fixture);
    const data = await startTeam(fixture, room, fake.env);
    const teamId = data.team.teamId;
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const add = (node: string) =>
      runCliAt(fixture, fixture.repo, ["work", "add", "item", "--to", node, "--json"], leadEnvironment);
    const shadow = join(profiles, "peer.md");
    const shippedPeer = join(shippedRoot, "profiles", "peer.md");

    // One file decides the mandate: the shadow that replaced it outright.
    await writeFile(shadow, "---\nharness: codex\nmodel: default\n---\nRole: Peer, edited mid-generation.\n");
    const replaced = failure((await add("beta")).stderr);
    expect(replaced.code).toBe("SLP_SNAPSHOT_CHANGED");
    expect(replaced.message).toContain(`pinned profile peer, whose bytes are decided by ${shadow};`);
    expect(replaced.message).toContain("put those bytes back as they were pinned");
    expect(replaced.message).toContain(`stop the generation with maestro team stop ${teamId}`);

    // Two files decide it: the shadow's frontmatter over the shipped mandate.
    await writeFile(shadow, "---\nharness: codex\nmodel: default\n---\n");
    const inheriting = failure((await add("beta")).stderr);
    expect(inheriting.code).toBe("SLP_SNAPSHOT_CHANGED");
    expect(inheriting.message).toContain(
      `pinned profile peer, whose bytes are decided by ${shadow} (frontmatter) and ${shippedPeer} (mandate);`,
    );
    await rm(shadow);
    expect((await add("beta")).exitCode).toBe(0);

    // A pinned profile with no shipped layer to fall back to: name where it was sought.
    expect((await add("peer-housekeeper")).exitCode).toBe(0);
    await rm(join(profiles, "housekeeper.md"));
    const missing = failure((await add("delta")).stderr);
    expect(missing.code).toBe("SLP_SNAPSHOT_CHANGED");
    expect(missing.message).toContain("pinned profile housekeeper (now missing from ");
    expect(missing.message).toContain(join(".maestro", "profiles"));
    expect(missing.message).toContain(`${profiles}, ${join(shippedRoot, "profiles")});`);
    expect(missing.message).toContain("restore it in one of those directories");
    expect(missing.message).toContain(`stop the generation with maestro team stop ${teamId}`);
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

    const render = renderedProfilePath(fixture.home, "claude", "peer-refuter");
    await rm(render);
    const uninstalled = await add(["--to", "peer-refuter"]);
    expect(uninstalled.exitCode).toBe(1);
    expect(failure(uninstalled.stderr).code).toBe("PROFILE_NOT_INSTALLED");
    expect(failure(uninstalled.stderr).message).toContain("maestro install");
    expect(await Bun.file(render).exists()).toBe(false);
  });
}, 30_000);

test("profile-shadow-inherits: a seat shadow carrying frontmatter and no body renders the shipped mandate under the shadow's frontmatter; its digest still covers both files; a shipped or non-seat profile with no body is still refused (w700, d862)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const profiles = join(fixture.home, "maestro", "profiles");
    await mkdir(profiles, { recursive: true });
    // The live specimen's shape: a shadow that exists for its frontmatter alone.
    await writeFile(
      join(profiles, "lead.md"),
      "---\nharness: claude\nmodel: opus\neffort: high\ndescription: SLP Lead seat on Claude Opus\n---\n",
    );
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    const shared = sharedContract(await readFile(join(fixture.home, "maestro", "SLP.md"), "utf8"));
    const shippedLead = parseProfile(
      "lead.md",
      await readFile(join(shippedRoot, "profiles", "lead.md"), "utf8"),
    ).body;
    const rendered = await claudeRender(fixture, "lead");
    // The body is the shipped mandate, inherited, not a copy anyone maintains.
    expect(rendered.body).toBe(`\n${shared}\n\n${shippedLead}\n`);
    expect(rendered.frontmatter).toContain("\nmodel: opus");
    expect(rendered.frontmatter).toContain("\neffort: high");
    // The frontmatter-only shadow carries no mandate of its own to report.
    expect(installed.stdout).not.toContain("shadows the shipped lead mandate");

    const resolved = (await resolveProfile("lead", profileDirectories(fixture.repo, fixture.home)))!;
    expect(resolved.path).toBe(join(profiles, "lead.md"));
    expect(resolved.bodyPath).toBe(join(shippedRoot, "profiles", "lead.md"));
    expect(resolved.body).toBe(shippedLead);

    // d862: two files decide this mandate, so both are inside the pinned digest -
    // otherwise a mid-generation edit to the inherited body would pass unnoticed.
    const layers = join(fixture.root, "layers");
    await mkdir(join(layers, "top"), { recursive: true });
    await mkdir(join(layers, "bottom"), { recursive: true });
    await writeFile(join(layers, "top", "lead.md"), "---\nharness: claude\ndescription: shadow\n---\n");
    const bottom = join(layers, "bottom", "lead.md");
    await writeFile(bottom, "---\nharness: codex\ndescription: shipped-ish\n---\nRole: Lead.\n\nFirst.\n");
    const directories = [join(layers, "top"), join(layers, "bottom")];
    const before = profileDigest((await resolveProfile("lead", directories))!);
    await writeFile(bottom, "---\nharness: codex\ndescription: shipped-ish\n---\nRole: Lead.\n\nSecond.\n");
    const after = (await resolveProfile("lead", directories))!;
    expect(after.body).toBe("Role: Lead.\n\nSecond.");
    expect(profileDigest(after)).not.toBe(before);
    // The shadow's own frontmatter still wins; only the mandate is inherited.
    expect(after.frontmatter.harness).toBe("claude");

    // The bottom layer is the last word, so an empty body there is still refused.
    await writeFile(bottom, "---\nharness: codex\ndescription: shipped-ish\n---\n");
    await expect(resolveProfile("lead", directories)).rejects.toThrow(/missing body/);
    expect(() =>
      parseProfile(join(shippedRoot, "profiles", "lead.md"), "---\nharness: codex\ndescription: x\n---\n"),
    ).toThrow("missing body: the mandate below the frontmatter is empty");

    // Seat shadows only: a non-seat profile with no body is refused as before.
    await writeFile(join(profiles, "refuter.md"), "---\nharness: claude\ndescription: refuter\n---\n");
    const refused = await runCli(fixture, ["install"], { PATH: path });
    expect(refused.exitCode).toBe(1);
    expect(failure(refused.stderr).code).toBe("INVALID_PROFILE");
    expect(failure(refused.stderr).message).toContain("missing body");
  });
}, 40_000);

test("profile-shadow-reported: a seat shadow that still carries its own mandate keeps overriding, and install reports it - a byte-identical copy as a warning, a genuinely different body as a plain line (w700, d862)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const profiles = join(fixture.home, "maestro", "profiles");
    await mkdir(profiles, { recursive: true });
    const shippedLead = parseProfile(
      "lead.md",
      await readFile(join(shippedRoot, "profiles", "lead.md"), "utf8"),
    ).body;
    const shadow = join(profiles, "lead.md");
    const frontmatter = "---\nharness: claude\nmodel: opus\ndescription: SLP Lead seat on Claude Opus\n---\n";

    // The duplicate: the exact shape that goes stale the next time lead.md changes.
    await writeFile(shadow, `${frontmatter}${shippedLead}\n`);
    const duplicate = await runCli(fixture, ["install"], { PATH: path });
    expect(duplicate.exitCode).toBe(0);
    expect(duplicate.stdout).toContain(
      `warning: ${shadow} shadows the shipped lead mandate with a byte-identical copy`,
    );
    expect(duplicate.stdout).toContain("delete its body and keep its frontmatter");
    const shared = sharedContract(await readFile(join(fixture.home, "maestro", "SLP.md"), "utf8"));
    expect((await claudeRender(fixture, "lead")).body).toBe(`\n${shared}\n\n${shippedLead}\n`);

    // A deliberate override still overrides, and is reported without alarm.
    const ownBody = "Role: Lead.\n\nThis room's Lead runs the release checklist first.";
    await writeFile(shadow, `${frontmatter}${ownBody}\n`);
    const different = await runCli(fixture, ["install"], { PATH: path });
    expect(different.exitCode).toBe(0);
    expect(different.stdout).toContain(`${shadow} carries its own lead mandate, which differs from the shipped one`);
    expect(different.stdout).not.toContain("byte-identical");
    expect((await claudeRender(fixture, "lead")).body).toBe(`\n${shared}\n\n${ownBody}\n`);

    // Reduced to frontmatter, the shadow stops being reported and inherits.
    await writeFile(shadow, frontmatter);
    const inherited = await runCli(fixture, ["install"], { PATH: path });
    expect(inherited.exitCode).toBe(0);
    expect(inherited.stdout).not.toContain("lead mandate");
    expect((await claudeRender(fixture, "lead")).body).toBe(`\n${shared}\n\n${shippedLead}\n`);
  });
}, 40_000);

test("shadow-advice-precondition: every install line that advises deleting a shadow body says the edit changes the pinned digest and when it is safe, whether the copy is identical or different (w705, d864)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const profiles = join(fixture.home, "maestro", "profiles");
    await mkdir(profiles, { recursive: true });
    const shippedLead = parseProfile(
      "lead.md",
      await readFile(join(shippedRoot, "profiles", "lead.md"), "utf8"),
    ).body;
    const shadow = join(profiles, "lead.md");
    const frontmatter = "---\nharness: claude\nmodel: opus\ndescription: SLP Lead seat on Claude Opus\n---\n";
    const precondition =
      "that edit changes the lead profile digest, which any RUNNING SLP generation on this machine has pinned, so make it only when none is running or stop that generation first with maestro team stop <team>";

    await writeFile(shadow, `${frontmatter}${shippedLead}\n`);
    const duplicate = await runCli(fixture, ["install"], { PATH: path });
    expect(duplicate.exitCode).toBe(0);
    const duplicateLine = duplicate.stdout.split("\n").find((line) => line.includes("byte-identical copy"))!;
    expect(duplicateLine).toContain("delete its body and keep its frontmatter");
    expect(duplicateLine).toContain(precondition);

    await writeFile(shadow, `${frontmatter}Role: Lead.\n\nThis room's Lead runs the release checklist first.\n`);
    const different = await runCli(fixture, ["install"], { PATH: path });
    expect(different.exitCode).toBe(0);
    const differentLine = different.stdout.split("\n").find((line) => line.includes("carries its own lead mandate"))!;
    expect(differentLine).toContain("delete its body to inherit instead");
    expect(differentLine).toContain(precondition);

    // Nothing to advise, nothing to caveat.
    await writeFile(shadow, frontmatter);
    const inherited = await runCli(fixture, ["install"], { PATH: path });
    expect(inherited.exitCode).toBe(0);
    expect(inherited.stdout).not.toContain("RUNNING SLP generation");
  });
}, 40_000);

test("profile-shadow-composed: a frontmatter-only peer shadow leaves the composed peer variant byte-identical - shared contract, inherited peer mandate, then the node body (w700 scope check)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const profiles = join(fixture.home, "maestro", "profiles");
    await mkdir(profiles, { recursive: true });
    const refuterBody = "Role: Refuter.\n\nAttack the strongest claim in the return first.";
    await writeFile(
      join(profiles, "refuter.md"),
      `---\nharness: claude\nmodel: sonnet\ndescription: refuter lens\n---\n${refuterBody}\n`,
    );
    // The peer seat is the one whose body the composed variant concatenates, so
    // it is the only place inheritance could reach that site.
    await writeFile(join(profiles, "peer.md"), "---\nharness: claude\nmodel: opus\ndescription: peer on opus\n---\n");
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const shared = sharedContract(await readFile(join(fixture.home, "maestro", "SLP.md"), "utf8"));
    const shippedPeer = parseProfile(
      "peer.md",
      await readFile(join(shippedRoot, "profiles", "peer.md"), "utf8"),
    ).body;
    // Identical to what a byte-copy shadow renders today: the concatenation site
    // is untouched and only its input is inherited.
    expect((await claudeRender(fixture, "peer-refuter")).body).toBe(
      `\n${shared}\n\n${shippedPeer}\n\n${refuterBody}\n`,
    );
    expect((await claudeRender(fixture, "peer")).body).toBe(`\n${shared}\n\n${shippedPeer}\n`);
    expect((await claudeRender(fixture, "peer")).frontmatter).toContain("\nmodel: opus");
  });
}, 40_000);
