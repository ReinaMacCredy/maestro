import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync, realpathSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { HerdrClient, SlpRuntimeError, herdrProtocol } from "../src/plugins/herdr-client.ts";
import { materializeProfiles, seatTokenPath } from "../src/plugins/profiles.ts";
import { materializeSkills } from "../src/plugins/skills.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { slpRuntimeDirectory } from "../src/plugins/slp-process.ts";
import { runCli, runCliAt, withFixture } from "./helpers.ts";
import {
  editFakeHerdrState,
  emitFakeHerdrEvent,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
  tripwireInvocations,
  waitForFakeHerdr,
} from "./helpers-herdr.ts";

function captureStderr(): { restore(): string } {
  const chunks: string[] = [];
  const original = process.stderr.write.bind(process.stderr);
  process.stderr.write = ((chunk: string | Uint8Array) => {
    chunks.push(typeof chunk === "string" ? chunk : new TextDecoder().decode(chunk));
    return true;
  }) as typeof process.stderr.write;
  return {
    restore() {
      process.stderr.write = original;
      return chunks.join("");
    },
  };
}

test("herdr-client-request: one line per connection resolved by id, Herdr error codes, protocol mismatch warns and the call proceeds, an absent method fails by name (red 1)", async () => {
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const client = new HerdrClient({ ...process.env, ...fake.env, HOME: fixture.home });

    const agents = await client.request<{ agents: unknown[]; type: string }>("agent.list");
    expect(agents).toEqual({ agents: [], type: "agent_list" });
    expect(await fakeHerdrCommands(fake)).toEqual([["ping"], ["agent", "list"]]);

    const failure = await client.request("agent.get", { target: "nobody" }).catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(SlpRuntimeError);
    expect((failure as SlpRuntimeError).herdrCode).toBe("agent_not_found");
    expect((failure as SlpRuntimeError).message).toContain("agent_not_found");

    const missing = await client.request("agent.wait", { target: "nobody" }).catch((error: unknown) => error);
    expect(missing).toBeInstanceOf(SlpRuntimeError);
    expect((missing as SlpRuntimeError).code).toBe("HERDR_METHOD_MISSING");
    expect((missing as SlpRuntimeError).message).toContain("agent.wait");

    // A newer Herdr: the check runs once per client, so a fresh client sees it.
    await editFakeHerdrState(fake, (state) => {
      state.protocol = herdrProtocol + 1;
    });
    const stderr = captureStderr();
    let warned: string;
    try {
      const fresh = new HerdrClient({ ...process.env, ...fake.env, HOME: fixture.home });
      const listed = await fresh.request<{ type: string }>("workspace.list");
      expect(listed.type).toBe("workspace_list");
    } finally {
      warned = stderr.restore();
    }
    expect(warned).toContain(`protocol ${herdrProtocol + 1}`);
    expect(warned).toContain(`protocol ${herdrProtocol}`);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
});

// w712, d871: a caller of agent.list must tell a successful empty answer
// apart from an outage, so the fixture must be able to fail one
// named method while the server stays up and the socket stays in place.
// stopFakeHerdrServers cannot stand in for this: it kills spawned children
// and unlinks the socket the CLI still needs.
test("herdr-fail-methods: a named method answered with a Herdr error makes agentList throw instead of returning an empty list, while the server stays up and every other method keeps serving; unset, the same call succeeds (w712, d871)", async () => {
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const client = new HerdrClient({ ...process.env, ...fake.env, HOME: fixture.home });

    // Two successful answers, one empty and one not. Neither is an outage.
    expect(await client.agentList()).toEqual([]);
    await editFakeHerdrState(fake, (state) => {
      state.agents = [{ agent_status: "idle", name: "lead-probe", pane_id: "wZ:p1", workspace_id: "wZ" }];
    });
    expect((await client.agentList()).map((agent) => agent.name)).toEqual(["lead-probe"]);

    // The outage. It must throw, and it must not degrade to [].
    await setFakeHerdrBehavior(fake, { failMethods: ["agent.list"] });
    const failure = await client.agentList().catch((error: unknown) => error);
    expect(Array.isArray(failure)).toBe(false);
    expect(failure).toBeInstanceOf(SlpRuntimeError);
    expect((failure as SlpRuntimeError).herdrCode).toBe("internal");
    expect((failure as SlpRuntimeError).code).not.toBe("HERDR_METHOD_MISSING");
    expect((failure as SlpRuntimeError).message).toContain("agent.list");

    // The server is still up and the socket is still in place: the whole
    // reason this knob exists rather than stopping the server.
    expect(existsSync(fake.socket)).toBe(true);
    expect(await client.workspaceList()).toEqual([]);
    expect(await client.agentGet("lead-probe")).toMatchObject({ name: "lead-probe" });

    // Unset restores the successful answer on the same client.
    await setFakeHerdrBehavior(fake, { failMethods: [] });
    expect((await client.agentList()).map((agent) => agent.name)).toEqual(["lead-probe"]);

    // d875: a successful answer with no agents array, or with one that is not
    // an array, is a protocol violation and throws rather than reading as [].
    for (const result of [{}, { agents: "none" }]) {
      await setFakeHerdrBehavior(fake, { malformedResults: { "agent.list": result } });
      const malformed = await client.agentList().catch((error: unknown) => error);
      expect(malformed).toBeInstanceOf(SlpRuntimeError);
      expect((malformed as SlpRuntimeError).code).toBe("HERDR_MALFORMED_RESPONSE");
    }
    await setFakeHerdrBehavior(fake, { malformedResults: {} });
    expect((await client.agentList()).map((agent) => agent.name)).toEqual(["lead-probe"]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
});

test("herdr-client-subscribe: pushed events arrive over one open connection until closed (red 2)", async () => {
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const client = new HerdrClient({ ...process.env, ...fake.env, HOME: fixture.home });
    const workspace = await client.workspaceCreate({ cwd: fixture.repo, label: "probe" });
    const tab = await client.tabCreate({ cwd: fixture.repo, label: "seat", workspace_id: workspace.workspace!.workspace_id });
    const paneId = tab.root_pane!.pane_id;
    const stream = await client.subscribe([
      { pane_id: paneId, type: "pane.agent_status_changed" },
      { type: "pane.closed" },
    ]);
    const iterator = stream.events[Symbol.asyncIterator]();
    expect(await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: paneId, agent_status: "blocked" } })).toBe(1);
    expect(await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: "w9:p9", agent_status: "idle" } })).toBe(0);
    expect(await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: paneId, agent_status: "idle" } })).toBe(1);
    const first = await iterator.next();
    const second = await iterator.next();
    expect([first.value, second.value].map((event) => [event.event, event.data.agent_status])).toEqual([
      ["pane_agent_status_changed", "blocked"],
      ["pane_agent_status_changed", "idle"],
    ]);
    await client.paneClose(paneId);
    const closed = await iterator.next();
    expect(closed.value).toMatchObject({ event: "pane_closed", data: { pane_id: paneId } });
    stream.close();
    expect((await iterator.next()).done).toBe(true);
    const subscribes = (await fakeHerdrCommands(fake)).filter((command) => command[0] === "events");
    expect(subscribes).toEqual([["events", "subscribe", `pane.agent_status_changed:${paneId}`, "pane.closed"]]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
});

