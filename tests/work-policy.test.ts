import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

test("7 policy-proof blocks a claim without matching proof", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(fixture, ["work", "add", "inspect", "--kind", "idea"]);
    const id = idFrom(added);
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);

    const result = await runCli(fixture, [
      "work",
      "done",
      id,
      "--claim",
      "checks pass",
      "--evidence",
      "raw evidence without a proof",
    ]);

    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("policy-proof");
  });
});

test("8 disabling policy-proof removes its flags while core evidence still completes verbatim", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["plugin", "disable", "policy-proof"])).exitCode).toBe(0);
    const added = await runCli(fixture, ["work", "add", "inspect", "--kind", "idea"]);
    const id = idFrom(added);
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);
    const evidence = "raw: checks=missing; keep  spacing & punctuation";

    const rejected = await runCli(fixture, [
      "work",
      "done",
      id,
      "--claim",
      "checks pass",
      "--evidence",
      evidence,
    ]);
    const completed = await runCli(fixture, [
      "work",
      "done",
      id,
      "--evidence",
      evidence,
    ]);
    const shown = await runCli(fixture, ["work", "show", id]);

    expect(rejected.exitCode).not.toBe(0);
    expect(rejected.stderr).toContain("unknown flag");
    expect(rejected.stderr).toContain("--claim");
    expect(completed.exitCode).toBe(0);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(evidence);
  });
});

test("9 policy-breakdown blocks only parentless childless write-like work", async () => {
  await withFixture(async (fixture) => {
    const root = idFrom(
      await runCli(fixture, ["work", "add", "root implementation", "--kind", "feature"]),
    );

    const blockedRoot = await runCli(fixture, ["work", "start", root]);
    const child = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "child implementation",
        "--kind",
        "feature",
        "--parent",
        root,
      ]),
    );
    const startedChild = await runCli(fixture, ["work", "start", child]);

    expect(blockedRoot.exitCode).not.toBe(0);
    expect(blockedRoot.stderr).toContain("policy-breakdown");
    expect(startedChild.exitCode).toBe(0);
    expect(startedChild.stderr).not.toContain("policy-breakdown");
  });
});

