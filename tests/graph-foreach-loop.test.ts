import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

const keyed = `---
name: keyed
description: foreach with a declared key
nodes:
  files: {kind: function, command: "printf '[{\\"name\\": \\"a.ts\\", \\"lines\\": 10}, {\\"name\\": \\"b.ts\\", \\"lines\\": 20}]'"}
  each: {kind: foreach, over: files, key: name}
  inspect: {kind: agent, profile: tester}
  gather: {kind: join}
edges:
  - {from: files, to: each}
  - {from: each, to: inspect}
  - {from: inspect, to: gather}
---

## inspect

Inspect {item.name} with {item.lines} lines (instance {instance}, index {index}).
`;

const looping = `---
name: looping
description: draft, review, loop back twice, then finish
nodes:
  draft: {kind: agent, profile: tester}
  review: {kind: agent, profile: tester}
  final: {kind: agent, profile: tester}
edges:
  - {from: draft, to: review}
  - {from: review, to: draft, max_rounds: 3}
  - {from: review, to: final}
verdict: final
---

## draft

Draft round {round}.

## review

Review round {round} of {draft}.

## final

Finish after {review}.
`;

test("graph-foreach-loop: foreach yields one instance per item with an instance key; a loop-back edge re-issues the target until max_rounds, then the run proceeds; rounds are journaled (red 7, item 1)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const keyedPath = await writeGraph(join(fixture.root, "graphs"), "keyed", keyed);
    const { run, envelope } = await graphRun(fixture, ["--file", keyedPath]);
    expect(envelope.nodes.map((node) => node.ref)).toEqual(["inspect@a.ts", "inspect@b.ts"]);
    expect(envelope.nodes[1]?.prompt).toBe("Inspect b.ts with 20 lines (instance b.ts, index 1).");
    expect(envelope.nodes[1]?.inputs).toEqual({ index: 1, item: { name: "b.ts", lines: 20 } });
    for (const ref of ["inspect@a.ts", "inspect@b.ts"]) {
      expect((await graphResult(fixture, run, ref, `${ref} ok`)).exitCode).toBe(0);
    }
    const done = await graphNext(fixture, run);
    expect(done.done).toBe(true);
    expect(done.verdict).toEqual({
      items: [
        { instance: "a.ts", producer: "inspect", value: "inspect@a.ts ok" },
        { instance: "b.ts", producer: "inspect", value: "inspect@b.ts ok" },
      ],
      total: 2,
    });
    const instances = data<{ events: Array<{ payload: Record<string, unknown>; type: string }> }>(
      await runCli(fixture, ["trace", run, "--json"]),
    ).events.find((event) => event.type === "graph.node.instances");
    expect(instances?.payload).toEqual({ node: "each", count: 2, keys: ["a.ts", "b.ts"], nodes: ["inspect"] });

    const loopPath = await writeGraph(join(fixture.root, "graphs"), "looping", looping);
    const loop = await graphRun(fixture, ["--file", loopPath]);
    const rounds: string[] = [];
    let current = loop.envelope;
    for (let step = 0; step < 12 && !current.done; step += 1) {
      expect(current.nodes).toHaveLength(1);
      const node = current.nodes[0]!;
      rounds.push(`${node.ref}:${node.round}`);
      expect((await graphResult(fixture, loop.run, node.ref, `${node.ref} r${node.round}`)).exitCode).toBe(0);
      current = await graphNext(fixture, loop.run);
    }
    expect(rounds).toEqual(["draft:1", "review:1", "draft:2", "review:2", "draft:3", "review:3", "final:1"]);
    expect(current.done).toBe(true);
    expect(current.verdict).toBe("final r1");
    expect(loop.envelope.nodes[0]?.prompt).toBe("Draft round 1.");
    const trace = data<{ events: Array<{ payload: Record<string, unknown>; type: string }> }>(
      await runCli(fixture, ["trace", loop.run, "--json"]),
    );
    expect(trace.events.filter((event) => event.type === "graph.loop").map((event) => event.payload)).toEqual([
      { from: "review", to: "draft", round: 2, max_rounds: 3, nodes: ["draft", "review"] },
      { from: "review", to: "draft", round: 3, max_rounds: 3, nodes: ["draft", "review"] },
    ]);
    expect(
      trace.events.filter((event) => event.type === "graph.node.issued").map((event) => `${event.payload.node}:${event.payload.round}`),
    ).toEqual(["draft:1", "review:1", "draft:2", "review:2", "draft:3", "review:3", "final:1"]);
  });
}, 30_000);
