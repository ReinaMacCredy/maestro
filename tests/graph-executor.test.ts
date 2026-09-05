import { expect, test } from "bun:test";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, failure, graphNext, graphRun, passthroughGraph, writeGraph, writeProfile } from "./graph-helpers.ts";

// The team-pane cases of red 18 (a Lead pane of a RUNNING team reports team,
// graph result --work binds a node) belong to the second close (d100).
test("graph-executor: a plain session reports executor subagent on run and next, --executor overrides it, and the graph plugin never calls work add/take/return/accept (red 18 subagent cases, d88, A7)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const path = await writeGraph(join(fixture.root, "graphs"), "passthrough", passthroughGraph);

    const plain = await graphRun(fixture, ["--file", path, "topic=executors"]);
    expect(plain.envelope.executor).toBe("subagent");
    expect((await graphNext(fixture, plain.run)).executor).toBe("subagent");
    const trace = data<{ events: Array<{ payload: { executor?: string }; type: string }> }>(await runCli(fixture, ["trace", plain.run, "--json"]));
    expect(trace.events[0]).toEqual(expect.objectContaining({ type: "graph.run", payload: expect.objectContaining({ executor: "subagent" }) }));

    const team = await graphRun(fixture, ["--file", path, "topic=executors", "--executor", "team"]);
    expect(team.envelope.executor).toBe("team");
    expect((await graphNext(fixture, team.run)).executor).toBe("team");
    const explicit = await graphRun(fixture, ["--file", path, "topic=executors", "--executor", "subagent"]);
    expect(explicit.envelope.executor).toBe("subagent");
    const bad = failure(await runCli(fixture, ["graph", "run", "--file", path, "topic=x", "--executor", "panes", "--json"]));
    expect(bad.code).toBe("INVALID_OPTION");

    const pluginDirectory = join(import.meta.dir, "..", "src", "plugins");
    const sources = (await readdir(pluginDirectory)).filter((entry) => entry.startsWith("graph") && entry.endsWith(".ts"));
    for (const source of sources) {
      const text = await readFile(join(pluginDirectory, source), "utf8");
      expect({ source, calls: text.match(/work add|work take|work return|work accept|takeWork|returnWork|acceptWork|maybeHandleSlpWork/g) ?? [] }).toEqual({ source, calls: [] });
    }
  });
}, 30_000);

import { mkdir, writeFile } from "node:fs/promises";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { installFakeHerdr } from "./helpers-herdr.ts";
import { runCliAt, type Fixture } from "./helpers.ts";

const typed = `---
name: typed-team
description: one schema node under the team executor
limits: {fanout: 2}
nodes:
  start: {kind: function, command: "printf ok"}
  a: {kind: agent, profile: refuter, schema: {type: object, required: [refuted], properties: {refuted: {type: boolean}}}}
  b: {kind: agent, profile: refuter}
  c: {kind: agent, profile: refuter}
  d: {kind: agent, profile: refuter}
edges:
  - {from: start, to: a}
  - {from: start, to: b}
  - {from: b, to: c}
  - {from: b, to: d}
verdict: a
---

## a

Refute A.

## b

B

## c

C

## d

D
`;

async function markedRoom(fixture: Fixture): Promise<string> {
  const room = await scaffoldRoom(fixture.home);
  const marked = await runCliAt(fixture, room, ["room", "mark"], { MAESTRO_ROOM_SCAFFOLD: "1", MAESTRO_SESSION_NONE: "1" });
  expect(marked.exitCode).toBe(0);
  return room;
}

