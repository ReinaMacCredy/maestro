import { expect, test } from "bun:test";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

const graph = `---
name: pull
description: function, agent and human in a line
input:
  subject: {required: true}
nodes:
  count: {kind: function, command: "printf '{\\"count\\": 3, \\"subject\\": %s}' {subject}"}
  summarize: {kind: agent, profile: tester}
  confirm: {kind: human}
edges:
  - {from: count, to: summarize}
  - {from: summarize, to: confirm}
verdict: confirm
---

## summarize

There are {count.count} items about {subject}.

## confirm

Approve the summary: {summarize}
`;

test("graph-next-pull: next returns only ready agent and human nodes, function nodes are already executed with their output on state, no model process ever starts, done returns the verdict (red 3, A1)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const bin = join(fixture.root, "bin");
    const log = join(fixture.root, "models.log");
    await mkdir(bin, { recursive: true });
    for (const model of ["claude", "codex"]) {
      const script = join(bin, model);
      await writeFile(script, `#!/bin/sh\necho "${model} $*" >> ${log}\n`);
      await chmod(script, 0o755);
    }
    const env = { PATH: [bin, dirname(process.execPath), "/usr/bin", "/bin"].join(":") };
    const path = await writeGraph(join(fixture.root, "graphs"), "pull", graph);

    const { run, envelope } = await graphRun(fixture, ["--file", path, "subject=\"leases\""], env);
    expect(envelope.nodes.map((node) => [node.ref, node.kind])).toEqual([["summarize", "agent"]]);
    expect(envelope.nodes[0]?.prompt).toBe("There are 3 items about \"leases\".");
    expect(envelope.state.count).toEqual({ count: 3, subject: "leases" });

    const again = await graphNext(fixture, run, env);
    expect(again.nodes.map((node) => node.ref)).toEqual(["summarize"]);
    expect((await graphResult(fixture, run, "summarize", "three lease exits", [], env)).exitCode).toBe(0);

    const human = await graphNext(fixture, run, env);
    expect(human.done).toBe(false);
    expect(human.nodes.map((node) => [node.ref, node.kind])).toEqual([["confirm", "human"]]);
    expect(human.nodes[0]?.prompt).toBe("Approve the summary: three lease exits");
    expect((await graphResult(fixture, run, "confirm", "approved", [], env)).exitCode).toBe(0);

    const done = await graphNext(fixture, run, env);
    expect(done.done).toBe(true);
    expect(done.verdict).toBe("approved");
    expect(done.nodes).toEqual([]);

    const trace = data<{ events: Array<{ payload: { kind?: string; node?: string }; type: string }> }>(
      await runCli(fixture, ["trace", run, "--json"], env),
    );
    expect(trace.events.map((event) => `${event.type}${event.payload.node ? `:${event.payload.node}` : ""}`)).toEqual([
      "graph.run",
      "graph.node.done:count",
      "graph.node.issued:summarize",
      "graph.node.done:summarize",
      "graph.node.issued:confirm",
      "graph.node.done:confirm",
      "graph.done",
    ]);
    expect(await Bun.file(log).exists()).toBe(false);
    const missing = await runCli(fixture, ["graph", "next", "w999"], env);
    expect(missing.exitCode).toBe(1);
    expect(missing.stderr).toContain("NOT_FOUND");
    void readFile;
  });
}, 30_000);