test("plugin-manifest: install renders the manifest with the binary path and links it once, a second install is a no-op, uninstall unlinks, the TOML names startup and panes and no events or actions, and only the maestro plugin changes (red 9, A4)", async () => {
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    await editFakeHerdrState(fake, (state) => {
      state.plugins.push({ plugin_id: "local.attention-broker", name: "Attention Broker", plugin_root: "/elsewhere" });
      // A stale maestro link from another home moves to this one.
      state.plugins.push({ plugin_id: "maestro", name: "Maestro", plugin_root: join(fixture.root, "stale-home", "maestro") });
    });
    const environment = { ...fake.env, PATH: [join(fixture.home, ".local", "bin"), ...(fake.env.PATH ?? "").split(":")].join(":"), SHELL: "/bin/zsh" };
    const installed = await runCli(fixture, ["install"], environment);
    expect(installed.exitCode).toBe(0);
    // d833: the room is the plugin root so Herdr's hooks run from the Hub.
    const directory = join(fixture.home, "maestro");
    expect(installed.stdout).toContain(`herdr plugin: linked at ${directory}`);
    const manifest = Bun.TOML.parse(await readFile(join(directory, "herdr-plugin.toml"), "utf8")) as Record<string, any>;
    const binary = join(fixture.home, ".local", "bin", "maestro");
    expect(manifest.id).toBe("maestro");
    expect(manifest.min_herdr_version).toBe("0.8.2");
    expect(manifest.version).toMatch(/^\d+\.\d+\.\d+/);
    expect(manifest.startup).toEqual([{ command: [binary, "slp", "restore"] }]);
    expect(manifest.panes).toEqual([{ id: "runtime", title: "SLP runtime", placement: "split", command: [binary, "slp", "runtime"] }]);
    expect(manifest.events).toBeUndefined();
    expect(manifest.actions).toBeUndefined();
    const links = async () => (await fakeHerdrCommands(fake)).filter((command) => command[0] === "plugin" && command[1] !== "list");
    expect(await links()).toEqual([["plugin", "unlink", "maestro"], ["plugin", "link", directory]]);

    const repeated = await runCli(fixture, ["install"], environment);
    expect(repeated.exitCode).toBe(0);
    expect(repeated.stdout).toContain(`herdr plugin: present at ${directory}`);
    expect(await links()).toEqual([["plugin", "unlink", "maestro"], ["plugin", "link", directory]]);
    expect((await readFakeHerdrState(fake)).plugins.map((plugin: { plugin_id: string }) => plugin.plugin_id))
      .toEqual(["local.attention-broker", "maestro"]);

    const uninstalled = await runCli(fixture, ["uninstall"], environment);
    expect(uninstalled.exitCode).toBe(0);
    expect(await links()).toEqual([["plugin", "unlink", "maestro"], ["plugin", "link", directory], ["plugin", "unlink", "maestro"]]);
    expect(existsSync(join(directory, "herdr-plugin.toml"))).toBe(false);
    expect(existsSync(join(directory, "SLP.md"))).toBe(true);
    expect((await readFakeHerdrState(fake)).plugins.map((plugin: { plugin_id: string }) => plugin.plugin_id))
      .toEqual(["local.attention-broker"]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 30_000);

test("install without Herdr renders the manifest, warns, and completes", async () => {
  await withFixture(async (fixture) => {
    const installed = await runCli(fixture, ["install"], {
      HERDR_SOCKET_PATH: join(fixture.root, "missing.sock"),
      PATH: [dirname(process.execPath), "/usr/bin", "/bin"].join(":"),
      SHELL: "/bin/zsh",
    });
    expect(installed.exitCode).toBe(0);
    expect(installed.stdout).toContain("herdr plugin: not linked (cannot reach Herdr at ");
    expect(installed.stdout).toContain("rerun maestro install with Herdr running");
    expect(existsSync(join(fixture.home, "maestro", "herdr-plugin.toml"))).toBe(true);
  });
}, 30_000);

async function markedRoom(fixture: { home: string; repo: string; root: string }): Promise<string> {
  const room = await scaffoldRoom(fixture.home);
  const marked = await runCliAt(fixture, room, ["room", "mark"], {
    MAESTRO_ROOM_SCAFFOLD: "1",
    MAESTRO_SESSION_NONE: "1",
  });
  expect(marked.exitCode).toBe(0);
  return room;
}

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

function failure(stderr: string): { code: string; message: string } {
  return (JSON.parse(stderr) as { error: { code: string; message: string } }).error;
}

test("team-start-socket: team start issues tab.create, agent.start, agent.prompt and plugin.pane.open with team and generation in env, over the socket only (red 3)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Speak the socket", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    const commands = await fakeHerdrCommands(fake);
    const kinds = commands.map((command) => command.slice(0, 2).join(" "));
    expect(kinds).toContain("tab create");
    expect(kinds).toContain("agent start");
    expect(kinds).toContain("agent prompt");
    const opened = commands.filter((command) => command[0] === "plugin" && command[1] === "pane" && command[2] === "open");
    expect(opened).toHaveLength(1);
    expect(opened[0]).toEqual([
      "plugin", "pane", "open", "--plugin", "maestro", "--entrypoint", "runtime", "--placement", "tab",
      "--workspace", data.team.workspaceId, "--cwd", realpathSync.native(fixture.repo),
      "--env", `MAESTRO_SLP_GENERATION=${data.team.generation}`, "--env", `MAESTRO_SLP_TEAM=${data.team.teamId}`,
      "--no-focus",
    ]);
    const state = await readFakeHerdrState(fake);
    expect(state.plugin_panes).toEqual([expect.objectContaining({ entrypoint: "runtime", pane_id: data.team.runtimePaneId })]);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    expect(
      database.query<{ runtime_pane_id: string }, []>("SELECT runtime_pane_id FROM slp_local_teams").get()?.runtime_pane_id,
    ).toBe(data.team.runtimePaneId);
    database.close();
    expect(await tripwireInvocations(fake)).toEqual([]);

    // Repair (d759) leaves a live runtime pane alone and reopens a missing one.
    const repeated = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Speak the socket", "--json"], fake.env);
    expect(repeated.exitCode).toBe(0);
    expect(envelope<StartedTeam>(repeated.stdout).team.runtimePaneId).toBe(data.team.runtimePaneId);
    await editFakeHerdrState(fake, (edited) => {
      edited.panes = edited.panes.filter((pane: { pane_id: string }) => pane.pane_id !== data.team.runtimePaneId);
      delete edited.processes[data.team.runtimePaneId];
    });
    const repaired = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Speak the socket", "--json"], fake.env);
    expect(repaired.exitCode).toBe(0);
    const reopened = envelope<StartedTeam>(repaired.stdout).team.runtimePaneId;
    expect(reopened).not.toBe(data.team.runtimePaneId);
    expect((await fakeHerdrCommands(fake)).filter((command) => command[0] === "plugin" && command[2] === "open")).toHaveLength(2);
    const hub = envelope<{ teams: Array<{ runtimePane: string }> }>((await runCliAt(fixture, room, ["status", "--json"], fake.env)).stdout);
    expect(hub.teams[0]?.runtimePane).toBe("on");
  });
}, 30_000);

