import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

// Live row 12 (run w653, --limit nodes=3): the run stopped at used 13 after
// two agent nodes were already issued, because table rows were counted
// after the join. d836: the unit is the issued agent node, checked before
// issuing.
const fanout = `---
name: budget
description: five agent nodes behind a function, a router and a skipped branch
nodes:
  start: {kind: function, command: "printf '{\\"go\\": true}'"}
  route: {kind: router}
  a: {kind: agent, profile: tester}
  b: {kind: agent, profile: tester}
  c: {kind: agent, profile: tester}
  d: {kind: agent, profile: tester}
  e: {kind: agent, profile: tester}
  never: {kind: agent, profile: tester}
  ask: {kind: human}
edges:
  - {from: start, to: route}
  - {from: route, to: ask, when: start.go}
  - {from: route, to: never, when: {path: start.go, eq: false}}
  - {from: ask, to: a}
  - {from: ask, to: b}
  - {from: ask, to: c}
  - {from: ask, to: d}
  - {from: ask, to: e}
---

## a

A

## b

B

## c

C

## d

D

## e

E

## never

never

## ask

Go on?
`;

const loop = `---
name: rounds
description: one agent re-issued by a loop
nodes:
  draft: {kind: agent, profile: tester}
  review: {kind: agent, profile: tester}
edges:
  - {from: draft, to: review}
  - {from: review, to: draft, max_rounds: 5}
---

## draft

Draft {round}.

## review

Review {round}.
`;

test("graph-node-budget: a nodes=N run never issues the N+1th agent node; deterministic, skipped and human rows do not count; a re-issued round counts again (live row 12 finding, d836)", async () => {
  await writeProfileAndRun(async (fixture, graphs) => {
    const path = await writeGraph(graphs, "budget", fanout);
    const capped = await graphRun(fixture, ["--file", path, "--limit", "nodes=3"]);
    expect(capped.envelope.nodes.map((node) => node.ref)).toEqual(["ask"]);
    expect((await graphResult(fixture, capped.run, "ask", "yes")).exitCode).toBe(0);
    const stopped = await graphNext(fixture, capped.run);
    expect(stopped).toEqual(expect.objectContaining({ done: true, stopped: "LIMIT", limit: "nodes", used: 4, nodes: [] }));
    const issued = data<{ events: Array<{ payload: { kind?: string; node?: string }; type: string }> }>(
      await runCli(fixture, ["trace", capped.run, "--json"]),
    ).events.filter((event) => event.type === "graph.node.issued");
    expect(issued.map((event) => event.payload.node)).toEqual(["ask", "a", "b", "c"]);
    expect(issued.filter((event) => event.payload.kind === "agent")).toHaveLength(3);

    const exact = await graphRun(fixture, ["--file", path, "--limit", "nodes=5"]);
    expect((await graphResult(fixture, exact.run, "ask", "yes")).exitCode).toBe(0);
    const all = await graphNext(fixture, exact.run);
    expect(all.done).toBe(false);
    expect(all.nodes.map((node) => node.ref)).toEqual(["a", "b", "c", "d", "e"]);

    const rounds = await graphRun(fixture, ["--file", await writeGraph(graphs, "rounds", loop), "--limit", "nodes=3"]);
    const seen: string[] = [];
    let current = rounds.envelope;
    for (let step = 0; step < 8 && !current.done; step += 1) {
      const node = current.nodes[0]!;
      seen.push(`${node.ref}:${node.round}`);
      expect((await graphResult(fixture, rounds.run, node.ref, "x")).exitCode).toBe(0);
      current = await graphNext(fixture, rounds.run);
    }
    expect(seen).toEqual(["draft:1", "review:1", "draft:2"]);
    expect(current).toEqual(expect.objectContaining({ done: true, stopped: "LIMIT", limit: "nodes", used: 4 }));
  });
}, 30_000);

async function writeProfileAndRun(run: (fixture: Parameters<Parameters<typeof withFixture>[0]>[0], graphs: string) => Promise<void>): Promise<void> {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    await run(fixture, join(fixture.root, "graphs"));
  });
}
