import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, setPlugin, withFixture, type Fixture } from "./helpers.ts";

const dispatchFlags = [
  "--objective", "return one view",
  "--owned-scope", "read-only fixture",
  "--excluded-scope", "source",
  "--mutation", "no-write",
  "--stop-condition", "one handback",
  "--lane", "scout",
  "--evidence-required", "source: note",
];

async function add(fixture: Fixture, title: string, extra: string[] = [], env = {}) {
  return runCli(fixture, ["work", "add", title, ...extra], env);
}

test("463 policy-card-budget ships disabled: unattended cards pile up until it is enabled", async () => {
  await withFixture(async (fixture) => {
    for (let index = 1; index <= 5; index += 1) {
      expect((await add(fixture, `finding ${index}`)).exitCode).toBe(0);
    }
    const plugins = await runCli(fixture, ["plugin", "list"]);
    expect(plugins.stdout).toMatch(/policy-card-budget.*disabled/);

    expect((await runCli(fixture, ["plugin", "enable", "policy-card-budget"])).exitCode).toBe(0);
    const refused = await add(fixture, "finding 6");
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("GATE_BLOCKED");
    expect(refused.stderr).toContain("policy-card-budget");
  });
});

test("464 the fourth unattended card is refused and the error names the three exits", async () => {
  await withFixture(async (fixture) => {
    await setPlugin(fixture, "policy-card-budget", false);
    const ids: string[] = [];
    for (let index = 1; index <= 3; index += 1) {
      const added = await add(fixture, `finding ${index}`);
      expect(added.exitCode).toBe(0);
      ids.push(idFrom(added));
    }
    const refused = await add(fixture, "finding 4");
    expect(refused.exitCode).not.toBe(0);
    const envelope = JSON.parse(refused.stderr) as {
      error: { code: string; origin?: string; message: string; blockers?: Array<{ id: string }> };
    };
    expect(envelope.error.code).toBe("GATE_BLOCKED");
    expect(envelope.error.origin).toBe("policy-card-budget");
    for (const id of ids) expect(envelope.error.message).toContain(id);
    expect(envelope.error.message).toContain("maestro work done");
    expect(envelope.error.message).toContain("maestro dispatch open");
    expect(envelope.error.message).toContain("maestro work cancel");
    expect(envelope.error.blockers?.map((blocker) => blocker.id)).toEqual(ids);
  });
});

test("465 release, an unaccepted dispatch, a rotated session id, and --parent do not free a slot", async () => {
  await withFixture(async (fixture) => {
    await setPlugin(fixture, "policy-card-budget", false);
    const first = idFrom(await add(fixture, "finding 1"));
    // Holding then releasing (lane.md step 5) leaves the card unattended again.
    expect((await runCli(fixture, ["work", "start", first, "--atomic-reason", "fixture"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "release", first])).exitCode).toBe(0);
    const second = idFrom(await add(fixture, "finding 2"));
    // A dispatch nobody accepted is not a worker.
    expect((await runCli(fixture, ["dispatch", "open", second, ...dispatchFlags, "--pane", "no:such:pane"])).exitCode).toBe(0);
    expect((await add(fixture, "finding 3")).exitCode).toBe(0);

    const rotated = await add(fixture, "finding 4", [], { MAESTRO_SESSION_ID: "another-session" });
    expect(rotated.exitCode).not.toBe(0);
    expect(rotated.stderr).toContain("policy-card-budget");

    const child = await add(fixture, "finding 4 as child", ["--parent", first]);
    expect(child.exitCode).not.toBe(0);
    expect(child.stderr).toContain("policy-card-budget");
  });
});

test("466 a live holder, an accepted dispatch, and a cancelled card each free a slot; a dead holder does not", async () => {
  await withFixture(async (fixture) => {
    await setPlugin(fixture, "policy-card-budget", false);
    const held = idFrom(await add(fixture, "held by a live session"));
    expect((await runCli(fixture, ["work", "start", held, "--atomic-reason", "fixture"])).exitCode).toBe(0);

    const dispatched = idFrom(await add(fixture, "accepted by a lane"));
    expect((await runCli(fixture, [
      "dispatch", "open", dispatched, ...dispatchFlags, "--pane", "w1:p1", "--target-session", "lane-one",
    ])).exitCode).toBe(0);
    expect((await runCli(fixture, ["dispatch", "accept", "x1"], { MAESTRO_SESSION_ID: "lane-one" })).exitCode).toBe(0);

    const dead = idFrom(await add(fixture, "held by a dead session"));
    expect((await runCli(fixture, ["work", "start", dead, "--atomic-reason", "fixture"], {
      MAESTRO_SESSION_ID: "gone",
      MAESTRO_SESSION_PID: "999999",
    })).exitCode).toBe(0);

    // held + dispatched are attended; dead counts as one; two more fill the budget.
    expect((await add(fixture, "unattended 1")).exitCode).toBe(0);
    const third = idFrom(await add(fixture, "unattended 2"));
    const refused = await add(fixture, "unattended 3");
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain(dead);

    expect((await runCli(fixture, ["work", "cancel", third, "--reason", "fixture"])).exitCode).toBe(0);
    expect((await add(fixture, "unattended 3")).exitCode).toBe(0);
  });
});

