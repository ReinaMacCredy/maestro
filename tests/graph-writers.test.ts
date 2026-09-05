import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { initializeGitRepository, runCli, runTool, withFixture } from "./helpers.ts";
import { data, failure, graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

const writerFirst = `---
name: writer-first
description: a writing node declared before its sibling
nodes:
  start: {kind: function, command: "printf ok"}
  fix: {kind: agent, profile: tester, writes: true}
  a: {kind: agent, profile: tester}
  b: {kind: agent, profile: tester}
edges:
  - {from: start, to: fix}
  - {from: start, to: a}
  - {from: fix, to: b}
---

## fix

Fix it.

## a

A

## b

B after {fix}
`;

const siblingFirst = writerFirst
  .replace("name: writer-first", "name: sibling-first")
  .replace("  fix: {kind: agent, profile: tester, writes: true}\n  a: {kind: agent, profile: tester}\n", "  a: {kind: agent, profile: tester}\n  fix: {kind: agent, profile: tester, writes: true}\n");

test("graph-writers: a writes node is never issued while another node is in flight and nothing else issues while it is; next from a session other than the run's holder is refused at the writing node naming the holder; --files is stored and the run commits nothing (red 10, d85, d99)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    await initializeGitRepository(fixture.repo);
    const head = (await runTool(["git", "rev-parse", "HEAD"], fixture.repo)).stdout.trim();
    const graphs = join(fixture.root, "graphs");

    // Writer first in node order: it runs alone, the sibling waits.
    const first = await graphRun(fixture, ["--file", await writeGraph(graphs, "writer-first", writerFirst)]);
    expect(first.envelope.nodes.map((node) => node.ref)).toEqual(["fix"]);
    expect((await graphNext(fixture, first.run)).nodes.map((node) => node.ref)).toEqual(["fix"]);
    expect((await graphResult(fixture, first.run, "fix", "patched", ["--files", "src/a.ts,src/b.ts"])).exitCode).toBe(0);
    const afterFix = await graphNext(fixture, first.run);
    expect(afterFix.nodes.map((node) => node.ref).sort()).toEqual(["a", "b"]);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const stored = database.query<{ files: string }, [string]>("SELECT files FROM graph_nodes WHERE run_id = ? AND node_id = 'fix'").get(first.run);
    expect(JSON.parse(stored?.files ?? "null")).toEqual(["src/a.ts", "src/b.ts"]);
    database.close();

    // Sibling first in node order: the writer waits for the sibling's return.
    const second = await graphRun(fixture, ["--file", await writeGraph(graphs, "sibling-first", siblingFirst)]);
    expect(second.envelope.nodes.map((node) => node.ref)).toEqual(["a"]);
    expect((await graphResult(fixture, second.run, "a", "done")).exitCode).toBe(0);
    expect((await graphNext(fixture, second.run)).nodes.map((node) => node.ref)).toEqual(["fix"]);

    // The lease: the run is held by its driver; another live session is refused at the writer naming the holder.
    const other = { MAESTRO_SESSION_ID: "other-session", MAESTRO_SESSION_PID: String(process.pid) };
    const third = await graphRun(fixture, ["--file", await writeGraph(graphs, "writer-first", writerFirst)]);
    void third.envelope;
    const heldRun = await graphRun(fixture, ["--file", await writeGraph(graphs, "sibling-first", siblingFirst)]);
    expect((await graphResult(fixture, heldRun.run, "a", "done")).exitCode).toBe(0);
    const refused = failure(await runCli(fixture, ["graph", "next", heldRun.run, "--json"], other));
    expect(refused.code).toBe("LEASE_HELD");
    expect(refused.holder).toBe("test-session");
    expect(refused.message).toContain("test-session");
    expect(refused.message).toContain("fix");
    expect(data<{ work: { heldBy: string } }>(await runCli(fixture, ["work", "show", heldRun.run, "--json"])).work.heldBy).toBe("test-session");
    expect((await graphNext(fixture, heldRun.run)).nodes.map((node) => node.ref)).toEqual(["fix"]);

    // A run whose driver died has no holder; the next driver takes it (d99 refuses only another holder).
    const dead = { MAESTRO_SESSION_ID: "dead-driver", MAESTRO_SESSION_PID: "2147483000" };
    const orphan = await graphRun(fixture, ["--file", await writeGraph(graphs, "sibling-first", siblingFirst)], dead);
    expect((await graphResult(fixture, orphan.run, "a", "done", [], dead)).exitCode).toBe(0);
    expect((await graphNext(fixture, orphan.run, other)).nodes.map((node) => node.ref)).toEqual(["fix"]);
    expect(data<{ work: { heldBy: string } }>(await runCli(fixture, ["work", "show", orphan.run, "--json"])).work.heldBy).toBe("other-session");

    expect((await runTool(["git", "rev-parse", "HEAD"], fixture.repo)).stdout.trim()).toBe(head);
    expect((await runTool(["git", "status", "--porcelain", "--untracked-files=no"], fixture.repo)).stdout.trim()).toBe("");
  });
}, 30_000);
