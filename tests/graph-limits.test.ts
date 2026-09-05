import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

const fanout = `---
name: fanout
description: three agents at once under fanout 2
limits: {fanout: 2}
nodes:
  start: {kind: function, command: "printf ok"}
  a: {kind: agent, profile: tester}
  b: {kind: agent, profile: tester}
  c: {kind: agent, profile: tester}
edges:
  - {from: start, to: a}
  - {from: start, to: b}
  - {from: start, to: c}
---

## a

A

## b

B

## c

C
`;

const loops = `---
name: loops
description: a loop of three rounds under loops 1
limits: {loops: 1}
nodes:
  draft: {kind: agent, profile: tester}
  review: {kind: agent, profile: tester}
edges:
  - {from: draft, to: review}
  - {from: review, to: draft, max_rounds: 3}
---

## draft

Draft {round}.

## review

Review {round}.
`;

const nodes = `---
name: wide
description: five instances under nodes 4
limits: {nodes: 4}
nodes:
  items: {kind: function, command: "printf '[1, 2, 3, 4, 5]'"}
  each: {kind: foreach, over: items}
  look: {kind: agent, profile: tester}
edges:
  - {from: items, to: each}
  - {from: each, to: look}
---

## look

Look at {item}.
`;

test("graph-limits: a fan-out over limits.fanout, a loop over limits.loops and a run over limits.nodes each end with stopped LIMIT and a partial result; --limit nodes=N overrides (red 9, d84)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const graphs = join(fixture.root, "graphs");

    const wide = await graphRun(fixture, ["--file", await writeGraph(graphs, "fanout", fanout)]);
    expect(wide.envelope).toEqual(expect.objectContaining({ done: true, stopped: "LIMIT", limit: "fanout", used: 3, nodes: [] }));
    expect((wide.envelope.partial as { start?: string }).start).toBe("ok");
    expect(data<{ work: { evidence: string; state: string } }>(await runCli(fixture, ["work", "show", wide.run, "--json"])).work).toEqual(
      expect.objectContaining({ state: "done", evidence: expect.stringContaining("LIMIT:fanout") }),
    );
    expect((await graphNext(fixture, wide.run)).stopped).toBe("LIMIT");

    const loop = await graphRun(fixture, ["--file", await writeGraph(graphs, "loops", loops)]);
    const rounds: string[] = [];
    let current = loop.envelope;
    for (let step = 0; step < 8 && !current.done; step += 1) {
      const node = current.nodes[0]!;
      rounds.push(`${node.ref}:${node.round}`);
      expect((await graphResult(fixture, loop.run, node.ref, `${node.ref} r${node.round}`)).exitCode).toBe(0);
      current = await graphNext(fixture, loop.run);
    }
    expect(rounds).toEqual(["draft:1", "review:1", "draft:2", "review:2"]);
    expect(current).toEqual(expect.objectContaining({ done: true, stopped: "LIMIT", limit: "loops", used: 2 }));
    expect((current.partial as { review?: string }).review).toBe("review r2");

    const path = await writeGraph(graphs, "wide", nodes);
    const capped = await graphRun(fixture, ["--file", path]);
    expect(capped.envelope).toEqual(expect.objectContaining({ done: true, stopped: "LIMIT", limit: "nodes", used: 7 }));
    expect((capped.envelope.partial as { items?: number[] }).items).toEqual([1, 2, 3, 4, 5]);
    const trace = data<{ events: Array<{ payload: Record<string, unknown>; type: string }> }>(await runCli(fixture, ["trace", capped.run, "--json"]));
    expect(trace.events.at(-1)).toEqual(expect.objectContaining({ type: "graph.stopped", payload: { limit: "nodes", used: 7 } }));

    const raised = await graphRun(fixture, ["--file", path, "--limit", "nodes=10"]);
    expect(raised.envelope.done).toBe(false);
    expect(raised.envelope.nodes).toHaveLength(5);
    const bad = await runCli(fixture, ["graph", "run", "--file", path, "--limit", "tokens=5", "--json"]);
    expect(bad.exitCode).toBe(1);
    expect(bad.stderr).toContain("INVALID_OPTION");
  });
}, 30_000);
