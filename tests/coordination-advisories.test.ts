import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

function sessionEnvironment(id: string, pid = process.pid): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(pid),
  };
}

test("B3.5 sibling overlap warns for live holders but stays silent for dead or unrelated holders", async () => {
  await withFixture(async (fixture) => {
    const firstParent = idFrom(await runCli(fixture, ["work", "add", "first parent"]));
    const held = idFrom(
      await runCli(fixture, ["work", "add", "held sibling", "--parent", firstParent]),
    );
    const claimed = idFrom(
      await runCli(fixture, ["work", "add", "claimed sibling", "--parent", firstParent]),
    );
    expect((await runCli(fixture, ["work", "start", held], sessionEnvironment("live-a"))).exitCode).toBe(0);

    const overlap = await runCli(
      fixture,
      ["work", "start", claimed],
      sessionEnvironment("live-b"),
    );
    expect(overlap.exitCode).toBe(0);
    expect(overlap.stderr).toContain("[overlap]");
    expect(overlap.stderr).toContain("live-a");
    expect(overlap.stderr).toContain(held);

    const unrelatedParent = idFrom(await runCli(fixture, ["work", "add", "other parent"]));
    const unrelated = idFrom(
      await runCli(fixture, ["work", "add", "unrelated child", "--parent", unrelatedParent]),
    );
    const unrelatedStart = await runCli(
      fixture,
      ["work", "start", unrelated],
      sessionEnvironment("live-c"),
    );
    expect(unrelatedStart.exitCode).toBe(0);
    expect(unrelatedStart.stderr).not.toContain("[overlap]");

    const deadParent = idFrom(await runCli(fixture, ["work", "add", "dead parent"]));
    const deadHeld = idFrom(
      await runCli(fixture, ["work", "add", "dead held", "--parent", deadParent]),
    );
    const deadSibling = idFrom(
      await runCli(fixture, ["work", "add", "dead sibling", "--parent", deadParent]),
    );
    expect(
      (await runCli(fixture, ["work", "start", deadHeld], sessionEnvironment("dead-a", 99_999_999)))
        .exitCode,
    ).toBe(0);
    const deadStart = await runCli(
      fixture,
      ["work", "start", deadSibling],
      sessionEnvironment("live-d"),
    );
    expect(deadStart.exitCode).toBe(0);
    expect(deadStart.stderr).not.toContain("[overlap]");
  });
});

test("B3.6 status and hook briefs show live peers with held work and stay silent when alone", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "peer parent"]));
    const held = idFrom(
      await runCli(fixture, ["work", "add", "peer-held work", "--parent", parent]),
    );
    expect((await runCli(fixture, ["work", "start", held], sessionEnvironment("peer-a"))).exitCode).toBe(0);

    const status = await runCli(fixture, ["status"], sessionEnvironment("peer-b"));
    expect(status.stdout).toContain("live peers:");
    expect(status.stdout).toContain("peer-a");
    expect(status.stdout).toContain(held);

    const brief = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      sessionEnvironment("peer-b"),
    );
    expect(brief.stdout).toContain("live peers:");
    expect(brief.stdout).toContain("peer-a");
    expect(brief.stdout).toContain(held);
  });

  await withFixture(async (fixture) => {
    const status = await runCli(fixture, ["status"], sessionEnvironment("solo"));
    const brief = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      sessionEnvironment("solo"),
    );
    expect(status.stdout).not.toContain("live peers:");
    expect(brief.stdout).not.toContain("live peers:");
  });
});

test("B3.7 status hides dead noise but keeps dead work and dispatch owners", async () => {
  await withFixture(async (fixture) => {
    const held = idFrom(await runCli(fixture, ["work", "add", "dead-held work"]));
    expect(
      (
        await runCli(
          fixture,
          ["work", "start", held, "--atomic-reason", "status fixture"],
          sessionEnvironment("dead-holder", process.pid),
        )
      ).exitCode,
    ).toBe(0);
    const dispatched = idFrom(await runCli(fixture, ["work", "add", "dispatched work"]));
    const opened = await runCli(fixture, [
      "dispatch",
      "open",
      dispatched,
      "--objective",
      "exercise status filtering",
      "--owned-scope",
      "tests",
      "--excluded-scope",
      "production",
      "--mutation",
      "no-write",
      "--stop-condition",
      "status is observed",
      "--lane",
      "scout",
      "--evidence-required",
      "test output",
      "--pane",
      "fixture",
      "--target-session",
      "dead-dispatch",
    ]);
    expect(opened.exitCode).toBe(0);
    for (const [id, pid] of [
      ["dead-dispatch", 99_999_998],
      ["dead-hidden", 99_999_997],
    ] as const) {
      expect(
        (
          await runCli(
            fixture,
            ["hook", "record", "--event", "SessionStart"],
            sessionEnvironment(id, pid),
          )
        ).exitCode,
      ).toBe(0);
    }
    const deadHolderDatabase = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    deadHolderDatabase.query("UPDATE sessions SET pid = ?, anchor = 'pid' WHERE id = ?")
      .run(99_999_999, "dead-holder");
    deadHolderDatabase.close();

    const status = await runCli(fixture, ["status"]);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain("dead-holder [dead]");
    expect(status.stdout).toContain(`holds: ${held}`);
    expect(status.stdout).toContain("dead-dispatch [dead]");
    expect(status.stdout).not.toContain("dead-hidden [dead]");
    expect(status.stdout.trim().split("\n").at(-1)).toBe(
      "1 dead sessions hidden (2 hold work); --all to list",
    );

    const all = await runCli(fixture, ["status", "--all"]);
    expect(all.exitCode).toBe(0);
    expect(all.stdout).toContain("dead-hidden [dead]");
    expect(all.stdout).not.toContain("dead sessions hidden");

    const json = await runCli(fixture, ["status", "--json"]);
    expect(json.exitCode).toBe(0);
    const data = (JSON.parse(json.stdout) as {
      data: {
        held: Record<string, string[]>;
        hiddenDead: number;
        sessions: Array<{ id: string }>;
      };
    }).data;
    expect(data.sessions.map((session) => session.id)).toContain("dead-holder");
    expect(data.sessions.map((session) => session.id)).toContain("dead-dispatch");
    expect(data.sessions.map((session) => session.id)).not.toContain("dead-hidden");
    expect(data.held["dead-holder"]).toEqual([held]);
    expect(data.held["dead-dispatch"]).toEqual([]);
    expect(data.held).not.toHaveProperty("dead-hidden");
    expect(data.hiddenDead).toBe(1);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    try {
      expect(
        database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM sessions").get()
          ?.count,
      ).toBe(4);
    } finally {
      database.close();
    }
  });
});

test("B3.8 status rejects a work id outside an active SLP v2 team", async () => {
  await withFixture(async (fixture) => {
    const status = await runCli(fixture, ["status", "w1"]);
    expect(status.exitCode).toBe(1);
    expect(status.stderr).toContain("NO_ACTIVE_TEAM");
    expect(status.stderr).toContain("outside a running team use: maestro work show <id>");
  });
});
