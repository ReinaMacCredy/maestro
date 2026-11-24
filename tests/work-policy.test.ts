import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { realpathSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, initializeGitRepository, runCli, runTool, withFixture, workEvents, type Fixture } from "./helpers.ts";

test("630 policy-proof blocks a claim without matching proof", async () => {
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

test("631 disabling policy-proof removes its flags while core evidence still completes verbatim", async () => {
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

test("427 work done and show round-trip an optional candidate into the completion event", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "candidate completion", "--kind", "idea"]),
    );
    expect((await runCli(fixture, ["work", "start", work])).exitCode).toBe(0);
    const candidate = "tree digest: sha256:def456";

    const completed = await runCli(fixture, [
      "work",
      "done",
      work,
      "--evidence",
      "source: candidate round-trip",
      "--candidate",
      candidate,
    ]);
    expect(completed.exitCode).toBe(0);

    const shown = await runCli(fixture, ["work", "show", work]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`candidate: ${candidate}`);

    expect(workEvents(fixture, work)).toContain(`\"candidate\":\"${candidate}\"`);
  });
});

test("--atomic-reason is an unknown flag and a stored atomic reason still shows (d880)", async () => {
  await withFixture(async (fixture) => {
    for (const verb of [["work", "add", "late"], ["work", "start", "w1"], ["work", "done", "w1", "--evidence", "x"]]) {
      const refused = await runCli(fixture, [...verb, "--atomic-reason", "one edit"]);
      expect(JSON.parse(refused.stderr).error.code).toBe("UNKNOWN_FLAG");
    }
    const id = idFrom(await runCli(fixture, ["work", "add", "old card"]));
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE work SET atomic_reason = 'one edit' WHERE id = ?").run(id);
    database.close();
    expect((await runCli(fixture, ["work", "show", id])).stdout).toContain("atomic reason: one edit");
    const json = await runCli(fixture, ["work", "show", id, "--json"]);
    expect(JSON.parse(json.stdout).data.work.atomicReason).toBe("one edit");
  });
});

test("633 work list names the blocker of blocked work and clears it after its blocker completes", async () => {
  await withFixture(async (fixture) => {
    const first = idFrom(await runCli(fixture, ["work", "add", "first", "--kind", "idea"]));
    const second = idFrom(
      await runCli(fixture, ["work", "add", "second", "--kind", "idea", "--blocked-by", first]),
    );
    type Listed = { blockedBy: Array<{ id: string; state: string }>; gate: unknown; id: string };
    const listed = async (): Promise<Map<string, Listed>> => {
      const result = await runCli(fixture, ["work", "list", "--json"]);
      expect(result.exitCode).toBe(0);
      const works = (JSON.parse(result.stdout).data as { works: Listed[] }).works;
      return new Map(works.map((work) => [work.id, work]));
    };

    const before = await listed();
    expect(before.get(first)).toEqual(expect.objectContaining({ blockedBy: [], gate: null }));
    expect(before.get(second)?.blockedBy).toEqual([{ id: first, state: "open" }]);
    const refused = await runCli(fixture, ["work", "start", second]);
    expect((JSON.parse(refused.stderr) as { error: { command: string; message: string } }).error)
      .toEqual(expect.objectContaining({
        command: `maestro work start ${first}`,
        message:
          `${second} is blocked by unresolved work: ${first} [open]; ` +
          `run: maestro work start ${first}`,
      }));

    expect((await runCli(fixture, ["work", "start", first])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", first, "--evidence", "finished"])).exitCode).toBe(0);
    const after = await listed();
    expect(after.get(second)).toEqual(expect.objectContaining({ blockedBy: [], gate: null }));
  });
});

test("67 work list separates startable work from policy-breakdown gated work", async () => {
  await withFixture(async (fixture) => {
    const gated = idFrom(
      await runCli(fixture, ["work", "add", "gated root", "--kind", "feature"]),
    );
    const startable = idFrom(
      await runCli(fixture, ["work", "add", "startable idea", "--kind", "idea"]),
    );
    type Listed = { gate: { command: string | null; origin: string; reason: string } | null; id: string };
    const gateOf = async (id: string): Promise<Listed["gate"] | undefined> => {
      const json = await runCli(fixture, ["work", "list", "--json"]);
      expect(json.exitCode).toBe(0);
      return (JSON.parse(json.stdout).data as { works: Listed[] }).works
        .find((work) => work.id === id)?.gate;
    };

    expect(await gateOf(startable)).toBeNull();
    expect(await gateOf(gated)).toBeNull();

    const child = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "gated child",
        "--kind",
        "task",
        "--parent",
        gated,
      ]),
    );
    // With an OPEN child the parent is gated (open-children predicate).
    expect(await gateOf(gated)).not.toBeNull();

    expect((await runCli(fixture, ["work", "start", child])).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "done", child, "--evidence", "child done"])).exitCode,
    ).toBe(0);

    expect(await gateOf(gated)).toBeNull();
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
      const passivelyOpen = await runCli(fixture, ["work", "show", id], {
        MAESTRO_SESSION_ID: "session-b",
        MAESTRO_SESSION_PID: String(process.pid),
      });
      expect(passivelyOpen.stdout).toContain(`${id} [open]`);
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

