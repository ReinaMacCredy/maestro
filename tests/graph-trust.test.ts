import { expect, test } from "bun:test";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, failure, graphNext, graphRun, homeGraphs, repoGraphs, writeGraph, writeProfile } from "./graph-helpers.ts";

const withFunction = (name: string, marker: string) => `---
name: ${name}
description: a function node then an agent
nodes:
  probe: {kind: function, command: "printf ${marker}"}
  read: {kind: agent, profile: tester}
edges:
  - {from: probe, to: read}
---

## read

Probe said {probe} from ${marker}.
`;

test("graph-trust: a repo graph's function node is refused without a plugin-trust grant and runs with one, home and out-of-repo file graphs run without a grant, and a repo graph shadows a home graph of the same name (red 11, A3, d80, d835)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    await writeGraph(repoGraphs(fixture), "gated", withFunction("gated", "repo-fn"));

    const refused = failure(await runCli(fixture, ["graph", "run", "gated", "--json"]));
    expect(refused.code).toBe("GRAPH_UNTRUSTED");
    expect(refused.node).toBe("probe");
    expect(refused.message).toContain("maestro graph trust gated");
    const run = refused.run as string;
    expect(run).toMatch(/^w\d+$/);
    expect(failure(await runCli(fixture, ["graph", "next", run, "--json"])).code).toBe("GRAPH_UNTRUSTED");
    const trace = data<{ events: Array<{ type: string }> }>(await runCli(fixture, ["trace", run, "--json"]));
    expect(trace.events.map((event) => event.type)).toEqual(["graph.run"]);

    const trusted = await runCli(fixture, ["graph", "trust", "gated"]);
    expect(trusted.exitCode).toBe(0);
    expect(trusted.stdout).toContain("sha256:");
    const resumed = await graphNext(fixture, run);
    expect(resumed.nodes.map((node) => node.ref)).toEqual(["read"]);
    expect(resumed.nodes[0]?.prompt).toBe("Probe said repo-fn from repo-fn.");

    // Editing the trusted file drops the grant.
    await writeGraph(repoGraphs(fixture), "gated", withFunction("gated", "repo-fn-edited"));
    expect(failure(await runCli(fixture, ["graph", "run", "gated", "--json"])).code).toBe("GRAPH_UNTRUSTED");

    // An uninstalled file under the checkout is gated the same way, by path.
    const loose = await writeGraph(join(fixture.repo, "scratch"), "loose", withFunction("loose", "loose-fn"));
    const looseRefused = failure(await runCli(fixture, ["graph", "run", "--file", loose, "--json"]));
    expect(looseRefused.code).toBe("GRAPH_UNTRUSTED");
    expect(looseRefused.message).toContain(`maestro graph trust --file ${loose}`);
    expect((await runCli(fixture, ["graph", "trust", "--file", loose])).exitCode).toBe(0);
    expect((await graphNext(fixture, looseRefused.run as string)).nodes.map((node) => node.ref)).toEqual(["read"]);

    // Home graphs and files outside the checkout run fully.
    await writeGraph(homeGraphs(fixture), "homely", withFunction("homely", "home-fn"));
    const home = await graphRun(fixture, ["homely"]);
    expect(home.envelope.nodes[0]?.prompt).toBe("Probe said home-fn from home-fn.");
    const outside = await writeGraph(join(fixture.root, "elsewhere"), "outside", withFunction("outside", "outside-fn"));
    const file = await graphRun(fixture, ["--file", outside]);
    expect(file.envelope.nodes[0]?.prompt).toBe("Probe said outside-fn from outside-fn.");
    const notRepo = failure(await runCli(fixture, ["graph", "trust", "homely", "--json"]));
    expect(notRepo.code).toBe("NOT_REPO_GRAPH");

    // Shadowing by name: repo wins over home.
    await writeGraph(homeGraphs(fixture), "shadow", withFunction("shadow", "home-copy"));
    await writeGraph(repoGraphs(fixture), "shadow", withFunction("shadow", "repo-copy"));
    await writeFile(join(fixture.repo, "note"), "x");
    expect(data<{ origin: string }>(await runCli(fixture, ["graph", "show", "shadow", "--json"])).origin).toBe("repo");
    expect(failure(await runCli(fixture, ["graph", "run", "shadow", "--json"])).message).toContain("maestro graph trust shadow");
  });
}, 30_000);
