import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

const storeCardId = "feature-runtime-store";
const archiveCardId = "snapshot-runtime-archive";

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
}

function dispatchOpenArgs(work: string, target: string, pane = "w1:p-test"): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Verify the runtime contract",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push, tag, release",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "the runtime assertion passes",
    "--lane",
    "delivery",
    "--evidence-required",
    "source: focused test",
    "--pane",
    pane,
    "--target-session",
    target,
  ];
}

function dispatchId(stdout: string): string {
  const match = stdout.match(/^x\d+/);
  if (!match) throw new Error(`missing dispatch id in stdout: ${stdout}`);
  return match[0];
}

function handbackArgs(dispatch: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "the runtime contract is verified",
    "--proof",
    "source: focused test passes",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
  ];
}

async function writeStoreSource(fixture: Fixture): Promise<string> {
  const path = join(fixture.repo, "runtime-store.sqlite");
  const database = new Database(path, { create: true, strict: true });
  database.exec(`
    CREATE TABLE cards (
      id TEXT PRIMARY KEY NOT NULL,
      card_type TEXT NOT NULL,
      parent TEXT,
      status TEXT NOT NULL,
      title TEXT NOT NULL,
      record_file TEXT NOT NULL,
      card_yaml TEXT NOT NULL,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      imported_at TEXT NOT NULL
    );
    CREATE TABLE card_files (
      card_id TEXT NOT NULL,
      path TEXT NOT NULL,
      mode INTEGER NOT NULL,
      contents BLOB NOT NULL,
      sha256 TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY (card_id, path)
    );
    INSERT INTO cards
      (id, card_type, parent, status, title, record_file, card_yaml,
       created_at, updated_at, imported_at)
    VALUES
      ('${storeCardId}', 'feature', NULL, 'closed', 'Runtime Store Card', 'card.yaml',
       'id: ${storeCardId}\ncard_type: feature\nstatus: closed\ntitle: Runtime Store Card\ndescription: store-runtime-needle\n',
       '2026-08-27T00:00:00Z', '2026-08-27T00:00:00Z', '2026-08-27T00:00:00Z');
  `);
  database.close();
  return path;
}

