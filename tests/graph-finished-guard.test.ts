import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, failure, graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

// Live row 10 (run w654, 2026-09-05): a late result on a still-issued sibling
// fired a loop past limits.loops and finish() rewrote the FAILED run as a
// LIMIT stop, so the failure vanished from the envelope, the evidence and the
// journal.
const graph = `---
name: late
description: an agent in flight beside a function that fails, with a loop the late result would fire
limits: {loops: 0}
nodes:
  start: {kind: function, command: "printf ok"}
  a: {kind: agent, profile: tester}
  boom: {kind: function, command: "echo boom >&2; exit 1"}
edges:
  - {from: start, to: a}
  - {from: start, to: boom}
  - {from: a, to: a, max_rounds: 3}
---

## a

A at round {round}.
`;

test("graph-finished-guard: a finished run refuses a late result with INVALID_STATE and its FAILED outcome stays in the envelope, the work evidence and the journal (live row 10 finding)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const path = await writeGraph(join(fixture.root, "graphs"), "late", graph);
    const { run, envelope } = await graphRun(fixture, ["--file", path]);
    expect(envelope.done).toBe(true);
    expect(envelope.failed?.node).toBe("boom");
    expect(envelope.stopped).toBeUndefined();

    const late = failure(await graphResult(fixture, run, "a", "late answer"));
    expect(late.code).toBe("INVALID_STATE");
    expect(late.message).toContain("FAILED:boom");

    const after = await graphNext(fixture, run);
    expect(after.done).toBe(true);
    expect(after.failed?.node).toBe("boom");
    expect(after.stopped).toBeUndefined();
    const work = data<{ work: { evidence: string; state: string } }>(await runCli(fixture, ["work", "show", run, "--json"])).work;
    expect(work.state).toBe("done");
    expect(work.evidence).toContain("FAILED:boom");
    expect(work.evidence).not.toContain("LIMIT");
    const events = data<{ events: Array<{ type: string }> }>(await runCli(fixture, ["trace", run, "--json"])).events.map((event) => event.type);
    expect(events.filter((type) => type === "graph.done" || type === "graph.stopped")).toEqual([]);
    expect(events.filter((type) => type === "graph.failed" || type === "graph.node.failed")).toEqual(["graph.node.failed", "graph.failed"]);
    expect(events.at(-1)).toBe("graph.failed");
  });
}, 30_000);