test("294 INSERT OR REPLACE cannot rewrite an existing event log row", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["work", "add", "append-only replacement", "--kind", "idea"])).exitCode)
      .toBe(0);
    const { Store } = await import("../src/kernel/store.ts");
    const store = new Store(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      const original = store.database
        .query<Record<string, unknown> & { id: number }, []>("SELECT * FROM event_log ORDER BY id LIMIT 1")
        .get();
      expect(original).not.toBeNull();
      if (!original) throw new Error("missing original event log row");
      expect(() =>
        store.database
          .query(
            `INSERT OR REPLACE INTO event_log
              (id, type, entity_type, entity_id, session_id, payload, created_at)
             VALUES (?, 'rewritten', NULL, NULL, NULL, '{}', ?)`,
          )
          .run(original?.id, new Date().toISOString())
      ).toThrow("event log is append-only");
      expect(
        store.database
          .query<Record<string, unknown>, [number]>("SELECT * FROM event_log WHERE id = ?")
          .get(original?.id),
      ).toEqual(original);
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

test("30 proof blocks name concrete unblocking commands", async () => {
  await withFixture(async (fixture) => {
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

    expect(proof.exitCode).not.toBe(0);
    expect(proof.stderr).toContain("work done");
    expect(proof.stderr).toContain(proofWork);
    expect(proof.stderr).toContain("--proof");
  });
});

test("work add --start creates the card and holds its lease in one command (d919)", async () => {
  await withFixture(async (fixture) => {
    const session = { MAESTRO_SESSION_ID: "starter", MAESTRO_SESSION_PID: String(process.pid) };
    const added = await runCli(fixture, ["work", "add", "one step", "--start"], session);
    expect(added.exitCode).toBe(0);
    expect(added.stdout).toBe("w1 added: one step\nw1 started by starter\n");
    expect((await runCli(fixture, ["work", "show", "w1"])).stdout).toContain("w1 [active] one step\nkind: task\nheld by: starter");
    expect(workEvents(fixture, "w1").split("\n").map((line) => line.split(" ")[1])).toEqual(["work.add", "work.start"]);
    const taken = await runCli(fixture, ["work", "start", "w1"], { MAESTRO_SESSION_ID: "other", MAESTRO_SESSION_PID: String(process.pid) });
    expect(JSON.parse(taken.stderr).error.code).toBe("LEASE_HELD");

    const plain = await runCli(fixture, ["work", "add", "unheld"]);
    expect(plain.stdout).toBe("w2 added: unheld\n");
    expect((await runCli(fixture, ["work", "show", "w2"])).stdout).toContain("w2 [open] unheld");
  });
});

test("work add --start refuses like work start and leaves no card behind (d919)", async () => {
  await withFixture(async (fixture) => {
    const blocker = idFrom(await runCli(fixture, ["work", "add", "first"]));
    const blocked = await runCli(fixture, ["work", "add", "second", "--blocked-by", blocker, "--start"]);
    expect(JSON.parse(blocked.stderr).error.code).toBe("BLOCKED");
    const sessionless = await runCli(fixture, ["work", "add", "third", "--start"], { MAESTRO_SESSION_NONE: "1" });
    expect(JSON.parse(sessionless.stderr).error.code).toBe("SESSION_REQUIRED");
    const listed = JSON.parse((await runCli(fixture, ["work", "list", "--all", "--json"])).stdout);
    expect(listed.data.works.map((work: { id: string }) => work.id)).toEqual([blocker]);
  });
});

async function cleanGitRepository(fixture: Fixture): Promise<string> {
  await initializeGitRepository(fixture.repo);
  await writeFile(join(fixture.repo, ".gitignore"), ".maestro/\n");
  for (const command of [
    ["git", "add", ".gitignore"],
    ["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", "commit", "-m", "ignore"],
  ]) expect((await runTool(command, fixture.repo)).exitCode).toBe(0);
  return (await runTool(["git", "rev-parse", "HEAD"], fixture.repo)).stdout.trim();
}

type ShownProof = { clean?: boolean | null; command?: string; commit?: string | null; durationMs?: number; exitCode?: number; kind: string };

async function shownProof(fixture: Fixture, id: string): Promise<ShownProof> {
  return JSON.parse((await runCli(fixture, ["work", "show", id, "--json"])).stdout).data.proof;
}

test("work done --check runs the command in the repository root and records it; --json stays one envelope (d883)", async () => {
  await withFixture(async (fixture) => {
    const head = await cleanGitRepository(fixture);
    const id = idFrom(await runCli(fixture, ["work", "add", "checked"]));
    const done = await runCli(fixture, ["work", "done", id, "--check", "echo check-output; pwd -P", "--json"]);
    expect(done.exitCode).toBe(0);
    expect(done.stderr).toContain(`check-output\n${realpathSync(fixture.repo)}\n`);
    const envelope = JSON.parse(done.stdout);
    expect(envelope.data.work.state).toBe("done");
    const proof = await shownProof(fixture, id);
    expect(proof).toEqual({
      kind: "check",
      command: "echo check-output; pwd -P",
      exitCode: 0,
      durationMs: expect.any(Number),
      commit: head,
      clean: true,
    });
    const shown = (await runCli(fixture, ["work", "show", id])).stdout;
    expect(shown).toContain("proof kind: check\ncheck: echo check-output; pwd -P (exit 0, ");
    expect(shown).toContain(`commit ${head}, clean tree)`);
  });
});

test("work done --check records a failing check on a dirty tree and still closes the card (d883)", async () => {
  await withFixture(async (fixture) => {
    await cleanGitRepository(fixture);
    await writeFile(join(fixture.repo, "scratch.txt"), "dirty\n");
    const id = idFrom(await runCli(fixture, ["work", "add", "failing"]));
    const done = await runCli(fixture, ["work", "done", id, "--check", "exit 3"]);
    expect(done.exitCode).toBe(0);
    expect(done.stdout).toContain(`${id} done`);
    expect(done.stdout).toContain("check failed: exit 3 exited 3");
    expect(await shownProof(fixture, id)).toEqual(expect.objectContaining({ kind: "check", exitCode: 3, clean: false }));
    expect((await runCli(fixture, ["work", "show", id])).stdout).toContain(`${id} [done] failing`);
  });
});

test("work done with text only is stamped as text, and a refused close never runs the check (d883)", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "texted"]));
    expect((await runCli(fixture, ["work", "done", id, "--evidence", "read it"])).exitCode).toBe(0);
    expect(await shownProof(fixture, id)).toEqual({ kind: "text" });
    expect((await runCli(fixture, ["work", "show", id])).stdout).toContain("proof kind: text");

    const parent = idFrom(await runCli(fixture, ["work", "add", "parent", "--kind", "feature"]));
    await runCli(fixture, ["work", "add", "child", "--parent", parent]);
    const refused = await runCli(fixture, ["work", "done", parent, "--check", "touch ran"]);
    expect(JSON.parse(refused.stderr).error.origin).toBe("policy-breakdown");
    expect(await Bun.file(join(fixture.repo, "ran")).exists()).toBe(false);
  });
});

test("work done --check runs outside the store write lock, so the check may write the store itself (d883)", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "self noting"]));
    const note = `${process.execPath} ${join(import.meta.dir, "..", "bin", "maestro.ts")} work note ${id} inner`;
    expect((await runCli(fixture, ["work", "done", id, "--check", note])).exitCode).toBe(0);
    expect(await shownProof(fixture, id)).toEqual(expect.objectContaining({ exitCode: 0 }));
    expect((await runCli(fixture, ["work", "show", id])).stdout).toContain("note: inner");
    const killed = idFrom(await runCli(fixture, ["work", "add", "killed"]));
    await runCli(fixture, ["work", "done", killed, "--check", "kill -9 $$"]);
    expect(await shownProof(fixture, killed)).toEqual(expect.objectContaining({ exitCode: 137 }));
  });
});

test("work done --check outside a git repository records no commit and no tree state (d883)", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "loose"]));
    expect((await runCli(fixture, ["work", "done", id, "--check", "true"])).exitCode).toBe(0);
    expect(await shownProof(fixture, id)).toEqual(expect.objectContaining({ kind: "check", exitCode: 0, commit: null, clean: null }));
  });
});