test("watch-gone: install writes no Watch shim, team start opens the runtime plugin pane and no watch tab (red 8)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const environment = { ...fake.env, PATH: [join(fixture.home, ".local", "bin"), ...(fake.env.PATH ?? "").split(":")].join(":"), SHELL: "/bin/zsh" };
    expect((await runCli(fixture, ["install"], environment)).exitCode).toBe(0);
    expect(existsSync(join(fixture.home, ".local", "bin", "maestro-slp-watch"))).toBe(false);
    expect(existsSync(join(import.meta.dir, "..", "src", "plugins", "slp-watch.ts"))).toBe(false);
    expect(existsSync(join(import.meta.dir, "..", "bin", "maestro-slp-watch.ts"))).toBe(false);

    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "No Watch", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    const state = await readFakeHerdrState(fake);
    expect((state.tabs as Array<{ label: string }>).some((tab) => tab.label.includes("watch"))).toBe(false);
    expect((state.panes as Array<{ pane_id: string; plugin_id?: string }>).find((pane) => pane.pane_id === data.team.runtimePaneId)?.plugin_id).toBe("maestro");
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const teamJson = envelope<Record<string, unknown>>(
      (await runCliAt(fixture, fixture.repo, ["status", "--json"], { ...fake.env, HERDR_PANE_ID: lead.paneId })).stdout,
    );
    expect(teamJson).not.toHaveProperty("watch");
    expect(teamJson.runtimePane).toBe("on");
    const runtimeStatus = await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], { ...fake.env, HERDR_PANE_ID: lead.paneId });
    expect(runtimeStatus.exitCode).toBe(0);
    expect(envelope<{ pending: unknown[]; runtimePaneId: string }>(runtimeStatus.stdout)).toMatchObject({ pending: [], runtimePaneId: data.team.runtimePaneId });
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 30_000);

test("kernel-no-herdr: with HERDR_SOCKET_PATH pointing at a missing socket the classic verbs work and team start fails naming the socket (red 11, A2)", async () => {
  await withFixture(async (fixture) => {
    const socket = join(fixture.root, "missing.sock");
    const environment = { HERDR_SOCKET_PATH: socket, PATH: [dirname(process.execPath), "/usr/bin", "/bin"].join(":") };
    const added = await runCli(fixture, ["work", "add", "kernel item", "--json"], environment);
    expect(added.exitCode).toBe(0);
    const id = envelope<{ work: { id: string } }>(added.stdout).work.id;
    expect((await runCli(fixture, ["work", "start", id, "--json"], environment)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", id, "--evidence", "kernel unaffected", "--json"], environment)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "list", "--json"], environment)).exitCode).toBe(0);
    const room = await markedRoom(fixture);
    await materializeSkills(fixture.home, "dev");
    await mkdir(dirname(seatTokenPath(fixture.home)), { recursive: true });
    await writeFile(seatTokenPath(fixture.home), "sk-ant-oat01-kernel-fixture");
    await materializeProfiles(fixture.home, fixture.repo);
    const refused = await runCliAt(fixture, room, ["team", "start", fixture.repo, "No Herdr", "--json"], environment);
    expect(refused.exitCode).toBe(1);
    expect(failure(refused.stderr).code).toBe("HERDR_UNAVAILABLE");
    expect(failure(refused.stderr).message).toContain(socket);
  });
}, 30_000);

interface EntryRow {
  actor: string;
  body: string;
  flag: string | null;
  work_id: string;
}

function runtimeEntries(fixture: { repo: string }): EntryRow[] {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
  try {
    return database
      .query<EntryRow, []>("SELECT work_id, actor, body, flag FROM slp_work_entries WHERE actor = 'runtime' ORDER BY id")
      .all();
  } finally {
    database.close();
  }
}

async function prompts(fake: Awaited<ReturnType<typeof installFakeHerdr>>): Promise<string[][]> {
  return (await fakeHerdrCommands(fake)).filter((command) => command[0] === "agent" && command[1] === "prompt");
}

async function attentionPrompts(fake: Awaited<ReturnType<typeof installFakeHerdr>>): Promise<string[][]> {
  return (await prompts(fake)).filter((command) => /^\[(attention|from runtime)\]/.test(command[3] ?? ""));
}

const hubPane = "hub:p1";

function seedHubPane(fixture: { home: string }) {
  return async (fake: Awaited<ReturnType<typeof installFakeHerdr>>): Promise<Record<string, string>> => {
    await editFakeHerdrState(fake, (state) => {
      state.workspaces.push({ workspace_id: "hub", cwd: fixture.home, label: "maestro" });
      state.tabs.push({ tab_id: "hub:t1", workspace_id: "hub", root_pane_id: hubPane, label: "1" });
      state.panes.push({ pane_id: hubPane, workspace_id: "hub", tab_id: "hub:t1", cwd: fixture.home, label: "maestro" });
      state.agents.push({ name: "hub-claude", pane_id: hubPane, workspace_id: "hub", agent_status: "idle", kind: "claude" });
    });
    return { HERDR_PANE_ID: hubPane };
  };
}

// A team whose runtime pane is a real `maestro slp runtime` child subscribed
// to the fake; resolves once the subscription covers every role pane.
async function startLiveTeam(
  fixture: { home: string; repo: string; root: string },
  behavior: Parameters<typeof installFakeHerdr>[1] = {},
  prepare: (fake: Awaited<ReturnType<typeof installFakeHerdr>>) => Promise<Record<string, string>> = seedHubPane(fixture),
): Promise<{ data: StartedTeam; fake: Awaited<ReturnType<typeof installFakeHerdr>>; room: string; subscribed(): Promise<void> }> {
  const room = await markedRoom(fixture);
  const fake = await installFakeHerdr(fixture, { runtimePane: "spawn", ...behavior });
  const startEnvironment = { ...fake.env, ...(await prepare(fake)) };
  const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Attention rules", "--json"], startEnvironment);
  expect(started.exitCode).toBe(0);
  const data = envelope<StartedTeam>(started.stdout);
  // The runtime discards Herdr's replay after every subscribe; a test emits
  // only once runtime.log shows the latest subscription, covering every role
  // pane, drained.
  const logPath = join(slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation), "runtime.log");
  const subscribed = async () => {
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
  return { data, fake, room, subscribed };
}

test("runtime-stall-line: a silent Peer pane records one stall:silence entry by actor runtime and nudges the Peer and the Hub with the d763 line (red 4)", async () => {
  await withFixture(async (fixture) => {
    const { fake, subscribed } = await startLiveTeam(fixture);
    const lead = (await readFakeHerdrState(fake)).agents.find((agent: { name: string }) => agent.name.startsWith("lead-")) as { name: string; pane_id: string };
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.pane_id };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Peer item", "--to", "peer-stall", "--json"], leadEnvironment);
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    await subscribed();
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id, "--json"], peerEnvironment)).exitCode).toBe(0);
    // room d137: the runtime may name the compact count in the evidence of a
    // nudge it already sends. It is not a nudge kind and fires nothing by
    // itself: the entry below exists because the pane went idle.
    expect(
      (await runCliAt(
        fixture,
        fixture.repo,
        ["hook", "record", "--event", "SessionStart", "--source", "compact", "--harness", "claude"],
        {
          ...peerEnvironment,
          MAESTRO_SESSION_ID: "runtime-compact-session",
          MAESTRO_SESSION_PID: String(process.pid),
        },
      )).exitCode,
    ).toBe(0);
    const before = (await attentionPrompts(fake)).length;
    expect(await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } })).toBe(1);
    await waitForFakeHerdr(() => runtimeEntries(fixture).length === 1, 5_000, "the stall entry");
    const [entry] = runtimeEntries(fixture);
    expect(entry).toMatchObject({ actor: "runtime", flag: "stall:silence", work_id: peer.work.id });
    expect(entry?.body).toContain("agent_status idle");
    expect(entry?.body).toContain(`while ${peer.work.id} ACTIVE owned by ${peer.role.name}`);
    expect(entry?.body).toMatch(/; compacts: 1 \(last \d+[smhd]\)/);
    const evidence = entry?.body.replace(/^silence: /, "");
    const line = `[from runtime][${peer.work.id}] silence ${evidence}; stop and run: maestro work note ${peer.work.id} "<what you need>" --blocked`;
    // w708: the copy is about someone else's item, so it ends at a read and never
    // tells the Hub to record a blocker on an item it does not hold.
    const copy = `[from runtime][${peer.work.id} copy] silence ${evidence}; ${peer.role.name} holds ${peer.work.id} and has been nudged; read: maestro status ${peer.work.id}`;
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).length === before + 2, 5_000, "two nudges");
    expect((await attentionPrompts(fake)).slice(before)).toEqual([
      ["agent", "prompt", peer.role.name, line],
      ["agent", "prompt", hubPane, copy],
    ]);
    expect(copy).not.toContain("--blocked");
    // The same event again changes nothing until the store moves (d763).
    await Bun.sleep(50);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await Bun.sleep(300);
    expect(runtimeEntries(fixture)).toHaveLength(1);
    expect(await attentionPrompts(fake)).toHaveLength(before + 2);
    const shown = await runCliAt(fixture, fixture.repo, ["status", peer.work.id], peerEnvironment);
    expect(shown.stdout).toContain(`note [stall:silence] by runtime: silence: ${peer.role.name} pane ${peer.role.paneId}`);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 40_000);

