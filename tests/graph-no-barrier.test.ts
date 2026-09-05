import { expect, test } from "bun:test";
import { join } from "node:path";
import { withFixture } from "./helpers.ts";
import { graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

const graph = `---
name: pipeline
description: two agent stages per item, joined at the end
nodes:
  items: {kind: function, command: "printf '[\\"a\\", \\"b\\", \\"c\\"]'"}
  each: {kind: foreach, over: items}
  stage1: {kind: agent, profile: tester}
  stage2: {kind: agent, profile: tester}
  collect: {kind: join}
edges:
  - {from: items, to: each}
  - {from: each, to: stage1}
  - {from: stage1, to: stage2}
  - {from: stage2, to: collect}
---

## stage1

Stage one for {item}.

## stage2

Stage two for {item} after {stage1}.
`;

test("graph-no-barrier: after item A's stage 1 returns, next issues A's stage 2 while B and C are still open; only the join waits (red 4, A5)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const path = await writeGraph(join(fixture.root, "graphs"), "pipeline", graph);
    const { run, envelope } = await graphRun(fixture, ["--file", path]);
    expect(envelope.nodes.map((node) => node.ref).sort()).toEqual(["stage1@0", "stage1@1", "stage1@2"]);
    expect(envelope.nodes.find((node) => node.ref === "stage1@1")?.prompt).toBe("Stage one for b.");
    expect(envelope.nodes.find((node) => node.ref === "stage1@1")?.instance).toBe("1");

    expect((await graphResult(fixture, run, "stage1@0", "one-a")).exitCode).toBe(0);
    const next = await graphNext(fixture, run);
    expect(next.done).toBe(false);
    expect(next.nodes.map((node) => node.ref).sort()).toEqual(["stage1@1", "stage1@2", "stage2@0"]);
    expect(next.nodes.find((node) => node.ref === "stage2@0")?.prompt).toBe("Stage two for a after one-a.");

    expect((await graphResult(fixture, run, "stage2@0", "two-a")).exitCode).toBe(0);
    expect((await graphResult(fixture, run, "stage1@1", "one-b")).exitCode).toBe(0);
    expect((await graphResult(fixture, run, "stage1@2", "one-c")).exitCode).toBe(0);
    const late = await graphNext(fixture, run);
    expect(late.nodes.map((node) => node.ref).sort()).toEqual(["stage2@1", "stage2@2"]);
    expect((await graphResult(fixture, run, "stage2@1", "two-b")).exitCode).toBe(0);
    expect((await graphResult(fixture, run, "stage2@2", "two-c")).exitCode).toBe(0);
    const done = await graphNext(fixture, run);
    expect(done.done).toBe(true);
    expect(done.verdict).toEqual({
      items: [
        { instance: "0", producer: "stage2", value: "two-a" },
        { instance: "1", producer: "stage2", value: "two-b" },
        { instance: "2", producer: "stage2", value: "two-c" },
      ],
      total: 3,
    });
  });
}, 30_000);
