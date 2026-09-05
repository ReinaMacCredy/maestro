import { expect, test } from "bun:test";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, failure, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

const graph = `---
name: routed
description: a router over facts a function node put on state
nodes:
  facts: {kind: function, command: "printf '{\\"trust\\": true, \\"count\\": 2, \\"tier\\": \\"full\\", \\"tags\\": []}'"}
  route: {kind: router}
  security: {kind: agent, profile: tester}
  deep: {kind: agent, profile: tester}
  wide: {kind: agent, profile: tester}
  both: {kind: agent, profile: tester}
  either: {kind: agent, profile: tester}
  fallback: {kind: agent, profile: tester}
  after: {kind: agent, profile: tester}
edges:
  - {from: facts, to: route}
  - {from: route, to: security, when: facts.trust}
  - {from: route, to: deep, when: {path: facts.tier, eq: full}}
  - {from: route, to: wide, when: {path: facts.count, gt: 1}}
  - {from: route, to: both, when: {all: [facts.trust, {path: facts.count, gte: 2}]}}
  - {from: route, to: either, when: {any: [facts.tags, {path: facts.tier, eq: light}, {not: facts.trust}]}}
  - {from: route, to: fallback}
  - {from: either, to: after}
---

## security

S

## deep

D

## wide

W

## both

B

## either

E

## fallback

F

## after

A
`;

test("graph-router: when conditions (path truthiness, equality, numeric comparison, all, any, not) select edges, unselected targets are skipped, a JS-looking condition is refused as data, and no eval or new Function exists under the graph plugin (red 8, A2)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const path = await writeGraph(join(fixture.root, "graphs"), "routed", graph);
    const { run, envelope } = await graphRun(fixture, ["--file", path]);
    expect(envelope.nodes.map((node) => node.ref)).toEqual(["security", "deep", "wide", "both"]);
    expect(envelope.state.route).toEqual({ selected: ["security", "deep", "wide", "both"] });
    const trace = data<{ events: Array<{ payload: { node?: string }; type: string }> }>(await runCli(fixture, ["trace", run, "--json"]));
    expect(trace.events.filter((event) => event.type === "graph.node.skipped").map((event) => event.payload.node)).toEqual([
      "either",
      "fallback",
      "after",
    ]);

    for (const [name, condition] of [
      ["comparison", "facts.count > 1"],
      ["call", "eval(facts.count)"],
      ["template", "${facts.count}"],
    ] as const) {
      const bad = await writeGraph(join(fixture.root, "bad"), name, graph.replace("when: facts.trust}", `when: "${condition}"}`));
      const refused = failure(await runCli(fixture, ["graph", "run", "--file", bad, "--json"]));
      expect({ name, code: refused.code }).toEqual({ name, code: "GRAPH_INVALID" });
      expect(refused.message).toContain("data");
      expect(refused.message).toContain("never code");
    }

    const pluginDirectory = join(import.meta.dir, "..", "src", "plugins");
    const sources = (await readdir(pluginDirectory)).filter((entry) => entry.startsWith("graph") && entry.endsWith(".ts"));
    expect(sources.length).toBeGreaterThanOrEqual(2);
    for (const source of sources) {
      const text = await readFile(join(pluginDirectory, source), "utf8");
      expect({ source, evals: text.match(/\beval\(|new Function\b/g) ?? [] }).toEqual({ source, evals: [] });
    }
  });
}, 30_000);