test("runtime-silence: idle while holding ACTIVE work stalls once until the store changes, a --blocked seat is left alone, an idle seat with nothing to do wakes the seat above unless it just pushed (red 5, A6)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture, { promptEvents: true });
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Peer item", "--to", "peer-quiet", "--json"], leadEnvironment);
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    await subscribed();
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id, "--json"], peerEnvironment)).exitCode).toBe(0);
    const before = (await attentionPrompts(fake)).length;

    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(() => runtimeEntries(fixture).some((entry) => entry.flag === "stall:silence"), 5_000, "the silence entry");
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).length === before + 2, 5_000, "the silence nudge and its copy");
    const nudges = (await attentionPrompts(fake)).slice(before);
    expect(nudges.map((command) => command[2])).toEqual([peer.role.name, hubPane]);
    expect(nudges[0]?.[3]).toMatch(new RegExp(`^\\[from runtime\\]\\[${peer.work.id}\\] silence ${peer.role.name} pane \\S+ agent_status idle while ${peer.work.id} ACTIVE owned by ${peer.role.name}`));
    // A6: the nudge itself turned the Peer working then idle (promptEvents);
    // that second idle is not a second stall.
    await Bun.sleep(400);
    expect(runtimeEntries(fixture).filter((entry) => entry.flag === "stall:silence")).toHaveLength(1);
    expect((await attentionPrompts(fake)).length).toBe(before + 2);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await Bun.sleep(300);
    expect(runtimeEntries(fixture).filter((entry) => entry.flag === "stall:silence")).toHaveLength(1);

    // d761: a seat that declared --blocked is legitimately waiting.
    expect((await runCliAt(fixture, fixture.repo, ["work", "note", peer.work.id, "need the token", "--blocked", "--json"], peerEnvironment)).exitCode).toBe(0);
    const afterBlocked = (await attentionPrompts(fake)).length;
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await Bun.sleep(300);
    expect(runtimeEntries(fixture).filter((entry) => entry.flag === "stall:silence")).toHaveLength(1);
    expect((await attentionPrompts(fake)).length).toBe(afterBlocked);

    // Advisor F15: idle right after the Peer's own work return wakes nobody.
    expect((await runCliAt(fixture, fixture.repo, ["work", "return", peer.work.id, "result: done", "--json"], peerEnvironment)).exitCode).toBe(0);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await Bun.sleep(300);
    expect((await attentionPrompts(fake)).length).toBe(afterBlocked);

    // A Peer whose only item was cancelled has no `*` and never pushed: one
    // [attention] line to the Lead.
    const second = await runCliAt(fixture, fixture.repo, ["work", "add", "Cancelled item", "--to", "peer-empty", "--json"], leadEnvironment);
    expect(second.exitCode).toBe(0);
    const empty = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(second.stdout);
    await subscribed();
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", empty.work.id, "--outcome", "cancelled", "--json"], leadEnvironment)).exitCode).toBe(0);
    const beforeIdle = (await attentionPrompts(fake)).length;
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: empty.role.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).length === beforeIdle + 1, 5_000, "the idle wake");
    expect((await attentionPrompts(fake)).at(-1)).toEqual(["agent", "prompt", lead.name, `[attention] ${empty.role.name} idle`]);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: empty.role.paneId, agent_status: "idle" } });
    await Bun.sleep(300);
    expect((await attentionPrompts(fake)).length).toBe(beforeIdle + 1);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

// w708: age one record in the store so a 60-second window can be crossed without
// a 60-second test. The runtime reads created_at, so this is the same fact the
// clock would have produced. l3 added a second, longer window, so the age is a
// parameter rather than the one constant that used to cross the only window.
function backdate(
  fixture: { repo: string },
  table: "slp_work_entries" | "slp_decisions",
  id: string | number,
  ageMs = 300_000,
): void {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
  try {
    database
      .query(`UPDATE ${table} SET created_at = ? WHERE id = ?`)
      .run(new Date(Date.now() - ageMs).toISOString(), id);
  } finally {
    database.close();
  }
}

function latestDecision(fixture: { repo: string }, workId: string): { actor: string; id: string } {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
  try {
    return database
      .query<{ actor: string; id: string }, [string]>(
        "SELECT id, actor FROM slp_decisions WHERE work_id = ? ORDER BY created_at DESC, id DESC LIMIT 1",
      )
      .get(workId)!;
  } finally {
    database.close();
  }
}

function ownEntries(fixture: { repo: string }, workId: string, actor: string): Array<{ id: number }> {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
  try {
    return database
      .query<{ id: number }, [string, string]>(
        "SELECT id FROM slp_work_entries WHERE work_id = ? AND actor = ? ORDER BY id",
      )
      .all(workId, actor);
  } finally {
    database.close();
  }
}