test("10 ready excludes blocked work and promotes it after its blocker completes", async () => {
  await withFixture(async (fixture) => {
    const first = idFrom(await runCli(fixture, ["work", "add", "first", "--kind", "idea"]));
    const second = idFrom(
      await runCli(fixture, ["work", "add", "second", "--kind", "idea", "--blocked-by", first]),
    );

    const before = await runCli(fixture, ["ready", "--json"]);
    expect(before.exitCode).toBe(0);
    const beforeData = JSON.parse(before.stdout).data as {
      gated: unknown[];
      works: Array<{ id: string }>;
    };
    expect(beforeData.works.map((work) => work.id)).toContain(first);
    expect(beforeData.works.map((work) => work.id)).not.toContain(second);
    expect(beforeData.gated).toEqual([]);

    expect((await runCli(fixture, ["work", "start", first])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", first, "--evidence", "finished"])).exitCode).toBe(0);
    const after = await runCli(fixture, ["ready", "--json"]);
    expect(after.exitCode).toBe(0);
    const afterData = JSON.parse(after.stdout).data as {
      gated: unknown[];
      works: Array<{ id: string }>;
    };

    expect(afterData.works.map((work) => work.id)).toContain(second);
    expect(afterData.gated).toEqual([]);
  });
});

test("67 ready separates startable work from policy-breakdown gated work", async () => {
  await withFixture(async (fixture) => {
    const gated = idFrom(
      await runCli(fixture, ["work", "add", "gated root", "--kind", "feature"]),
    );
    const startable = idFrom(
      await runCli(fixture, ["work", "add", "startable idea", "--kind", "idea"]),
    );
    const reason =
      `parentless write-like work requires a child breakdown; run: maestro work add ` +
      `"<child>" --parent ${gated} --kind task; for new atomic work use ` +
      `--atomic-reason "<reason>"`;

    const blocked = await runCli(fixture, ["work", "start", gated]);
    const human = await runCli(fixture, ["ready"]);
    const json = await runCli(fixture, ["ready", "--json"]);
    expect(json.exitCode).toBe(0);
    const data = JSON.parse(json.stdout).data as {
      gated: Array<{ id: string; origin: string; reason: string; title: string }>;
      works: Array<{ id: string }>;
    };

    expect(blocked.exitCode).not.toBe(0);
    expect((JSON.parse(blocked.stderr) as { error: { reason: string } }).error.reason).toBe(reason);
    expect(data.works.map((work) => work.id)).toEqual([startable]);
    expect(data.gated).toEqual([
      { id: gated, title: "gated root", origin: "policy-breakdown", reason },
    ]);
    expect(human.stdout.indexOf(startable)).toBeLessThan(human.stdout.indexOf(gated));
    expect(human.stdout).toContain(reason);

    expect(
      (
        await runCli(fixture, [
          "work",
          "add",
          "gated child",
          "--kind",
          "task",
          "--parent",
          gated,
        ])
      ).exitCode,
    ).toBe(0);
    const after = await runCli(fixture, ["ready", "--json"]);
    expect(after.exitCode).toBe(0);
    const afterData = JSON.parse(after.stdout).data as {
      gated: Array<{ id: string }>;
      works: Array<{ id: string }>;
    };
    expect(afterData.works.map((work) => work.id)).toContain(gated);
    expect(afterData.gated.map((work) => work.id)).not.toContain(gated);
    expect((await runCli(fixture, ["work", "start", gated])).exitCode).toBe(0);
  });
});

test("11 live leases refuse a second session and dead-session leases expire passively", async () => {
  await withFixture(async (fixture) => {
    const holder = Bun.spawn(["sleep", "30"]);
    try {
      const id = idFrom(await runCli(fixture, ["work", "add", "shared", "--kind", "idea"]));
      const claimed = await runCli(fixture, ["work", "start", id], {
        MAESTRO_SESSION_ID: "session-a",
        MAESTRO_SESSION_PID: String(holder.pid),
      });
      const refused = await runCli(fixture, ["work", "start", id], {
        MAESTRO_SESSION_ID: "session-b",
        MAESTRO_SESSION_PID: String(process.pid),
      });

      expect(claimed.exitCode).toBe(0);
      expect(refused.exitCode).not.toBe(0);
      expect(refused.stderr).toContain("session-a");
      const completionCollision = await runCli(
        fixture,
        ["work", "done", id, "--evidence", "not mine"],
        {
          MAESTRO_SESSION_ID: "session-b",
          MAESTRO_SESSION_PID: String(process.pid),
        },
      );
      expect(completionCollision.exitCode).not.toBe(0);
      expect(completionCollision.stderr).toContain("session-a");

      holder.kill();
      await holder.exited;
      const passivelyReady = await runCli(fixture, ["ready"], {
        MAESTRO_SESSION_ID: "session-b",
        MAESTRO_SESSION_PID: String(process.pid),
      });
      expect(passivelyReady.stdout).toContain(id);
      const reclaimed = await runCli(fixture, ["work", "start", id], {
        MAESTRO_SESSION_ID: "session-b",
        MAESTRO_SESSION_PID: String(process.pid),
      });
      expect(reclaimed.exitCode).toBe(0);
    } finally {
      holder.kill();
    }
  });
});

test("12 write verbs append events and the store rejects mutation of prior log rows", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(fixture, ["work", "add", "logged", "--kind", "idea"]);
    expect(added.exitCode).toBe(0);

    const { Store } = await import("../src/kernel/store.ts");
    const store = new Store(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      const count = store.database
        .query<{ count: number }, []>("SELECT count(*) AS count FROM event_log")
        .get()?.count;
      expect(count).toBeGreaterThan(0);
      expect(() => store.database.run("UPDATE event_log SET type = 'changed'")).toThrow();
      expect(() => store.database.run("DELETE FROM event_log")).toThrow();
    } finally {
      store.close();
    }
  });
});

