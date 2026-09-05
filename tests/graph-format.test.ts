import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, failure, homeGraphs, writeGraph, writeProfile } from "./graph-helpers.ts";

const good = `---
name: demo
description: a graph with every node kind
input:
  range: {required: true}
  tier: {default: light}
limits: {nodes: 10}
nodes:
  classify: {kind: agent, profile: tester, schema: {type: object, required: [ok], properties: {ok: {type: boolean}}}}
  route: {kind: router}
  stat: {kind: function, command: "git diff --stat {range}"}
  collect: {kind: join, key: [file, line], window: 3}
  each: {kind: foreach, over: collect.items}
  check: {kind: agent, profile: tester}
  ask: {kind: human}
edges:
  - {from: classify, to: route}
  - {from: route, to: stat, when: classify.ok}
  - {from: route, to: ask, when: {path: tier, eq: full}}
  - {from: stat, to: collect}
  - {from: collect, to: each}
  - {from: each, to: check}
---

## classify

Classify {range} at tier {tier}.

## check

Check {item}.

## ask

Confirm before continuing.
`;

test("graph-format: a graph markdown parses into frontmatter and sections; each malformed shape fails GRAPH_INVALID naming the node (red 1)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    await writeGraph(homeGraphs(fixture), "demo", good);

    const shown = data<{
      graph: { edges: Array<{ from: string; to: string }>; nodes: Array<{ kind: string; node: string; prompt?: string }> };
      name: string;
      origin: string;
    }>(await runCli(fixture, ["graph", "show", "demo", "--json"]));
    expect(shown.name).toBe("demo");
    expect(shown.origin).toBe("home");
    expect(shown.graph.nodes.map((node) => [node.node, node.kind])).toEqual([
      ["classify", "agent"],
      ["route", "router"],
      ["stat", "function"],
      ["collect", "join"],
      ["each", "foreach"],
      ["check", "agent"],
      ["ask", "human"],
    ]);
    expect(shown.graph.nodes.find((node) => node.node === "classify")?.prompt).toBe("Classify {range} at tier {tier}.");
    expect(shown.graph.nodes.find((node) => node.node === "ask")?.prompt).toBe("Confirm before continuing.");
    expect(shown.graph.edges).toHaveLength(6);
    const text = await runCli(fixture, ["graph", "show", "demo"]);
    expect(text.exitCode).toBe(0);
    expect(text.stdout).toContain("## classify");

    const invalid: Array<[string, string, string[]]> = [
      [
        "no-section",
        good.replace("## check\n\nCheck {item}.\n", ""),
        ["check", "## check"],
      ],
      [
        "unknown-kind",
        good.replace("{kind: human}", "{kind: oracle}"),
        ["ask", "oracle"],
      ],
      [
        "missing-target",
        good.replace("{from: each, to: check}", "{from: each, to: ghost}"),
        ["each", "ghost"],
      ],
      [
        "router-no-when",
        good
          .replace("{from: route, to: stat, when: classify.ok}", "{from: route, to: stat}")
          .replace("{from: route, to: ask, when: {path: tier, eq: full}}", "{from: route, to: ask}"),
        ["route", "when"],
      ],
      [
        "no-profile",
        good.replace("check: {kind: agent, profile: tester}", "check: {kind: agent, profile: phantom}"),
        ["check", "phantom"],
      ],
    ];
    for (const [name, text, expected] of invalid) {
      const path = await writeGraph(join(fixture.root, "bad"), name, text);
      const refused = await runCli(fixture, ["graph", "run", "--file", path, "range=HEAD", "--json"]);
      const error = failure(refused);
      expect({ name, code: error.code }).toEqual({ name, code: "GRAPH_INVALID" });
      for (const fragment of expected) {
        expect({ name, message: error.message }).toEqual({ name, message: expect.stringContaining(fragment) });
      }
    }
    // Nothing invalid ever became a run.
    const works = data<{ works: Array<{ kind: string }> }>(await runCli(fixture, ["work", "list", "--json"]));
    expect(works.works.filter((work) => work.kind === "graph")).toEqual([]);
  });
}, 30_000);
