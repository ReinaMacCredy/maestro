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
