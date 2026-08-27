import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { existsSync } from "node:fs";
import {
  idFrom,
  initializeGitRepository,
  prepareInstallFixture,
  runCli,
  runCliAt,
  runTool,
  type Fixture,
  withFixture,
} from "./helpers.ts";

function session(id: string, pid = process.pid): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(pid) };
}

function probeDatabase(fixture: Fixture): Database {
  return new Database(join(fixture.repo, ".maestro", "maestro.db"));
}

test("220 the former system author id records as an ordinary session", async () => {
  await withFixture(async (fixture) => {
    const recorded = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      session("supervisor"),
    );
    expect(recorded.exitCode).toBe(0);

    const database = probeDatabase(fixture);
    try {
      expect(
        database
          .query<{ count: number }, [string]>("SELECT count(*) AS count FROM sessions WHERE id = ?")
          .get("supervisor")?.count,
      ).toBe(1);
    } finally {
      database.close();
    }
  });
});

const legacySchema = `
  CREATE TABLE schema_version (version INTEGER NOT NULL);
  INSERT INTO schema_version (version) VALUES (1);
  CREATE TABLE cards (
    id TEXT PRIMARY KEY NOT NULL, card_type TEXT NOT NULL, parent TEXT, status TEXT NOT NULL,
    title TEXT NOT NULL, record_file TEXT NOT NULL, card_yaml TEXT NOT NULL,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, imported_at TEXT NOT NULL
  );
  INSERT INTO cards VALUES ('old-card', 'feature', NULL, 'shipped', 'Old card', 'card.yaml',
    'id: old-card', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
  CREATE TABLE card_files (
    card_id TEXT NOT NULL, path TEXT NOT NULL, mode INTEGER NOT NULL, contents BLOB NOT NULL,
    sha256 TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY (card_id, path)
  );
`;

function writeLegacyStore(fixture: Fixture): void {
  const legacy = new Database(join(fixture.repo, ".maestro", "store.sqlite"), { create: true });
  try {
    legacy.exec(legacySchema);
  } finally {
    legacy.close();
  }
}

test("223 doctor names the old store a migrating repo still carries", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const clean = await runCli(fixture, ["doctor"], { PATH: path });
    expect(clean.exitCode).toBe(0);
    expect(clean.stdout).not.toContain("legacy store");

    writeLegacyStore(fixture);
    const carrying = await runCli(fixture, ["doctor"], { PATH: path });
    expect(carrying.exitCode).toBe(0);
    expect(carrying.stdout).toContain("legacy store: 1 card(s) not imported");
    expect(carrying.stdout).toContain("maestro import rust");

    expect((await runCli(fixture, ["import", "rust"])).exitCode).toBe(0);
    const imported = await runCli(fixture, ["doctor"], { PATH: path });
    expect(imported.stdout).toContain("legacy store: imported");
    expect(imported.stdout).not.toContain("not imported");
  });
});

test("224 the import says how to read what it just imported", async () => {
  await withFixture(async (fixture) => {
    writeLegacyStore(fixture);
    const imported = await runCli(fixture, ["import", "rust"]);
    expect(imported.exitCode).toBe(0);
    expect(imported.stdout).toContain("read them: maestro legacy show old-card");
  });
});

test("225 the codex check verifies hook declarations but never guesses trusted hashes", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const hooksPath = join(fixture.repo, ".codex", "hooks.json");
    const installed = JSON.parse(await Bun.file(hooksPath).text()) as
      { hooks: Record<string, unknown> };
    expect(Object.keys(installed.hooks).sort()).toEqual(["SessionStart", "UserPromptSubmit"]);

    await mkdir(join(fixture.home, ".codex"), { recursive: true });
    await writeFile(
      join(fixture.home, ".codex", "config.toml"),
      `[hooks.state."${hooksPath}:session_start:0:0"]\ntrusted_hash = "sha256:deadbeef"\n`,
    );
    const partial = await runCli(fixture, ["doctor"], { PATH: path });
    expect(partial.stdout).toContain("codex hooks: unverified");

    await writeFile(
      join(fixture.home, ".codex", "config.toml"),
      `[hooks.state."${hooksPath}:session_start:0:0"]\ntrusted_hash = "sha256:deadbeef"\n` +
        `[hooks.state."${hooksPath}:user_prompt_submit:0:0"]\ntrusted_hash = "sha256:deadbeef"\n`,
    );
    const recorded = await runCli(fixture, ["doctor"], { PATH: path });
    expect(recorded.stdout).toContain("codex hooks: recorded by Codex");
    expect(recorded.stdout).not.toContain("codex hooks: trusted");

    delete installed.hooks.UserPromptSubmit;
    await writeFile(hooksPath, JSON.stringify(installed, null, 2));
    const stale = await runCli(fixture, ["doctor"], { PATH: path });
    expect(stale.stdout).toContain("codex hooks: stale");
    expect(stale.stdout).toContain("maestro install");
  });
});