test("22 paired claims and proofs complete work and are recorded in evidence", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "paired proof", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);

    const completed = await runCli(fixture, [
      "work",
      "done",
      id,
      "--claim",
      "tests pass",
      "--proof",
      "bun test: 1 pass",
    ]);
    const shown = await runCli(fixture, ["work", "show", id]);

    expect(completed.exitCode).toBe(0);
    expect(shown.stdout).toContain("tests pass");
    expect(shown.stdout).toContain("bun test: 1 pass");
  });
});

test("23 policy-proof blocks completion without evidence or claims", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "empty completion", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);

    const completed = await runCli(fixture, ["work", "done", id]);

    expect(completed.exitCode).not.toBe(0);
    expect(completed.stderr).toContain("policy-proof");
  });
});

test("24 unexpected positionals fail with UNKNOWN_ARGUMENT naming the token", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "strict arguments", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);

    const completed = await runCli(fixture, [
      "work",
      "done",
      id,
      "discarded text",
      "--evidence",
      "real evidence",
    ]);
    expect(completed.exitCode).not.toBe(0);
    const error = JSON.parse(completed.stderr.trim());
    expect(error.error.code).toBe("UNKNOWN_ARGUMENT");
    expect(error.error.message).toContain("discarded text");

    const drafted = await runCli(fixture, [
      "decision",
      "draft",
      "kept decision text",
      "discarded decision text",
    ]);
    expect(drafted.exitCode).not.toBe(0);
    const draftError = JSON.parse(drafted.stderr.trim());
    expect(draftError.error.code).toBe("UNKNOWN_ARGUMENT");
    expect(draftError.error.message).toContain("discarded decision text");

    const emptyCreate = await runCli(fixture, [
      "decision",
      "draft",
      "kept empty decision text",
      "",
    ]);
    expect(emptyCreate.exitCode).not.toBe(0);
    const emptyCreateError = JSON.parse(emptyCreate.stderr.trim());
    expect(emptyCreateError.error.code).toBe("UNKNOWN_ARGUMENT");
    expect(emptyCreateError.error.argument).toBe("");

    const editable = idFrom(await runCli(fixture, ["decision", "draft", "editable decision"]));
    const emptyEdit = await runCli(fixture, ["decision", "draft", editable, ""]);
    expect(emptyEdit.exitCode).not.toBe(0);
    const emptyEditError = JSON.parse(emptyEdit.stderr.trim());
    expect(emptyEditError.error.code).toBe("MISSING_ARGUMENT");
  });
});

test("30 breakdown and proof blocks name concrete unblocking commands", async () => {
  await withFixture(async (fixture) => {
    const root = idFrom(
      await runCli(fixture, ["work", "add", "needs breakdown", "--kind", "feature"]),
    );
    const breakdown = await runCli(fixture, ["work", "start", root]);

    const proofWork = idFrom(
      await runCli(fixture, ["work", "add", "needs proof", "--kind", "idea"]),
    );
    expect((await runCli(fixture, ["work", "start", proofWork])).exitCode).toBe(0);
    const proof = await runCli(fixture, [
      "work",
      "done",
      proofWork,
      "--claim",
      "tests pass",
    ]);

    expect(breakdown.exitCode).not.toBe(0);
    expect(breakdown.stderr).toContain("work add");
    expect(breakdown.stderr).toContain("--parent");
    expect(breakdown.stderr).toContain(root);
    expect(breakdown.stderr).toContain("--atomic-reason");
    expect(proof.exitCode).not.toBe(0);
    expect(proof.stderr).toContain("work done");
    expect(proof.stderr).toContain(proofWork);
    expect(proof.stderr).toContain("--proof");
  });
});