test("runtime-records-are-life: a decision against the held item is evidence of life - it suppresses the silence stall while fresh and is named in the evidence instead of 'no entries' - and only the holder's own records re-arm the once-rule, never the Hub writing on the item (w708, d867)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startLiveTeam(fixture, { promptEvents: true });
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const run = async (args: string[], environment: Record<string, string>, cwd = fixture.repo) => {
      const result = await runCliAt(fixture, cwd, args, environment);
      expect({ code: result.exitCode, stderr: result.stderr }).toMatchObject({ code: 0 });
      return result;
    };
    // A delivery item the Lead holds: d834 leaves the team card out of this stall.
    const added = await run(["work", "add", "Lead delivery item", "--json"], leadEnvironment);
    const item = envelope<{ work: { id: string } }>(added.stdout).work.id;
    await run(["work", "take", item, "--json"], leadEnvironment);
    const silence = () => runtimeEntries(fixture).filter((entry) => entry.flag === "stall:silence" && entry.work_id === item);

    // The practice this team runs on: record the decision before the code moves.
    await run(["decide", "Shape it this way", "--why", "the narrow fix holds", "--work", item, "--json"], leadEnvironment);
    const decision = latestDecision(fixture, item);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await Bun.sleep(400);
    expect(silence()).toEqual([]);

    // The same decision, no longer fresh: the seat is nudged once, and the
    // evidence names the decision rather than claiming the item has no records.
    backdate(fixture, "slp_decisions", decision.id);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(() => silence().length === 1, 5_000, "the silence entry naming the decision");
    expect(silence()[0]?.body).toContain(`latest decision ${decision.id} by ${lead.name}`);
    expect(silence()[0]?.body).not.toContain("no entries");

    // Supervision does not re-arm the alarm against the seat being supervised.
    await run(["work", "note", item, "carry on, this is a correction not a gap", "--json"], fake.env, room);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await Bun.sleep(400);
    expect(silence()).toHaveLength(1);

    // l3 (w712): the holder answers the nudge with an account of why it is
    // waiting. Fresh, it is life and withholds the nudge. Aged past the 60-second
    // push window it used to re-arm the once-rule and the same nudge landed a
    // second time, which punished the seat for reporting; a record written after
    // the stall entry now quiets that nudge for the whole answered window.
    await run(["work", "note", item, "still working, suite in flight", "--json"], leadEnvironment);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await Bun.sleep(400);
    expect(silence()).toHaveLength(1);
    const own = ownEntries(fixture, item, lead.name);
    // Both records age together, because the account was written after the nudge
    // it answers and the runtime reads created_at for both.
    const age = (noteMs: number) => {
      backdate(fixture, "slp_decisions", decision.id, noteMs + 120_000);
      backdate(fixture, "slp_work_entries", ownEntries(fixture, item, "runtime").at(-1)!.id, noteMs + 60_000);
      backdate(fixture, "slp_work_entries", own.at(-1)!.id, noteMs);
    };
    age(300_000);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await Bun.sleep(400);
    expect(silence()).toHaveLength(1);

    // l3: the withhold does not consume the once-rule mark. Once the account is
    // older than the answered window, the next idle event that arrives still
    // carries the nudge, and the evidence still names the holder's own record.
    // w742 Q1: the event here is injected, and the runtime has no timer that
    // would produce one on its own - this pins the withhold's expiry, not the
    // arrival of a second nudge against a pane that has stopped changing.
    age(20 * 60_000);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(() => silence().length === 2, 5_000, "the second silence entry once the holder's account is stale");
    expect(silence()[1]?.body).toContain(`latest entry NOTE by ${lead.name}`);
  });
}, 60_000);

test("runtime-blocked-holder: the d761 exemption is the holder's own declaration, so neither a decision on the item nor another seat's note masks a standing --blocked, and the holder's own later record still releases it (w708 rework, d761)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startLiveTeam(fixture, { promptEvents: true });
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const run = async (args: string[], environment: Record<string, string>, cwd = fixture.repo) => {
      const result = await runCliAt(fixture, cwd, args, environment);
      expect({ code: result.exitCode, stderr: result.stderr }).toMatchObject({ code: 0 });
      return result;
    };
    const added = await run(["work", "add", "Lead delivery item", "--json"], leadEnvironment);
    const item = envelope<{ work: { id: string } }>(added.stdout).work.id;
    await run(["work", "take", item, "--json"], leadEnvironment);
    const silence = () => runtimeEntries(fixture).filter((entry) => entry.flag === "stall:silence" && entry.work_id === item);
    const idle = async () => {
      await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
      await Bun.sleep(400);
    };

    // The seat declares it is waiting, and the declaration is aged so that the
    // recency withhold cannot be what keeps the nudge away: only d761 can.
    await run(["work", "note", item, "waiting on the token from the owner", "--blocked", "--json"], leadEnvironment);
    backdate(fixture, "slp_work_entries", ownEntries(fixture, item, lead.name).at(-1)!.id);
    await idle();
    expect(silence()).toEqual([]);

    // A decision recorded against the same item carries no flag. It is evidence
    // of life, not a withdrawal of the declaration, so the seat stays exempt.
    await run(["decide", "Shape it this way", "--why", "the narrow fix holds", "--work", item, "--json"], leadEnvironment);
    backdate(fixture, "slp_decisions", latestDecision(fixture, item).id);
    await Bun.sleep(5_100);
    await idle();
    expect(silence()).toEqual([]);

    // Nor is another seat's note this seat's declaration: being written about
    // does not unblock a seat that is still waiting.
    await run(["work", "note", item, "acknowledged, the owner has it", "--json"], fake.env, room);
    await Bun.sleep(5_100);
    await idle();
    expect(silence()).toEqual([]);

    // The holder's own later record does release the exemption: it says the seat
    // is no longer declaring itself blocked, and the detector arms again.
    await run(["work", "note", item, "token arrived, back on it", "--json"], leadEnvironment);
    backdate(fixture, "slp_work_entries", ownEntries(fixture, item, lead.name).at(-1)!.id);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(() => silence().length === 1, 5_000, "the silence entry once the declaration is withdrawn");
    expect(silence()[0]?.body).toContain(`latest entry NOTE by ${lead.name}`);
  });
}, 60_000);

test("runtime-stale-wake: a stall nudge left queued is checked against the store when it finally flushes, and a nudge about an item that has since moved on is dropped rather than delivered (w708, d867)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Peer item", "--to", "peer-busy", "--json"], leadEnvironment);
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    await subscribed();
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id, "--json"], peerEnvironment)).exitCode).toBe(0);

    await setFakeHerdrBehavior(fake, { prompts: false });
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(
      () => runtimeEntries(fixture).some((entry) => entry.flag === "stall:silence" && entry.work_id === peer.work.id),
      5_000,
      "the silence entry",
    );
    const pendingFor = async (target: string) =>
      envelope<{ pending: Array<{ line: string; target: string }> }>(
        (await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], leadEnvironment)).stdout,
      ).pending.filter((line) => line.target === target && line.line.includes(`[${peer.work.id}] silence`));
    await waitForFakeHerdr(async () => (await pendingFor(peer.role.name)).length === 1, 5_000, "the queued nudge");
    const copyPending = async () =>
      envelope<{ pending: Array<{ line: string; target: string }> }>(
        (await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], leadEnvironment)).stdout,
      ).pending.filter((line) => line.target === hubPane && line.line.includes(`[${peer.work.id} copy]`));
    await waitForFakeHerdr(async () => (await copyPending()).length === 1, 5_000, "the queued Hub copy");
    const nudgesToHolder = async () =>
      (await attentionPrompts(fake)).filter((command) => command[2] === peer.role.name && (command[3] ?? "").includes(`[${peer.work.id}] silence`));
    const attempted = (await nudgesToHolder()).length;
    const copiesToHub = async () =>
      (await attentionPrompts(fake)).filter((command) => command[2] === hubPane && (command[3] ?? "").includes(`${peer.work.id} copy`));
    const copiesAttempted = (await copiesToHub()).length;

    expect((await runCliAt(fixture, fixture.repo, ["work", "return", peer.work.id, "result: done", "--json"], peerEnvironment)).exitCode).toBe(0);
    await setFakeHerdrBehavior(fake, { prompts: true });
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(async () => (await pendingFor(peer.role.name)).length === 0, 5_000, "the stale nudge leaving the queue");
    expect((await nudgesToHolder()).length).toBe(attempted);
    const log = await readFile(join(slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation), "runtime.log"), "utf8");
    expect(log).toContain(`stale wake about ${peer.work.id} silence dropped: ${peer.work.id} is RETURNED`);

    // Only a new wake to the Hub flushes the Hub's queue, so a pane loss carries the copy to its delivery check.
    await emitFakeHerdrEvent(fake, { event: "pane_exited", data: { pane_id: peer.role.paneId, workspace_id: data.team.workspaceId } });
    await waitForFakeHerdr(
      async () => (await attentionPrompts(fake)).some((command) => command[2] === hubPane && command[3] === `[attention] ${peer.role.name} pane exited`),
      5_000,
      "the pane-loss wake to the Hub",
    );
    await waitForFakeHerdr(async () => (await copyPending()).length === 0, 5_000, "the stale copy leaving the queue");
    expect((await copiesToHub()).length).toBe(copiesAttempted);
    const flushedLog = await readFile(join(slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation), "runtime.log"), "utf8");
    expect(flushedLog).toContain(`stale wake about ${peer.work.id} silence copy dropped: ${peer.work.id} is RETURNED`);
  });
}, 60_000);