test("230 concurrent dispatch acceptance has exactly one winner", async () => {
  await withFixture(async (fixture) => {
    for (let attempt = 0; attempt < 8; attempt += 1) {
      const work = idFrom(
        await runCli(
          fixture,
          ["work", "add", `accept race ${attempt}`, "--atomic-reason", "race fixture"],
        ),
      );
      const opened = await runCli(fixture, [
        "dispatch",
        "open",
        work,
        "--objective",
        "accept once",
        "--owned-scope",
        "scratch store",
        "--excluded-scope",
        "source edits",
        "--mutation",
        "no-write",
        "--stop-condition",
        "accepted",
        "--lane",
        "delivery",
        "--evidence-required",
        "source proof",
        "--pane",
        "w1:pD",
      ]);
      expect(opened.exitCode).toBe(0);
      const dispatch = opened.stdout.match(/\bx\d+\b/)?.[0];
      if (!dispatch) throw new Error(`missing dispatch id: ${opened.stdout}`);

      const contenders = await Promise.all([
        runCli(fixture, ["dispatch", "accept", dispatch], session(`accept-a-${attempt}`)),
        runCli(fixture, ["dispatch", "accept", dispatch], session(`accept-b-${attempt}`)),
      ]);
      const winners = contenders.filter((result) => result.exitCode === 0);
      const losers = contenders.filter((result) => result.exitCode !== 0);
      expect(winners).toHaveLength(1);
      expect(losers).toHaveLength(1);
      expect(losers[0]?.stderr).toContain("DISPATCH_CLAIMED");
      const winnerSession = contenders[0]?.exitCode === 0
        ? `accept-a-${attempt}`
        : `accept-b-${attempt}`;

      const database = probeDatabase(fixture);
      try {
        expect(
          database
            .query<{ claimed_by: string | null; held_by: string | null }, [string]>(
              "SELECT claimed_by, held_by FROM dispatches WHERE id = ?",
            )
            .get(dispatch),
        ).toEqual({ claimed_by: winnerSession, held_by: null });
        expect(
          database
            .query<{ count: number }, [string]>(
              "SELECT count(*) AS count FROM event_log WHERE type = 'dispatch.accept' AND entity_id = ?",
            )
            .get(dispatch)?.count,
        ).toBe(1);
      } finally {
        database.close();
      }

      expect(
        (
          await runCli(fixture, [
            "dispatch",
            "confirm",
            dispatch,
            "--session",
            winnerSession,
          ])
        ).exitCode,
      ).toBe(0);
      const confirmed = probeDatabase(fixture);
      try {
        expect(
          confirmed
            .query<{ claimed_by: string | null; held_by: string | null }, [string]>(
              "SELECT claimed_by, held_by FROM dispatches WHERE id = ?",
            )
            .get(dispatch),
        ).toEqual({ claimed_by: null, held_by: winnerSession });
        expect(
          confirmed
            .query<{ count: number }, [string]>(
              "SELECT count(*) AS count FROM event_log WHERE type = 'dispatch.confirm' AND entity_id = ?",
            )
            .get(dispatch)?.count,
        ).toBe(1);
      } finally {
        confirmed.close();
      }
    }
  });
});

test("232 install writes Codex wiring into the git main worktree, where Codex reads it", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const linked = join(fixture.root, "linked");
    expect(
      (await runTool(["git", "worktree", "add", "-b", "feature", linked], fixture.repo)).exitCode,
    ).toBe(0);

    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCliAt(fixture, linked, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);

    // Codex resolves project config to the main worktree, so wiring written only
    // into the linked worktree is never read (d39).
    const mainWiring = JSON.parse(
      await Bun.file(join(fixture.repo, ".codex", "hooks.json")).text(),
    ) as { hooks: Record<string, unknown> };
    expect(Object.keys(mainWiring.hooks).sort()).toEqual(["SessionStart", "UserPromptSubmit"]);
    expect(existsSync(join(fixture.repo, ".codex", "hooks", "maestro-record.ts"))).toBe(true);
    expect(installed.stdout).toContain(join(fixture.repo, ".codex"));
  });
});

test("233 the codex check reads the wiring Codex reads, not the linked worktree's copy", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const linked = join(fixture.root, "linked");
    expect(
      (await runTool(["git", "worktree", "add", "-b", "feature", linked], fixture.repo)).exitCode,
    ).toBe(0);
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCliAt(fixture, linked, ["install"], { PATH: path })).exitCode).toBe(0);

    const mainHooks = join(fixture.repo, ".codex", "hooks.json");
    const wiring = JSON.parse(await Bun.file(mainHooks).text()) as
      { hooks: Record<string, unknown> };
    delete wiring.hooks.UserPromptSubmit;
    await writeFile(mainHooks, JSON.stringify(wiring, null, 2));

    // The linked worktree's own copy still declares UserPromptSubmit; the check
    // must not be fooled by it.
    const linkedWiring = JSON.parse(await Bun.file(join(linked, ".codex", "hooks.json")).text()) as
      { hooks: Record<string, unknown> };
    expect(Object.keys(linkedWiring.hooks)).toContain("UserPromptSubmit");

    const diagnosed = await runCliAt(fixture, linked, ["doctor"], { PATH: path });
    expect(diagnosed.stdout).toContain("codex hooks: stale");
  });
});