async function writeArchiveSource(fixture: Fixture): Promise<string> {
  const path = join(fixture.repo, "runtime-archive.sqlite");
  const database = new Database(path, { create: true, strict: true });
  database.exec(`
    CREATE TABLE archived_snapshots (
      id TEXT PRIMARY KEY NOT NULL,
      archived_at TEXT NOT NULL,
      source_relpath TEXT NOT NULL,
      manifest_json TEXT NOT NULL,
      snapshot_zstd BLOB NOT NULL,
      snapshot_sha256 TEXT NOT NULL,
      search_text TEXT NOT NULL,
      last_checked_at TEXT
    );
  `);
  database
    .query(
      `INSERT INTO archived_snapshots
        (id, archived_at, source_relpath, manifest_json, snapshot_zstd,
         snapshot_sha256, search_text, last_checked_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .run(
      archiveCardId,
      "2026-08-27T00:01:00Z",
      `archive/${archiveCardId}`,
      JSON.stringify({ format_version: "maestro.archive.snapshot.v1", files: [] }),
      new Uint8Array([0]),
      "runtime-archive-sha",
      "title: Runtime Archive Snapshot\ndescription: archive-runtime-needle\n",
    );
  database.close();
  return path;
}

test("320 import rust keeps store and archive sources independently replaceable", async () => {
  await withFixture(async (fixture) => {
    const store = await writeStoreSource(fixture);
    const archive = await writeArchiveSource(fixture);

    expect((await runCli(fixture, ["import", "rust", "--path", store])).exitCode).toBe(0);
    expect((await runCli(fixture, ["import", "rust", "--path", archive])).exitCode).toBe(0);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
      strict: true,
    });
    expect(
      database
        .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_cards")
        .get()?.count,
    ).toBe(2);
    expect(
      database
        .query<{ count: number }, []>(
          "SELECT count(*) AS count FROM legacy_cards WHERE card_type = 'archive'",
        )
        .get()?.count,
    ).toBe(1);
    expect(
      database
        .query<{ source: string }, []>("SELECT DISTINCT source FROM legacy_cards ORDER BY source")
        .all()
        .map((row) => row.source),
    ).toEqual(["archive", "store"]);
    database.close();

    expect((await runCli(fixture, ["legacy", "show", storeCardId])).exitCode).toBe(0);
    expect((await runCli(fixture, ["legacy", "show", archiveCardId])).exitCode).toBe(0);
    expect((await runCli(fixture, ["search", "store-runtime-needle"])).stdout).toContain(storeCardId);
    expect((await runCli(fixture, ["search", "archive-runtime-needle"])).stdout).toContain(
      archiveCardId,
    );

    expect((await runCli(fixture, ["import", "rust", "--path", archive])).exitCode).toBe(0);
    const afterRepeat = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
      strict: true,
    });
    expect(
      afterRepeat
        .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_cards")
        .get()?.count,
    ).toBe(2);
    afterRepeat.close();

    const promoted = await runCli(fixture, ["import", "rust", "--path", store, "--promote"]);
    const rerun = await runCli(fixture, ["import", "rust", "--path", store, "--promote"]);
    expect(promoted.exitCode).toBe(0);
    expect(rerun.exitCode).toBe(0);
    expect(rerun.stdout).toContain("0 work created");
    expect(rerun.stdout).toContain("0 decisions created");
    expect((await runCli(fixture, ["legacy", "show", archiveCardId])).exitCode).toBe(0);
  });
});

test("321 handback keeps the work lease until the owning session completes work", async () => {
  await withFixture(async (fixture) => {
    const owner = session("runtime-owner");
    const other = session("runtime-other");
    const work = idFrom(
      await runCli(
        fixture,
        ["work", "add", "handback lease", "--atomic-reason", "fixture"],
        owner,
      ),
    );
    expect((await runCli(fixture, ["work", "start", work], owner)).exitCode).toBe(0);
    const opened = await runCli(fixture, dispatchOpenArgs(work, "runtime-owner"), owner);
    const dispatch = dispatchId(opened.stdout);
    expect(opened.exitCode).toBe(0);
    expect((await runCli(fixture, ["dispatch", "accept", dispatch], owner)).exitCode).toBe(0);
    expect((await runCli(fixture, handbackArgs(dispatch), owner)).exitCode).toBe(0);

    const refused = await runCli(
      fixture,
      ["work", "done", work, "--evidence", "source: wrong session"],
      other,
    );
    expect(refused.exitCode).not.toBe(0);
    expect(JSON.parse(refused.stderr).error.code).toBe("LEASE_HELD");

    const completed = await runCli(
      fixture,
      ["work", "done", work, "--evidence", "source: owner session"],
      owner,
    );
    expect(completed.exitCode).toBe(0);
  });
});

test("322 help --help prints top-level help without changing existing help forms", async () => {
  await withFixture(async (fixture) => {
    const topLevel = await runCli(fixture, ["help"]);
    const flagged = await runCli(fixture, ["help", "--help"]);
    const perVerb = await runCli(fixture, ["help", "work"]);

    expect(topLevel.exitCode).toBe(0);
    expect(flagged.exitCode).toBe(0);
    expect(flagged.stdout).toBe(topLevel.stdout);
    expect(flagged.stdout).toContain("verbs:");
    expect(perVerb.exitCode).toBe(0);
    expect(perVerb.stdout).toContain("usage: maestro work");
  });
});

test("323 cancelled dispatches leave the live council before work start", async () => {
  await withFixture(async (fixture) => {
    const owner = session("council-owner");
    const work = idFrom(
      await runCli(
        fixture,
        ["work", "add", "cancelled council lane", "--atomic-reason", "fixture"],
        owner,
      ),
    );
    const first = dispatchId(
      (await runCli(fixture, dispatchOpenArgs(work, "council-owner"), owner)).stdout,
    );
    const second = dispatchId(
      (await runCli(fixture, dispatchOpenArgs(work, "council-owner"), owner)).stdout,
    );

    expect(
      (
        await runCli(
          fixture,
          ["dispatch", "cancel", first, "--reason", "lane replaced"],
          owner,
        )
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["dispatch", "accept", second], owner)).exitCode).toBe(0);

    const started = await runCli(fixture, ["work", "start", work], owner);
    expect(started.exitCode).toBe(0);
    const listed = await runCli(fixture, ["dispatch", "list", work], owner);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).not.toContain("council:");
  });
});

test("324 unaccepted dispatch attention appears after ten minutes and clears on accept", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "unaccepted dispatch attention",
        "--atomic-reason",
        "fixture",
      ]),
    );
    const oldPane = "w1:p-unaccepted";
    const oldDispatch = dispatchId(
      (
        await runCli(
          fixture,
          dispatchOpenArgs(work, "unaccepted-holder", oldPane),
        )
      ).stdout,
    );
    dispatchId(
      (
        await runCli(
          fixture,
          dispatchOpenArgs(work, "young-holder", "w1:p-young"),
        )
      ).stdout,
    );
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      strict: true,
    });
    database
      .query("UPDATE dispatches SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 11 * 60_000).toISOString(), oldDispatch);
    database.close();

    const attention = await runCli(fixture, ["attention"]);
    expect(attention.exitCode).toBe(0);
    for (const line of [
      `attention DISPATCH_UNACCEPTED dispatch ${oldDispatch}`,
      `observed: ${oldDispatch} opened 11 minutes ago on pane ${oldPane}, never accepted`,
      "evidence: dispatch state open; no session bound to the pane",
      "unknown: whether the brief reached the pane",
      "question: was the stored contract delivered?",
      `smallest action: herdr agent list, then herdr agent prompt <name> with the stored contract from maestro dispatch show ${oldDispatch}; open the prompt with a plain lowercase sentence, never a word a harness could read as a slash command, and confirm agent_status=working before leaving`,
      "human decision needed: no",
    ]) {
      expect(attention.stdout).toContain(line);
    }

    const repeated = await runCli(fixture, ["attention"]);
    expect(repeated.stdout).toContain(`attention DISPATCH_UNACCEPTED dispatch ${oldDispatch}`);
    const recorded = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
      strict: true,
    });
    expect(
      recorded
        .query<{ count: number }, []>(
          "SELECT count(*) AS count FROM attention WHERE kind = 'DISPATCH_UNACCEPTED'",
        )
        .get()?.count,
    ).toBe(1);
    recorded.close();

    const hook = await runCli(fixture, ["hook", "record", "--event", "SessionStart"]);
    expect(hook.stdout).toContain(`attention DISPATCH_UNACCEPTED dispatch ${oldDispatch}`);
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const brief = await runCli(fixture, ["brief"]);
    expect(brief.stdout).toContain(
      `${fixture.repo}: attention DISPATCH_UNACCEPTED dispatch ${oldDispatch}`,
    );

    expect(
      (
        await runCli(
          fixture,
          ["dispatch", "accept", oldDispatch],
          session("unaccepted-holder"),
        )
      ).exitCode,
    ).toBe(0);
    const cleared = await runCli(fixture, ["attention"]);
    expect(cleared.stdout).not.toContain("DISPATCH_UNACCEPTED");
  });
});

test("457 pending claims suppress stale attention while live and route dead claims to confirm", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "pending claim attention", "--atomic-reason", "fixture"]),
    );
    const openArgs = dispatchOpenArgs(work, "unused").slice(0, -2);
    const dispatch = dispatchId((await runCli(fixture, openArgs)).stdout);
    const claimant = session("pending-claimant");
    expect(
      (await runCli(fixture, ["hook", "record", "--event", "SessionStart"], claimant)).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["dispatch", "accept", dispatch], claimant)).exitCode).toBe(0);

    const path = join(fixture.repo, ".maestro", "maestro.db");
    const database = new Database(path);
    database
      .query("UPDATE dispatches SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 11 * 60_000).toISOString(), dispatch);
    database
      .query("UPDATE sessions SET anchor = 'ttl' WHERE id = ?")
      .run("pending-claimant");
    database.close();

    const live = await runCli(
      fixture,
      ["attention", "--json", "--dispatch-stale", "0.000001"],
      session("scanner"),
    );
    expect(live.exitCode).toBe(0);
    expect(live.stdout).not.toContain("DISPATCH_UNACCEPTED");
    expect(live.stdout).not.toContain("DISPATCH_UNRETURNED");

    const stored = new Database(path);
    stored
      .query("UPDATE sessions SET anchor = 'pid', pid = ? WHERE id = ?")
      .run(2147483647, "pending-claimant");
    stored.close();

    const dead = await runCli(
      fixture,
      ["attention", "--json", "--dispatch-stale", "0.000001"],
      session("scanner"),
    );
    expect(dead.exitCode).toBe(0);
    expect(dead.stdout).toContain("DISPATCH_UNACCEPTED");
    expect(dead.stdout).not.toContain("DISPATCH_UNRETURNED");
    expect(dead.stdout).toContain(
      `maestro dispatch confirm ${dispatch} --session pending-claimant`,
    );
  });
});

test("325 a dispatch opened before the handback cannot count as its review", async () => {
  await withFixture(async (fixture) => {
    const owner = session("citing-owner");
    const work = idFrom(
      await runCli(fixture, ["work", "add", "premature citation", "--atomic-reason", "fixture"], owner),
    );
    const first = dispatchId(
      (await runCli(fixture, dispatchOpenArgs(work, "citing-owner"), owner)).stdout,
    );
    const citing = dispatchOpenArgs(work, "citing-owner", "w1:p-cite").map((arg) =>
      arg === "Verify the runtime contract" ? "Verify the runtime contract after h1" : arg
    );
    const second = dispatchId((await runCli(fixture, citing, owner)).stdout);
    expect((await runCli(fixture, ["dispatch", "accept", first], owner)).exitCode).toBe(0);
    const filed = await runCli(
      fixture,
      [
        "handback", "file", first, "--status", "DONE", "--claim", "done", "--proof", "source: fixture",
        "--assumptions", "None", "--residual-risks", "None", "--incidental-findings", "None",
      ],
      owner,
    );
    expect(filed.exitCode).toBe(0);
    expect(filed.stdout).toContain("h1");
    // Return the citing lane too so the council unseals and the packets
    // become reviewable; the premature citation must still not count.
    expect((await runCli(fixture, ["dispatch", "accept", second], owner)).exitCode).toBe(0);
    expect(
      (await runCli(fixture, [
        "handback", "file", second, "--status", "DONE", "--claim", "done", "--proof", "source: fixture",
        "--assumptions", "None", "--residual-risks", "None", "--incidental-findings", "None",
      ], owner)).exitCode,
    ).toBe(0);

    const scan = await runCli(fixture, ["attention", "--json"], owner);
    const detections = (JSON.parse(scan.stdout) as { data: { detections: { kind: string; fingerprint: string }[] } }).data.detections;
    expect(detections.map((d) => d.fingerprint)).toContain(`handback-unreviewed:${first}`);
  });
});