test("runtime-queue: a wake for a working target waits for its idle, a failed prompt stays queued and slp status lists it, identical events within 5 s deliver once (red 6)", async () => {
  await withFixture(async (fixture) => {
    const { fake, subscribed } = await startLiveTeam(fixture);
    const lead = (await readFakeHerdrState(fake)).agents.find((agent: { name: string }) => agent.name.startsWith("lead-")) as { name: string; pane_id: string };
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.pane_id };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Cancelled item", "--to", "peer-wait", "--json"], leadEnvironment);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    await subscribed();
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", peer.work.id, "--outcome", "cancelled", "--json"], leadEnvironment)).exitCode).toBe(0);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.pane_id, agent_status: "working" } });
    await Bun.sleep(200);
    const before = (await attentionPrompts(fake)).length;
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(async () => {
      const status = await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], leadEnvironment);
      return envelope<{ pending: Array<{ target: string }> }>(status.stdout).pending.some((line) => line.target === lead.name);
    }, 5_000, "the queued wake");
    const queued = envelope<{ pending: Array<{ line: string; queuedAt: string; subject: string; target: string }>; runtime: { state: string } }>(
      (await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], leadEnvironment)).stdout,
    );
    expect(queued.runtime.state).toBe("running");
    expect(queued.pending).toEqual([{ line: `[attention] ${peer.role.name} idle`, queuedAt: expect.any(String), subject: `${peer.role.name} idle`, target: lead.name }]);
    expect((await attentionPrompts(fake)).length).toBe(before);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.pane_id, agent_status: "idle" } });
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).length === before + 1, 5_000, "the flushed wake");
    expect((await attentionPrompts(fake)).at(-1)).toEqual(["agent", "prompt", lead.name, `[attention] ${peer.role.name} idle`]);
    const text = (await runCliAt(fixture, fixture.repo, ["slp", "status"], leadEnvironment)).stdout;
    expect(text).toContain("pending: none");

    // A failed prompt stays queued until the target's next idle.
    const second = await runCliAt(fixture, fixture.repo, ["work", "add", "Second cancelled item", "--to", "peer-wait-again", "--json"], leadEnvironment);
    const again = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(second.stdout);
    await subscribed();
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", again.work.id, "--outcome", "cancelled", "--json"], leadEnvironment)).exitCode).toBe(0);
    await setFakeHerdrBehavior(fake, { prompts: false });
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: again.role.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(async () => {
      const status = await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], leadEnvironment);
      return envelope<{ pending: Array<{ line: string; target: string }> }>(status.stdout).pending
        .some((line) => line.target === lead.name && line.line === `[attention] ${again.role.name} idle`);
    }, 5_000, "the queued failed wake");
    await setFakeHerdrBehavior(fake, { prompts: true });
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.pane_id, agent_status: "idle" } });
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).some((command) => command[2] === lead.name && command[3] === `[attention] ${again.role.name} idle`), 5_000, "the retried wake");
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

// Live row 17 (lab g18): a wake for the Hub targeted the literal Herdr agent
// `supervisor`, which nothing had recorded, and stayed pending forever with
// agent_not_found. team start records the Hub pane that ran it.
test("runtime-hub: team start records the Hub pane that ran it; with no recorded pane and no supervisor agent a wake for the Hub is dropped with a logged reason and slp status lists it as unreachable, not pending", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const recorded = envelope<{ supervisorPaneId: string | null }>(
      (await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], leadEnvironment)).stdout,
    );
    expect(recorded.supervisorPaneId).toBe(hubPane);
    // The suite may itself run inside a Herdr pane (d770 ancestor walk): the
    // recorded fact is cleared so this half tests the no-pane case.
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      database.query("UPDATE slp_local_teams SET supervisor_pane_id = '' WHERE team_id = ? AND generation = ?").run(data.team.teamId, data.team.generation);
    } finally {
      database.close();
    }
    const before = (await attentionPrompts(fake)).length;
    await emitFakeHerdrEvent(fake, { event: "pane_exited", data: { pane_id: lead.paneId, workspace_id: data.team.workspaceId } });
    const logPath = join(slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation), "runtime.log");
    await waitForFakeHerdr(async () => (await readFile(logPath, "utf8")).includes("unreachable"), 5_000, "the dropped wake");
    const dropped = (await readFile(logPath, "utf8")).split("\n").find((line) => line.includes("unreachable"))!;
    expect(dropped).toContain(`${lead.name} pane exited`);
    expect(dropped).toContain("no Hub Supervisor pane recorded");
    expect(dropped).toContain("supervisor");
    const status = envelope<{ pending: unknown[]; unreachable: Array<{ droppedAt: string; line: string; reason: string; subject: string }> }>(
      (await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], leadEnvironment)).stdout,
    );
    expect(status.pending).toEqual([]);
    expect(status.unreachable).toEqual([{ line: `[attention] ${lead.name} pane exited`, droppedAt: expect.any(String), reason: expect.stringContaining("no Hub Supervisor pane recorded"), subject: `${lead.name} pane exited` }]);
    const text = (await runCliAt(fixture, fixture.repo, ["slp", "status"], leadEnvironment)).stdout;
    expect(text).toContain(`unreachable ${lead.name} pane exited:`);
    expect(text).not.toContain("pending supervisor");
    expect((await attentionPrompts(fake)).length).toBe(before);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