test("467 breakdown proceeds interleaved: the third child is refused until a sibling is started", async () => {
  await withFixture(async (fixture) => {
    await setPlugin(fixture, "policy-card-budget", false);
    const parent = idFrom(await add(fixture, "feature", ["--kind", "feature"]));
    const first = idFrom(await add(fixture, "child 1", ["--parent", parent]));
    expect((await add(fixture, "child 2", ["--parent", parent])).exitCode).toBe(0);
    const refused = await add(fixture, "child 3", ["--parent", parent]);
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("policy-card-budget");

    expect((await runCli(fixture, ["work", "start", first])).exitCode).toBe(0);
    expect((await add(fixture, "child 3", ["--parent", parent])).exitCode).toBe(0);
  });
});

test("468 the budget reads its limit from the plugin config", async () => {
  await withFixture(async (fixture) => {
    await writeFile(
      join(fixture.repo, ".maestro", "config"),
      `${JSON.stringify({ plugins: [{ name: "policy-card-budget", disabled: false, config: { limit: 1 } }] })}\n`,
    );
    expect((await add(fixture, "only one")).exitCode).toBe(0);
    const refused = await add(fixture, "a second");
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("policy-card-budget");
  });
});

test("479 cancelling a parent cascades through mutable descendants and frees their card budget", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await add(fixture, "cascade parent"));
    const heldChild = idFrom(await add(fixture, "held child", ["--parent", parent]));
    const doneChild = idFrom(await add(fixture, "done child", ["--parent", parent]));
    const cancelledChild = idFrom(
      await add(fixture, "cancelled child", ["--parent", parent]),
    );
    const heldSession = { MAESTRO_SESSION_ID: "held-child", MAESTRO_SESSION_PID: String(process.pid) };
    const doneSession = { MAESTRO_SESSION_ID: "done-child", MAESTRO_SESSION_PID: String(process.pid) };

    expect((await runCli(fixture, ["work", "start", heldChild], heldSession)).exitCode).toBe(0);
    const openGrandchild = idFrom(
      await add(fixture, "open grandchild", ["--parent", heldChild]),
    );
    expect((await runCli(fixture, ["work", "start", doneChild], doneSession)).exitCode).toBe(0);
    expect(
      (
        await runCli(
          fixture,
          ["work", "done", doneChild, "--claim", "finished", "--proof", "fixture"],
          doneSession,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(fixture, [
          "work", "cancel", cancelledChild, "--reason", "already settled",
        ])
      ).exitCode,
    ).toBe(0);

    await setPlugin(fixture, "policy-card-budget", false);
    expect((await add(fixture, "budget filler")).exitCode).toBe(0);
    expect((await add(fixture, "blocked before cascade")).exitCode).not.toBe(0);

    const reason = "parent no longer needed";
    expect(
      (await runCli(fixture, ["work", "cancel", parent, "--reason", reason])).exitCode,
    ).toBe(0);

    const shown = async (id: string) => {
      const result = await runCli(fixture, ["work", "show", id, "--json"]);
      expect(result.exitCode).toBe(0);
      return (JSON.parse(result.stdout) as {
        data: { work: { cancelReason: string | null; heldBy: string | null; state: string } };
      }).data.work;
    };
    expect(await shown(parent)).toMatchObject({ state: "cancelled", cancelReason: reason });
    for (const id of [heldChild, openGrandchild]) {
      expect(await shown(id)).toMatchObject({
        state: "cancelled",
        cancelReason: `parent ${parent} cancelled: ${reason}`,
        heldBy: null,
      });
    }
    expect(await shown(doneChild)).toMatchObject({ state: "done", cancelReason: null });
    expect(await shown(cancelledChild)).toMatchObject({
      state: "cancelled",
      cancelReason: "already settled",
    });

    const ready = await runCli(fixture, ["ready"]);
    expect(ready.exitCode).toBe(0);
    for (const id of [parent, heldChild, openGrandchild, doneChild, cancelledChild]) {
      expect(ready.stdout).not.toContain(id);
    }

    expect((await add(fixture, "freed slot one")).exitCode).toBe(0);
    expect((await add(fixture, "freed slot two")).exitCode).toBe(0);
    expect((await add(fixture, "blocked after two freed slots")).exitCode).not.toBe(0);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const events = database
      .query<{ entity_id: string; payload: string }, []>(
        "SELECT entity_id, payload FROM event_log WHERE type = 'work.cancel' ORDER BY id",
      )
      .all()
      .filter((event) => [parent, heldChild, openGrandchild, doneChild, cancelledChild].includes(event.entity_id));
    database.close();
    expect(events.map((event) => event.entity_id)).toEqual([
      cancelledChild,
      parent,
      heldChild,
      openGrandchild,
    ]);
    expect(events.map((event) => JSON.parse(event.payload).reason)).toEqual([
      "already settled",
      reason,
      `parent ${parent} cancelled: ${reason}`,
      `parent ${parent} cancelled: ${reason}`,
    ]);
  });
});

test("469 [lint] lane.md tells the Lead that a handback finding is not a card (d703)", async () => {
  const room = (await readFile(join(import.meta.dir, "..", "src", "plugins", "room.ts"), "utf8")).replace(/\\`/g, "`");
  const step = room.split("\n11. ")[1]?.split("\n12. ")[0] ?? "";
  expect(step).toMatch(/finding returned in a handback is closed by that handback/);
  expect(step).toMatch(/becomes a card only when it is the next thing the Lead will actually do/);
});
