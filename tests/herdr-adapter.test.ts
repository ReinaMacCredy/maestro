import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync, realpathSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { HerdrClient, SlpRuntimeError, herdrProtocol } from "../src/plugins/herdr-client.ts";
import { materializeProfiles } from "../src/plugins/profiles.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
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

test("plugin-manifest: install renders the manifest with the binary path and links it once, a second install is a no-op, uninstall unlinks, the TOML names startup, panes, events and no actions, and only the maestro plugin changes (red 9, A4)", async () => {
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
    expect(manifest.events).toEqual([
      { on: "pane.exited", command: [binary, "slp", "event"] },
      { on: "pane.closed", command: [binary, "slp", "event"] },
    ]);
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
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    expect(opened[0]).toEqual([
      "plugin", "pane", "open", "--plugin", "maestro", "--entrypoint", "runtime", "--placement", "split",
      "--target-pane", supervisor.paneId, "--cwd", realpathSync.native(fixture.repo),
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

test("watch-gone: install writes no Watch shim, team start opens the runtime plugin pane and no watch tab, work note --stall from a role pane is still refused (red 8)", async () => {
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
    const stall = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", data.work.id, "looks stuck", "--stall", "silence", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead.paneId },
    );
    expect(stall.exitCode).toBe(1);
    expect(failure(stall.stderr).code).toBe("STALL_RETIRED");
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
    const added = await runCli(fixture, ["work", "add", "kernel item", "--atomic-reason", "one edit", "--json"], environment);
    expect(added.exitCode).toBe(0);
    const id = envelope<{ work: { id: string } }>(added.stdout).work.id;
    expect((await runCli(fixture, ["work", "start", id, "--json"], environment)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", id, "--evidence", "kernel unaffected", "--json"], environment)).exitCode).toBe(0);
    expect((await runCli(fixture, ["ready", "--json"], environment)).exitCode).toBe(0);
    const room = await markedRoom(fixture);
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

// A team whose runtime pane is a real `maestro slp runtime` child subscribed
// to the fake; resolves once the subscription covers every role pane.
async function startLiveTeam(
  fixture: { home: string; repo: string; root: string },
  behavior: Parameters<typeof installFakeHerdr>[1] = {},
): Promise<{ data: StartedTeam; fake: Awaited<ReturnType<typeof installFakeHerdr>>; room: string; subscribed(): Promise<void> }> {
  const room = await markedRoom(fixture);
  const fake = await installFakeHerdr(fixture, { runtimePane: "spawn", ...behavior });
  const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Attention rules", "--json"], fake.env);
  expect(started.exitCode).toBe(0);
  const data = envelope<StartedTeam>(started.stdout);
  const subscribed = async () => {
    await waitForFakeHerdr(async () => {
      const state = await readFakeHerdrState(fake);
      const roles = (state.agents as Array<{ pane_id: string }>).map((agent) => agent.pane_id);
      return (state.subscriptions as Array<Array<{ pane_id?: string }>>).some((subscription) =>
        roles.every((pane) => subscription.some((entry) => entry.pane_id === pane))
      );
    }, 10_000, "the runtime subscription to every role pane");
  };
  await subscribed();
  return { data, fake, room, subscribed };
}

test("runtime-blocked: a blocked Lead pane records one stall:dialog entry by actor runtime and nudges the Lead and the Team Supervisor with the d763 line (red 4)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    expect(await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "blocked" } })).toBe(1);
    await waitForFakeHerdr(() => runtimeEntries(fixture).length === 1, 5_000, "the stall entry");
    const [entry] = runtimeEntries(fixture);
    expect(entry).toMatchObject({ actor: "runtime", flag: "stall:dialog", work_id: data.work.id });
    expect(entry?.body).toContain("agent_status blocked");
    expect(entry?.body).toContain(`store: ${data.work.id} OPEN assigned to ${lead.name}`);
    const line = `[from runtime][${data.work.id}] dialog ${entry?.body.replace(/^dialog: /, "")}; stop and run: maestro work note ${data.work.id} "<what you need>" --blocked`;
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).length === 2, 5_000, "two nudges");
    expect(await attentionPrompts(fake)).toEqual([
      ["agent", "prompt", lead.name, line],
      ["agent", "prompt", supervisor.name, line],
    ]);
    // The same event again changes nothing until the store moves (d763).
    await Bun.sleep(50);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "blocked" } });
    await Bun.sleep(300);
    expect(runtimeEntries(fixture)).toHaveLength(1);
    expect(await attentionPrompts(fake)).toHaveLength(2);
    const shown = await runCliAt(fixture, fixture.repo, ["status", data.work.id], { ...fake.env, HERDR_PANE_ID: supervisor.paneId });
    expect(shown.stdout).toContain(`note [stall:dialog] by runtime: dialog: ${lead.name} pane ${lead.paneId}`);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 40_000);

test("runtime-silence: idle while holding ACTIVE work stalls once until the store changes, a --blocked seat is left alone, an idle seat with nothing to do wakes the seat above unless it just pushed (red 5, A6)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture, { promptEvents: true });
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
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
    expect(nudges.map((command) => command[2])).toEqual([peer.role.name, supervisor.name]);
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

test("runtime-queue: a wake for a working target waits for its idle, a failed prompt stays queued and slp status lists it, identical events within 5 s deliver once (red 6)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Cancelled item", "--to", "peer-wait", "--json"], leadEnvironment);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    await subscribed();
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", peer.work.id, "--outcome", "cancelled", "--json"], leadEnvironment)).exitCode).toBe(0);
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "working" } });
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
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: lead.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).length === before + 1, 5_000, "the flushed wake");
    expect((await attentionPrompts(fake)).at(-1)).toEqual(["agent", "prompt", lead.name, `[attention] ${peer.role.name} idle`]);
    const text = (await runCliAt(fixture, fixture.repo, ["slp", "status"], leadEnvironment)).stdout;
    expect(text).toContain("pending: none");

    // A failed prompt stays queued until the target's next idle.
    await setFakeHerdrBehavior(fake, { prompts: false });
    await emitFakeHerdrEvent(fake, { event: "pane_exited", data: { pane_id: peer.role.paneId, workspace_id: data.team.workspaceId } });
    await waitForFakeHerdr(async () => {
      const status = await runCliAt(fixture, fixture.repo, ["slp", "status", "--json"], leadEnvironment);
      return envelope<{ pending: Array<{ target: string }> }>(status.stdout).pending.some((line) => line.target === supervisor.name);
    }, 5_000, "the queued failed wake");
    await setFakeHerdrBehavior(fake, { prompts: true });
    await emitFakeHerdrEvent(fake, { event: "pane_agent_status_changed", data: { pane_id: supervisor.paneId, agent_status: "idle" } });
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).some((command) => command[2] === supervisor.name && command[3] === `[attention] ${peer.role.name} pane exited`), 5_000, "the retried wake");
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

