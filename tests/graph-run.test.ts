import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";
import { data, graphNext, graphResult, graphRun, passthroughGraph, writeGraph, writeProfile } from "./graph-helpers.ts";

test("graph-run: a run is one kind-graph work item held by the session with graph_nodes rows, nodes consume no w-id and never appear in ready, and no policy gates a run (red 2, A4)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const path = await writeGraph(join(fixture.root, "graphs"), "passthrough", passthroughGraph);
    await writeFile(
      join(fixture.repo, ".maestro", "config"),
      `${JSON.stringify({
        plugins: [
          { name: "policy-breakdown", disabled: false },
          { name: "policy-proof", disabled: false },
          { name: "policy-tdd", disabled: false },
        ],
      })}\n`,
    );
    const before = idFrom(await runCli(fixture, ["work", "add", "before the run", "--kind", "idea"]));

    const { run, envelope } = await graphRun(fixture, ["--file", path, "topic=leases"]);
    expect(envelope.done).toBe(false);
    expect(envelope.nodes.map((node) => node.ref)).toEqual(["answer"]);
    expect(envelope.nodes[0]?.prompt).toBe("Answer about leases.");

    const shown = data<{ work: { heldBy: string; kind: string; state: string; title: string } }>(
      await runCli(fixture, ["work", "show", run, "--json"]),
    );
    expect(shown.work).toEqual(expect.objectContaining({ heldBy: "test-session", kind: "graph", state: "active" }));
    expect(shown.work.title).toBe("graph passthrough topic=leases");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const nodes = database
      .query<{ instance_key: string; kind: string; node_id: string; state: string }, [string]>(
        "SELECT node_id, instance_key, kind, state FROM graph_nodes WHERE run_id = ? ORDER BY rowid",
      )
      .all(run);
    expect(nodes).toEqual([{ instance_key: "", kind: "agent", node_id: "answer", state: "issued" }]);
    const runs = database.query<{ count: number }, []>("SELECT count(*) AS count FROM work WHERE kind = 'graph'").get();
    expect(runs?.count).toBe(1);
    database.close();

    // No w-id consumed by nodes: the next card follows the run id directly.
    const after = idFrom(await runCli(fixture, ["work", "add", "after the run", "--kind", "idea"]));
    expect(Number(after.slice(1))).toBe(Number(run.slice(1)) + 1);
    expect(Number(run.slice(1))).toBe(Number(before.slice(1)) + 1);

    const ready = data<{ gated: Array<{ id: string }>; works: Array<{ id: string }> }>(await runCli(fixture, ["ready", "--json"]));
    expect(ready.works.map((work) => work.id)).not.toContain(run);
    expect(ready.gated.map((work) => work.id)).not.toContain(run);

    // The final done crosses no policy gate.
    expect((await graphResult(fixture, run, "answer", "leases have three exits")).exitCode).toBe(0);
    const finished = await graphNext(fixture, run);
    expect(finished.done).toBe(true);
    expect(finished.verdict).toBe("leases have three exits");
    const doneWork = data<{ work: { evidence: string; heldBy: string | null; state: string } }>(
      await runCli(fixture, ["work", "show", run, "--json"]),
    );
    expect(doneWork.work.state).toBe("done");
    expect(doneWork.work.heldBy).toBeNull();
    expect(doneWork.work.evidence).toContain("leases have three exits");
    const events = data<{ events: Array<{ type: string }> }>(await runCli(fixture, ["trace", run, "--json"]));
    expect(events.events.map((event) => event.type)).toEqual([
      "graph.run",
      "graph.node.issued",
      "graph.node.done",
      "graph.done",
    ]);

    // A run whose driver died never counts as an unattended card (advisor F9):
    // with the budget at 1 and every human card held, only the orphaned run
    // could fill the slot.
    await writeFile(
      join(fixture.repo, ".maestro", "config"),
      `${JSON.stringify({ plugins: [{ name: "policy-card-budget", disabled: false, config: { limit: 1 } }] })}\n`,
    );
    for (const id of [before, after]) expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);
    const dead = { MAESTRO_SESSION_ID: "dead-driver", MAESTRO_SESSION_PID: "2147483000" };
    const orphan = await graphRun(fixture, ["--file", path, "topic=orphans"], dead);
    expect(orphan.envelope.done).toBe(false);
    const budget = await runCli(fixture, ["work", "add", "human card beside an orphaned run", "--kind", "idea"]);
    expect(budget.exitCode).toBe(0);
    const orphanReady = data<{ gated: Array<{ id: string }>; works: Array<{ id: string }> }>(await runCli(fixture, ["ready", "--json"]));
    expect(orphanReady.works.map((work) => work.id)).not.toContain(orphan.run);
    expect(orphanReady.gated.map((work) => work.id)).not.toContain(orphan.run);
  });
}, 30_000);