// d898: the Hub reviews the Lead, so every wake about the Lead - the copy of
// its stall nudge, its pane loss and its idle with nothing to do - reaches the
// Hub pane recorded at team start instead of being dropped as unreachable.
test("runtime-lead-wakes-hub: a Lead's stall copy, pane loss and idle wake each reach the Hub pane (d898)", async () => {
  await withFixture(async (fixture) => {
    const hubPane = "hub:p1";
    const { data, fake, subscribed } = await startLiveTeam(fixture, {}, async (seeded) => {
      await editFakeHerdrState(seeded, (state) => {
        state.workspaces.push({ workspace_id: "hub", cwd: fixture.home, label: "maestro" });
        state.tabs.push({ tab_id: "hub:t1", workspace_id: "hub", root_pane_id: hubPane, label: "1" });
        state.panes.push({ pane_id: hubPane, workspace_id: "hub", tab_id: "hub:t1", cwd: fixture.home, label: "maestro" });
        state.agents.push({ name: "hub-claude", pane_id: hubPane, workspace_id: "hub", agent_status: "idle", kind: "claude" });
      });
      return { HERDR_PANE_ID: hubPane };
    });
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const hubPrompts = async () => (await attentionPrompts(fake)).filter((command) => command[2] === hubPane);

    // Idle with nothing to do: the Lead's item waits on the Hub's review, and its
    // own return is old enough not to count as the push that just woke someone.
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", data.work.id], leadEnvironment)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "return", data.work.id, "done"], leadEnvironment)).exitCode).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      database.query("UPDATE slp_activity SET created_at = ? WHERE actor = ?").run(new Date(Date.now() - 300_000).toISOString(), lead.name);
    } finally {
      database.close();
    }
    await subscribed();
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(async () => (await hubPrompts()).length === 1, 5_000, "the Lead idle wake at the Hub");
    expect((await hubPrompts())[0]).toEqual(["agent", "prompt", hubPane, `[attention] ${lead.name} idle`]);

    // Silence: the Lead holds an item of its own ACTIVE and goes idle; it is
    // nudged and the Hub gets the copy.
    const own = await runCliAt(fixture, fixture.repo, ["work", "add", "Own item", "--json"], leadEnvironment);
    expect(own.exitCode).toBe(0);
    const ownId = envelope<{ work: { id: string } }>(own.stdout).work.id;
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", ownId], leadEnvironment)).exitCode).toBe(0);
    await Bun.sleep(5_100);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(async () => (await hubPrompts()).length === 2, 5_000, "the Lead stall copy at the Hub");
    expect((await hubPrompts())[1]?.[3]).toStartWith(`[from runtime][${ownId} copy] silence ${lead.name} pane`);
    expect((await attentionPrompts(fake)).some((command) =>
      command[2] === lead.name && (command[3] ?? "").startsWith(`[from runtime][${ownId}] silence`)
    )).toBe(true);

    // Pane loss.
    await emitFakeHerdrEvent(fake, { event: "pane_exited", data: { pane_id: lead.paneId, workspace_id: data.team.workspaceId } });
    await waitForFakeHerdr(async () => (await hubPrompts()).length === 3, 5_000, "the Lead pane loss at the Hub");
    expect((await hubPrompts())[2]).toEqual(["agent", "prompt", hubPane, `[attention] ${lead.name} pane exited`]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

test("runtime-exit: a Peer pane exit is recorded on the team card and wakes the Hub (red 7)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Peer item", "--to", "peer-gone", "--json"], leadEnvironment);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    await subscribed();
    await emitFakeHerdrEvent(fake, { event: "pane_exited", data: { pane_id: peer.role.paneId, workspace_id: data.team.workspaceId } });
    await waitForFakeHerdr(() => runtimeEntries(fixture).length === 1, 5_000, "the loss entry");
    expect(runtimeEntries(fixture)[0]).toMatchObject({ flag: "pane:exited", work_id: data.work.id });
    expect(runtimeEntries(fixture)[0]?.body).toContain(`exited: ${peer.role.name} pane ${peer.role.paneId}`);
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).length === 1, 5_000, "the Hub wake");
    expect((await attentionPrompts(fake))[0]).toEqual(["agent", "prompt", hubPane, `[attention] ${peer.role.name} pane exited`]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

test("restore: a RUNNING generation with live role panes gets its runtime pane reopened and subscribed; one whose panes are gone is noted as lost and nothing opens (red 10)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Restore me", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    // Herdr came back without the plugin pane: the recorded pane is gone.
    await editFakeHerdrState(fake, (state) => {
      state.panes = state.panes.filter((pane: { pane_id: string }) => pane.pane_id !== data.team.runtimePaneId);
      delete state.processes[data.team.runtimePaneId];
      state.behavior.runtimePane = "spawn";
    });
    const restored = await runCliAt(fixture, room, ["slp", "restore", "--json"], fake.env);
    expect(restored.exitCode).toBe(0);
    const outcome = envelope<{ generations: Array<{ generation: string; outcome: string; paneId?: string }> }>(restored.stdout).generations;
    expect(outcome).toEqual([{ generation: `${data.team.teamId}:g${data.team.generation}`, outcome: "reopened", paneId: expect.any(String) }]);
    expect(outcome[0]?.paneId).not.toBe(data.team.runtimePaneId);
    await waitForFakeHerdr(async () => (await readFakeHerdrState(fake)).subscriptions.length === 1, 10_000, "the restored subscription");
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    expect(database.query<{ runtime_pane_id: string }, []>("SELECT runtime_pane_id FROM slp_local_teams").get()?.runtime_pane_id).toBe(outcome[0]?.paneId);
    database.close();
    // A second restore sees the live runtime lock and opens nothing.
    const again = await runCliAt(fixture, room, ["slp", "restore", "--json"], fake.env);
    expect(envelope<{ generations: Array<{ outcome: string }> }>(again.stdout).generations[0]?.outcome).toBe("running");
    expect((await fakeHerdrCommands(fake)).filter((command) => command[0] === "plugin" && command[2] === "open")).toHaveLength(2);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });

  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Lose me", "--json"], fake.env);
    const data = envelope<StartedTeam>(started.stdout);
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    await editFakeHerdrState(fake, (state) => {
      state.agents = [];
      state.panes = [];
      state.processes = {};
    });
    const opens = async () => (await fakeHerdrCommands(fake)).filter((command) => command[0] === "plugin" && command[2] === "open");
    const before = (await opens()).length;
    const restored = await runCliAt(fixture, room, ["slp", "restore", "--json"], fake.env);
    expect(restored.exitCode).toBe(0);
    expect(envelope<{ generations: Array<{ generation: string; outcome: string }> }>(restored.stdout).generations).toEqual([
      { generation: `${data.team.teamId}:g${data.team.generation}`, outcome: "lost" },
    ]);
    expect((await opens()).length).toBe(before);
    expect(runtimeEntries(fixture)).toEqual([expect.objectContaining({ flag: "pane:lost", work_id: data.work.id })]);
    expect(runtimeEntries(fixture)[0]?.body).toContain("survived the Herdr restart");
    // Restore again: the loss is noted once.
    await runCliAt(fixture, room, ["slp", "restore", "--json"], fake.env);
    expect(runtimeEntries(fixture)).toHaveLength(1);
    // w713, d870: restore NEVER commits, however total the loss looks. At
    // Herdr startup an agent.list that answers successfully but empty is
    // indistinguishable from genuine loss, so restore records and stops
    // there. Every seat is gone here and the generation is still
    // RUNNING with no STOP row at all.
    const projectAfterRestore = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const roomAfterRestore = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(
      projectAfterRestore
        .query<{ state: string }, []>("SELECT state FROM slp_local_teams")
        .get()?.state,
    ).toBe("RUNNING");
    expect(roomAfterRestore.query<{ state: string }, []>("SELECT state FROM slp_teams").get()?.state).toBe("RUNNING");
    expect(
      projectAfterRestore
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_lifecycle_operations WHERE operation = 'STOP'")
        .get()?.count,
    ).toBe(0);
    expect(
      projectAfterRestore
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work WHERE abandoned_at IS NOT NULL")
        .get()?.count,
    ).toBe(0);
    projectAfterRestore.close();
    roomAfterRestore.close();
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

test("start-before-active: a start whose agent is not yet listed as active waits for it, and a first prompt answered agent_not_ready is retried; the team still starts (live 2026-09-05)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { agentActivationDelayReads: 3, promptNotReadyAttempts: 1, runtimePane: "record" });
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Prompt before active", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    expect(data.team.roles.map((role) => role.role)).toEqual(["lead"]);
    const contractPrompts = (await prompts(fake)).filter((command) => /^slp team /.test(command[3] ?? ""));
    // The Lead's first prompt failed and was retried.
    expect(contractPrompts.map((command) => command[2])).toEqual([`lead-${data.team.teamId}`, `lead-${data.team.teamId}`]);
    const state = await readFakeHerdrState(fake);
    expect(state.agents.map((agent: { agent_status: string }) => agent.agent_status)).toEqual(["idle"]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 30_000);

test("peer-after-subscribe: a Peer opened by work add --to after the runtime subscribed is watched; idle on its pane while holding its item records one stall:silence entry and the d763 prompt, with Herdr replaying history on every subscribe (live g12, 2026-09-05)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Late peer item", "--to", "peer-late", "--json"], leadEnvironment);
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    await subscribed();
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id, "--json"], peerEnvironment)).exitCode).toBe(0);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "working" } });
    await Bun.sleep(200);
    const silencePrompts = async () => (await attentionPrompts(fake)).filter((command) => (command[3] ?? "").includes("] silence "));
    expect(await silencePrompts()).toEqual([]);
    expect(await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } })).toBe(1);
    await waitForFakeHerdr(() => runtimeEntries(fixture).some((entry) => entry.flag === "stall:silence"), 8_000, "the stall entry for the late Peer");
    const entries = runtimeEntries(fixture).filter((entry) => entry.flag === "stall:silence");
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({ actor: "runtime", work_id: peer.work.id });
    expect(entries[0]?.body).toContain(`${peer.role.name} pane ${peer.role.paneId} agent_status idle while ${peer.work.id} ACTIVE owned by ${peer.role.name}`);
    await waitForFakeHerdr(async () => (await silencePrompts()).length === 2, 8_000, "the nudge and its copy");
    const nudges = await silencePrompts();
    expect(nudges.map((command) => command[2])).toEqual([peer.role.name, hubPane]);
    expect(nudges[0]?.[3]).toMatch(new RegExp(`^\\[from runtime\\]\\[${peer.work.id}\\] silence .*; stop and run: maestro work note ${peer.work.id} "<what you need>" --blocked$`));
    // The runtime resubscribed with the late Peer's pane after discarding the replay.
    const log = await readFile(join(slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation), "runtime.log"), "utf8");
    const latest = log.split("\n").filter((line) => line.includes(" subscribed: ")).at(-1) ?? "";
    expect(latest).toContain(`pane.agent_status_changed:${peer.role.paneId}`);
    expect(log.match(/replay drained: \d+ events discarded/g)?.length).toBeGreaterThanOrEqual(2);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