test("runtime-exit: a Peer pane exit is recorded on the team card and wakes the Team Supervisor; the [[events]] command path does the same with no runtime (red 7)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, subscribed } = await startLiveTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const added = await runCliAt(fixture, fixture.repo, ["work", "add", "Peer item", "--to", "peer-gone", "--json"], leadEnvironment);
    const peer = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(added.stdout);
    await subscribed();
    await emitFakeHerdrEvent(fake, { event: "pane_exited", data: { pane_id: peer.role.paneId, workspace_id: data.team.workspaceId } });
    await waitForFakeHerdr(() => runtimeEntries(fixture).length === 1, 5_000, "the loss entry");
    expect(runtimeEntries(fixture)[0]).toMatchObject({ flag: "pane:exited", work_id: data.work.id });
    expect(runtimeEntries(fixture)[0]?.body).toContain(`exited: ${peer.role.name} pane ${peer.role.paneId}`);
    await waitForFakeHerdr(async () => (await attentionPrompts(fake)).length === 1, 5_000, "the supervisor wake");
    expect((await attentionPrompts(fake))[0]).toEqual(["agent", "prompt", supervisor.name, `[attention] ${peer.role.name} pane exited`]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });

  // The safety net: no runtime subscribed, Herdr runs `maestro slp event`.
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Hook path", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<StartedTeam>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const hooked = await runCliAt(fixture, room, ["slp", "event", "--json"], {
      ...fake.env,
      HERDR_PLUGIN_EVENT: "pane.closed",
      HERDR_PLUGIN_EVENT_JSON: JSON.stringify({ event: "pane_closed", data: { pane_id: lead.paneId, workspace_id: data.team.workspaceId } }),
    });
    expect(hooked.exitCode).toBe(0);
    expect(envelope<{ handled: boolean; seat: string }>(hooked.stdout)).toEqual({ handled: true, seat: lead.name });
    expect(runtimeEntries(fixture)).toEqual([expect.objectContaining({ flag: "pane:closed", work_id: data.work.id })]);
    expect(await attentionPrompts(fake)).toEqual([["agent", "prompt", supervisor.name, `[attention] ${lead.name} pane closed`]]);
    const unknown = await runCliAt(fixture, room, ["slp", "event", "--json"], {
      ...fake.env,
      HERDR_PLUGIN_EVENT_JSON: JSON.stringify({ event: "pane_closed", data: { pane_id: "w9:p9", workspace_id: "w9" } }),
    });
    expect(unknown.exitCode).toBe(0);
    expect(envelope<{ handled: boolean }>(unknown.stdout).handled).toBe(false);
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
    expect(data.team.roles.map((role) => role.role)).toEqual(["team-supervisor", "lead"]);
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const contractPrompts = (await prompts(fake)).filter((command) => /^slp team /.test(command[3] ?? ""));
    // The Supervisor's first prompt failed and was retried; the Lead's landed once.
    expect(contractPrompts.map((command) => command[2])).toEqual([supervisor.name, supervisor.name, `lead-${data.team.teamId}`]);
    const state = await readFakeHerdrState(fake);
    expect(state.agents.map((agent: { agent_status: string }) => agent.agent_status)).toEqual(["idle", "idle"]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 30_000);
