import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

const graph = `---
name: lenses
description: three lenses joined twice with different windows
nodes:
  start: {kind: function, command: "printf ok"}
  lens-a: {kind: agent, profile: tester}
  lens-b: {kind: agent, profile: tester}
  lens-c: {kind: agent, profile: tester}
  tight: {kind: join, collect: findings, key: [file, line], window: 0}
  loose: {kind: join, collect: findings, key: [file, line], window: 3}
edges:
  - {from: start, to: lens-a}
  - {from: start, to: lens-b}
  - {from: start, to: lens-c}
  - {from: lens-a, to: tight}
  - {from: lens-b, to: tight}
  - {from: lens-c, to: tight}
  - {from: lens-a, to: loose}
  - {from: lens-b, to: loose}
  - {from: lens-c, to: loose}
---

## lens-a

Lens A.

## lens-b

Lens B.

## lens-c

Lens C.
`;

test("graph-join: join waits for every producer, dedups by [file, line] with window 0 and 3 keeping the first item with the other lenses as provenance, and never appears in next (red 6, d82)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const path = await writeGraph(join(fixture.root, "graphs"), "lenses", graph);
    const { run, envelope } = await graphRun(fixture, ["--file", path]);
    expect(envelope.nodes.map((node) => node.ref)).toEqual(["lens-a", "lens-b", "lens-c"]);

    const a = { findings: [{ file: "a.ts", line: 10, summary: "A1" }, { file: "b.ts", line: 5, summary: "A2" }] };
    const b = { findings: [{ file: "a.ts", line: 10, summary: "B1" }, { file: "a.ts", line: 12, summary: "B2" }] };
    const c = { findings: [{ file: "b.ts", line: 5, summary: "C1" }] };
    expect((await graphResult(fixture, run, "lens-a", JSON.stringify(a))).exitCode).toBe(0);
    expect((await graphResult(fixture, run, "lens-b", JSON.stringify(b))).exitCode).toBe(0);
    const waiting = await graphNext(fixture, run);
    expect(waiting.done).toBe(false);
    expect(waiting.nodes.map((node) => node.ref)).toEqual(["lens-c"]);
    expect(waiting.state.tight).toBeUndefined();

    expect((await graphResult(fixture, run, "lens-c", JSON.stringify(c))).exitCode).toBe(0);
    const done = await graphNext(fixture, run);
    expect(done.done).toBe(true);
    expect(done.verdict).toEqual({
      tight: {
        items: [
          { file: "a.ts", line: 10, summary: "A1", producer: "lens-a", provenance: ["lens-b"] },
          { file: "b.ts", line: 5, summary: "A2", producer: "lens-a", provenance: ["lens-c"] },
          { file: "a.ts", line: 12, summary: "B2", producer: "lens-b" },
        ],
        total: 5,
      },
      loose: {
        items: [
          { file: "a.ts", line: 10, summary: "A1", producer: "lens-a", provenance: ["lens-b", "lens-b"] },
          { file: "b.ts", line: 5, summary: "A2", producer: "lens-a", provenance: ["lens-c"] },
        ],
        total: 5,
      },
    });
    const trace = data<{ events: Array<{ payload: { kind?: string }; type: string }> }>(await runCli(fixture, ["trace", run, "--json"]));
    expect(trace.events.filter((event) => event.type === "graph.node.issued" && event.payload.kind === "join")).toEqual([]);
  });
}, 30_000);