test("graph-executor: from a Lead pane of a RUNNING team graph run reports executor team, graph result --work binds a node, next keeps it open through OPEN/ACTIVE/RETURNED and resolves it with the work return body on DONE, and a bound node counts toward limits.fanout (red 18 team cases, d88, d89)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const fake = await installFakeHerdr(fixture);
    await mkdir(join(fixture.home, "maestro", "graphs"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "graphs", "typed-team.md"), typed);
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Graph under a team", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const team = data<{ team: { roles: Array<{ paneId: string; role: string }> } }>(started);
    const lead = { ...fake.env, HERDR_PANE_ID: team.team.roles.find((role) => role.role === "lead")!.paneId };

    const run = data<{ executor: string; nodes: Array<{ ref: string; work?: string }>; run: string }>(
      await runCliAt(fixture, fixture.repo, ["graph", "run", "typed-team", "--json"], lead),
    );
    expect(run.executor).toBe("team");
    expect(run.nodes.map((node) => node.ref)).toEqual(["a", "b"]);
    const overridden = data<{ executor: string }>(
      await runCliAt(fixture, fixture.repo, ["graph", "run", "typed-team", "--executor", "subagent", "--json"], lead),
    );
    expect(overridden.executor).toBe("subagent");

    const added = data<{ role: { paneId: string }; work: { id: string } }>(
      await runCliAt(fixture, fixture.repo, ["work", "add", run.nodes[0]!.ref === "a" ? "Refute A." : "x", "--to", "peer-refuter", "--json"], lead),
    );
    const peer = { ...fake.env, HERDR_PANE_ID: added.role.paneId };
    const bound = await runCliAt(fixture, fixture.repo, ["graph", "result", run.run, "a", "--work", added.work.id, "--json"], lead);
    expect(bound.exitCode).toBe(0);
    expect(data<{ state: string; work: string }>(bound)).toEqual(expect.objectContaining({ state: "bound", work: added.work.id }));

    const open = data<{ nodes: Array<{ ref: string; work?: string; workState?: string }> }>(
      await runCliAt(fixture, fixture.repo, ["graph", "next", run.run, "--json"], lead),
    );
    expect(open.nodes.find((node) => node.ref === "a")).toEqual(expect.objectContaining({ work: added.work.id, workState: "OPEN" }));
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", added.work.id, "--json"], peer)).exitCode).toBe(0);
    expect(data<{ nodes: Array<{ ref: string; workState?: string }> }>(
      await runCliAt(fixture, fixture.repo, ["graph", "next", run.run, "--json"], lead),
    ).nodes.find((node) => node.ref === "a")?.workState).toBe("ACTIVE");
    expect((await runCliAt(fixture, fixture.repo, ["work", "return", added.work.id, 'refuted: {"refuted": false, "reason": "holds"}', "--json"], peer)).exitCode).toBe(0);
    expect(data<{ nodes: Array<{ ref: string; workState?: string }> }>(
      await runCliAt(fixture, fixture.repo, ["graph", "next", run.run, "--json"], lead),
    ).nodes.find((node) => node.ref === "a")?.workState).toBe("RETURNED");
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", added.work.id, "--json"], lead)).exitCode).toBe(0);
    const resolved = data<{ nodes: Array<{ ref: string }>; state: { a?: unknown } }>(
      await runCliAt(fixture, fixture.repo, ["graph", "next", run.run, "--json"], lead),
    );
    expect(resolved.nodes.map((node) => node.ref)).toEqual(["b"]);
    expect(resolved.state.a).toEqual({ refuted: false, reason: "holds" });
    const trace = data<{ events: Array<{ payload: Record<string, unknown>; type: string }> }>(
      await runCliAt(fixture, fixture.repo, ["trace", run.run, "--json"], lead),
    );
    expect(trace.events.find((event) => event.type === "graph.node.bound")?.payload).toEqual(expect.objectContaining({ ref: "a", work: added.work.id }));

    // A bound node in flight counts toward fanout: with b bound, c fills the second slot and d stops the run.
    const second = data<{ nodes: Array<{ ref: string }>; run: string }>(
      await runCliAt(fixture, fixture.repo, ["graph", "run", "typed-team", "--json"], lead),
    );
    const boundB = data<{ role: { paneId: string }; work: { id: string } }>(
      await runCliAt(fixture, fixture.repo, ["work", "add", "B", "--to", "peer-refuter", "--json"], lead),
    );
    expect((await runCliAt(fixture, fixture.repo, ["graph", "result", second.run, "a", "--text", '{"refuted": true}', "--json"], lead)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["graph", "result", second.run, "b", "--work", boundB.work.id, "--json"], lead)).exitCode).toBe(0);
    const stillB = data<{ nodes: Array<{ ref: string }> }>(await runCliAt(fixture, fixture.repo, ["graph", "next", second.run, "--json"], lead));
    expect(stillB.nodes.map((node) => node.ref)).toEqual(["b"]);
    const peerB = { ...fake.env, HERDR_PANE_ID: boundB.role.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", boundB.work.id, "--json"], peerB)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "return", boundB.work.id, "b done", "--json"], peerB)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", boundB.work.id, "--json"], lead)).exitCode).toBe(0);
    const afterB = data<{ nodes: Array<{ ref: string }> }>(await runCliAt(fixture, fixture.repo, ["graph", "next", second.run, "--json"], lead));
    expect(afterB.nodes.map((node) => node.ref)).toEqual(["c", "d"]);
    const boundC = data<{ work: { id: string } }>(await runCliAt(fixture, fixture.repo, ["work", "add", "C", "--to", "peer-refuter", "--json"], lead));
    expect((await runCliAt(fixture, fixture.repo, ["graph", "result", second.run, "c", "--work", boundC.work.id, "--json"], lead)).exitCode).toBe(0);
    const noText = failure(await runCliAt(fixture, fixture.repo, ["graph", "result", second.run, "d", "--work", boundC.work.id, "--text", "x", "--json"], lead));
    expect(noText.code).toBe("INVALID_OPTION");
    const missing = failure(await runCliAt(fixture, fixture.repo, ["graph", "result", second.run, "d", "--work", "w999", "--json"], lead));
    expect(missing.code).toBe("NOT_FOUND");
    const inflight = data<{ nodes: Array<{ ref: string; work?: string }> }>(await runCliAt(fixture, fixture.repo, ["graph", "next", second.run, "--json"], lead));
    expect(inflight.nodes.map((node) => [node.ref, node.work ?? null])).toEqual([["c", boundC.work.id], ["d", null]]);

    const plain = failure(await runCli(fixture, ["graph", "result", second.run, "d", "--work", boundC.work.id, "--json"]));
    expect(["ROLE_UNPROVEN", "NO_ACTIVE_TEAM"]).toContain(plain.code);

    // Live row 17 defect 5 (d838, then g18): a bound item whose body fails
    // the node's schema sends the node back to issued and unbound; two
    // retries, the third failure fails the node.
    const third = data<{ nodes: Array<{ brief: string; ref: string; schema?: unknown }>; run: string }>(
      await runCliAt(fixture, fixture.repo, ["graph", "run", "typed-team", "--json"], lead),
    );
    const typedA = third.nodes.find((node) => node.ref === "a")!;
    expect(typedA.brief).toContain("Return one JSON object with exactly these keys: refuted; no prose before or after.");
    expect(typedA.brief).toContain("Answer with one JSON object matching this schema:");
    expect(typedA.brief).toContain('"refuted"');
    const badItem = data<{ role: { paneId: string }; work: { id: string } }>(
      await runCliAt(fixture, fixture.repo, ["work", "add", typedA.brief, "--to", "peer-refuter", "--json"], lead),
    );
    const badPeer = { ...fake.env, HERDR_PANE_ID: badItem.role.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["graph", "result", third.run, "a", "--work", badItem.work.id, "--json"], lead)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", badItem.work.id, "--json"], badPeer)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "return", badItem.work.id, '{"category": "habit", "short_summary": "no refuted field"}', "--json"], badPeer)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", badItem.work.id, "--json"], lead)).exitCode).toBe(0);
    const retrying = data<{ done: boolean; nodes: Array<{ ref: string; retry?: { error: string; schema: unknown; work: string }; work?: string }> }>(
      await runCliAt(fixture, fixture.repo, ["graph", "next", third.run, "--json"], lead),
    );
    expect(retrying.done).toBe(false);
    const again = retrying.nodes.find((node) => node.ref === "a");
    expect(again?.work).toBeUndefined();
    expect(again?.retry).toEqual({ error: expect.stringContaining("missing required refuted"), schema: typedA.schema, work: badItem.work.id });
    const retryTrace = data<{ events: Array<{ payload: Record<string, unknown>; type: string }> }>(
      await runCliAt(fixture, fixture.repo, ["trace", third.run, "--json"], lead),
    );
    expect(retryTrace.events.find((event) => event.type === "graph.node.retry")?.payload).toEqual(
      expect.objectContaining({ ref: "a", work: badItem.work.id, attempt: 1 }),
    );

    const secondItem = data<{ role: { paneId: string }; work: { id: string } }>(
      await runCliAt(fixture, fixture.repo, ["work", "add", typedA.brief, "--to", "peer-refuter", "--json"], lead),
    );
    expect((await runCliAt(fixture, fixture.repo, ["graph", "result", third.run, "a", "--work", secondItem.work.id, "--json"], lead)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", secondItem.work.id, "--json"], badPeer)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "return", secondItem.work.id, "still no json", "--json"], badPeer)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", secondItem.work.id, "--json"], lead)).exitCode).toBe(0);
    const retryingAgain = data<{ done: boolean; nodes: Array<{ ref: string; retry?: { error: string; schema: unknown; work: string }; work?: string }> }>(
      await runCliAt(fixture, fixture.repo, ["graph", "next", third.run, "--json"], lead),
    );
    expect(retryingAgain.done).toBe(false);
    const secondRetry = retryingAgain.nodes.find((node) => node.ref === "a");
    expect(secondRetry?.work).toBeUndefined();
    expect(secondRetry?.retry).toEqual({ error: expect.stringContaining("no JSON found"), schema: typedA.schema, work: secondItem.work.id });
    const retryTrace2 = data<{ events: Array<{ payload: Record<string, unknown>; type: string }> }>(
      await runCliAt(fixture, fixture.repo, ["trace", third.run, "--json"], lead),
    );
    expect(retryTrace2.events.filter((event) => event.type === "graph.node.retry").map((event) => event.payload.attempt)).toEqual([1, 2]);

    const thirdItem = data<{ role: { paneId: string }; work: { id: string } }>(
      await runCliAt(fixture, fixture.repo, ["work", "add", typedA.brief, "--to", "peer-refuter", "--json"], lead),
    );
    expect((await runCliAt(fixture, fixture.repo, ["graph", "result", third.run, "a", "--work", thirdItem.work.id, "--json"], lead)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", thirdItem.work.id, "--json"], badPeer)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "return", thirdItem.work.id, "third miss", "--json"], badPeer)).exitCode).toBe(0);
    expect((await runCliAt(fixture, fixture.repo, ["work", "accept", thirdItem.work.id, "--json"], lead)).exitCode).toBe(0);
    const failed = data<{ done: boolean; failed?: { node: string; error?: string } }>(
      await runCliAt(fixture, fixture.repo, ["graph", "next", third.run, "--json"], lead),
    );
    expect(failed.done).toBe(true);
    expect(failed.failed?.node).toBe("a");
  });
}, 60_000);
