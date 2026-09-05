import { expect, test } from "bun:test";
import { HerdrClient, SlpRuntimeError, herdrProtocol } from "../src/plugins/herdr-client.ts";
import { withFixture } from "./helpers.ts";
import {
  editFakeHerdrState,
  emitFakeHerdrEvent,
  fakeHerdrCommands,
  installFakeHerdr,
  tripwireInvocations,
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