test("live-event-names: a live pane.agent_status_changed with the dotted wire name records the stall, and the dotted pane.created, pane.agent_detected, pane.exited and pane.closed are handled (live g14, 2026-09-05)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    // The fake pushes every live event under its dotted name, as Herdr does.
    const client = new HerdrClient({ ...process.env, ...fake.env, HOME: fixture.home });
    const stream = await client.subscribe([{ pane_id: lead.paneId, type: "pane.agent_status_changed" }]);
    const iterator = stream.events[Symbol.asyncIterator]();
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "working" } });
    const raw = await iterator.next();
    expect(raw.value.event).toBe("pane_agent_status_changed");
    stream.close();
    const wire = (await fakeHerdrCommands(fake)).length;
    void wire;

    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Dotted item", "--to", "peer-dot", "--json"], leadEnvironment);
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    await subscribed();
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id, "--json"], peerEnvironment)).exitCode).toBe(0);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "working" } });
    await Bun.sleep(200);
    expect(await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } })).toBe(1);
    await waitForFakeHerdr(() => runtimeEntries(fixture).some((entry) => entry.flag === "stall:silence" && entry.work_id === peer.work.id), 8_000, "the stall from the live-shaped event");
    const silencePrompts = async () => (await attentionPrompts(fake)).filter((command) => (command[3] ?? "").includes("] silence "));
    await waitForFakeHerdr(async () => (await silencePrompts()).length === 2, 8_000, "the nudge and its copy");
    expect((await silencePrompts()).map((command) => command[2])).toEqual([peer.role.name, hubPane]);

    // The other four dotted kinds: exited and closed record the loss, created
    // and detected are watched.
    await emitFakeHerdrEvent(fake, { event: "pane_exited", data: { pane_id: peer.role.paneId, workspace_id: data.team.workspaceId } });
    await waitForFakeHerdr(() => runtimeEntries(fixture).some((entry) => entry.flag === "pane:exited"), 8_000, "the exited entry");
    await emitFakeHerdrEvent(fake, { event: "pane_closed", data: { pane_id: lead.paneId, workspace_id: data.team.workspaceId } });
    await waitForFakeHerdr(() => runtimeEntries(fixture).some((entry) => entry.flag === "pane:closed"), 8_000, "the closed entry");
    const logPath = join(slpRuntimeDirectory(fixture.repo, data.team.teamId, data.team.generation), "runtime.log");
    await emitFakeHerdrEvent(fake, { event: "pane_created", data: { pane: { pane_id: `${data.team.workspaceId}:p77`, workspace_id: data.team.workspaceId, tab_id: `${data.team.workspaceId}:t77` } } });
    await waitForFakeHerdr(
      async () => (await readFile(logPath, "utf8")).includes(`watching new team pane ${data.team.workspaceId}:p77 (pane_created)`),
      8_000,
      "the created pane watched",
    );
    // Each new pane costs a resubscribe and a replay drain; the next event
    // goes in once that settled.
    await subscribed();
    await emitFakeHerdrEvent(fake, { event: "pane_agent_detected", data: { agent: "claude", pane_id: `${data.team.workspaceId}:p78`, workspace_id: data.team.workspaceId, released: false } });
    await waitForFakeHerdr(
      async () => (await readFile(logPath, "utf8")).includes(`watching new team pane ${data.team.workspaceId}:p78 (pane_agent_detected)`),
      8_000,
      "the detected pane watched",
    );
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

test("team-card-silence: a Lead idle while holding only the team card records no stall, a Peer idle holding a delivery item still does (d834, live g15 2026-09-05)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", data.work.id, "--json"], leadEnvironment)).exitCode).toBe(0);
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Delivery item", "--to", "peer-card", "--json"], leadEnvironment);
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    await subscribed();
    // The Lead's ordinary wait on its Peer: idle, then done, holding w1 only.
    for (const status of ["working", "idle", "working", "done"]) {
      await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: status } });
      await Bun.sleep(150);
    }
    await Bun.sleep(400);
    expect(runtimeEntries(fixture).filter((entry) => entry.flag === "stall:silence")).toEqual([]);
    // A Peer idle while holding its delivery item is still silent.
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id, "--json"], peerEnvironment)).exitCode).toBe(0);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "working" } });
    await Bun.sleep(150);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: peer.role.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(() => runtimeEntries(fixture).some((entry) => entry.flag === "stall:silence" && entry.work_id === peer.work.id), 8_000, "the silence stall on the delivery item");
    expect(runtimeEntries(fixture).filter((entry) => entry.flag === "stall:silence").map((entry) => entry.work_id)).toEqual([peer.work.id]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

// w736 F5: every room verb printed the protocol warning because the constant
// still named the Herdr maestro was built against in early September while the
// installed Herdr had moved to 0.9.0. The evidence is that Herdr's own bundled
// schema, `herdr api schema --json`, declares protocol 22 and lists every
// method this client calls, so the warning was noise on every line the Hub
// read. The fixture speaks the same protocol the client targets, which is what
// makes a mismatch in a test a deliberate one.
test("w736 the protocol constant names the Herdr the fixture speaks, so a matched server warns about nothing", async () => {
  expect(herdrProtocol).toBe(22);
  await withFixture(async (fixture) => {
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    expect((await readFakeHerdrState(fake)).protocol).toBe(herdrProtocol);
    const stderr = captureStderr();
    let warned: string;
    try {
      const client = new HerdrClient({ ...process.env, ...fake.env, HOME: fixture.home });
      await client.request<{ agents: unknown[] }>("agent.list");
    } finally {
      warned = stderr.restore();
    }
    expect(warned).not.toContain("built for protocol");
  });
});
