import { expect, setDefaultTimeout, test } from "bun:test";
import { Database } from "bun:sqlite";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

setDefaultTimeout(15_000);

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

test("182 ready and work show preserve stable work fields while adding decisions", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "read path item", "--atomic-reason", "fixture"]),
    );
    const readyBefore = await runCli(fixture, ["ready"], session("lane-ready"));
    const decision = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "keep decisions attached to work",
        "--work",
        work,
      ]),
    );

    const readyAfter = await runCli(fixture, ["ready"], session("lane-ready"));
    const showAfter = await runCli(fixture, ["work", "show", work]);

    expect(readyAfter.stdout.split("\n")[0]).toBe(readyBefore.stdout.split("\n")[0]);
    expect(showAfter.stdout).toContain(`decision: ${decision} [draft] keep decisions attached to work`);

    const showJson = await runCli(fixture, ["work", "show", work, "--json"]);
    expect(showJson.exitCode).toBe(0);
    const showEnvelope = JSON.parse(showJson.stdout) as {
      data: {
        decisions: Array<{ id: string }>;
        work: { atomicReason: string | null; id: string; kind: string; state: string; title: string };
      };
    };
    expect(showEnvelope.data.work).toEqual(expect.objectContaining({
      atomicReason: "fixture",
      id: work,
      kind: "task",
      state: "open",
      title: "read path item",
    }));
    expect(showEnvelope.data.decisions.map((item) => item.id)).toContain(decision);
  });
});

test("326 a start rejected by a gate leaves atomic_reason untouched", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(await runCli(fixture, ["work", "add", "atomic rollback target"]));
    expect(
      (await runCli(fixture, ["work", "add", "open child", "--kind", "task", "--parent", work]))
        .exitCode,
    ).toBe(0);

    const started = await runCli(fixture, ["work", "start", work, "--atomic-reason", "single-file change"]);
    expect(started.exitCode).not.toBe(0);
    expect(started.stderr).toContain("GATE_BLOCKED");

    const db = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const row = db.query("SELECT state, held_by, atomic_reason FROM work WHERE id = ?").get(work) as {
      state: string; held_by: string | null; atomic_reason: string | null;
    };
    db.close();
    expect(row).toEqual({ state: "open", held_by: null, atomic_reason: null });
  });
});
