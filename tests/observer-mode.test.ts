import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { Cli } from "../src/kernel/cli.ts";
import { observerMode } from "../src/plugins/observer-mode.ts";
import { idFrom, runCli, withFixture, writeConfig, writePlugin } from "./helpers.ts";

const withoutSession = { MAESTRO_SESSION_NONE: "1" };
const readOnly = { MAESTRO_READ_ONLY: "1" };

test("208 sessionless work start refuses a lease that cannot survive one read", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "trap check", "--atomic-reason", "fixture"]),
    );

    const started = await runCli(fixture, ["work", "start", work], withoutSession);
    expect(started.exitCode).toBe(1);
    expect(started.stderr).toContain('"code":"SESSION_REQUIRED"');
    expect(started.stderr).toContain("remove MAESTRO_SESSION_NONE");

    const shown = await runCli(fixture, ["work", "show", work], withoutSession);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`${work} [open]`);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    try {
      expect(
        database.query<{ held_by: string | null }, [string]>(
          "SELECT held_by FROM work WHERE id = ?",
        ).get(work)?.held_by,
      ).toBeNull();
    } finally {
      database.close();
    }
  });
});

test("209 every session-attributed write door shares the SESSION_REQUIRED gate", async () => {
  await withFixture(async (fixture) => {
    const commands = [
      ["work", "add"],
      ["work", "start"],
      ["work", "release"],
      ["work", "reclaim"],
      ["work", "note"],
      ["work", "done"],
      ["work", "cancel"],
      ["decision", "draft"],
      ["decision", "lock"],
      ["bundle", "open"],
      ["bundle", "close"],
      ["handoff"],
      ["bundle", "save"],
      ["dispatch", "open"],
      ["dispatch", "accept"],
      ["dispatch", "cancel"],
      ["dispatch", "unseal"],
      ["handback", "file"],
      ["hook", "record"],
      ["plugin", "enable"],
      ["plugin", "disable"],
      ["plugin", "new"],
      ["plugin", "add"],
      ["plugin", "remove"],
      ["install"],
    ];

    for (const command of commands) {
      const result = await runCli(fixture, command, withoutSession);
      expect(result.exitCode, command.join(" ")).toBe(1);
      expect(result.stderr, command.join(" ")).toContain('"code":"SESSION_REQUIRED"');
      expect(result.stderr, command.join(" ")).toContain("remove MAESTRO_SESSION_NONE");
    }
  });
});

test("210 root help documents the read-only observer flag", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["help"], readOnly);

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("MAESTRO_READ_ONLY=1");
  });
});

test("211 read-only observation does not refresh session last_seen", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["version"]);
    const path = join(fixture.repo, ".maestro", "maestro.db");
    const stale = "2020-01-01T00:00:00.000Z";
    const database = new Database(path);
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES ('test-session', 1, 'fixture', ?, 'codex', 'ttl', ?)`,
      )
      .run(stale, fixture.repo);
    database.close();

    const result = await runCli(fixture, ["version"], readOnly);
    expect(result.exitCode).toBe(0);

    const stored = new Database(path, { readonly: true });
    try {
      expect(
        stored.query<{ last_seen: string }, []>(
          "SELECT last_seen FROM sessions WHERE id = 'test-session'",
        ).get()?.last_seen,
      ).toBe(stale);
    } finally {
      stored.close();
    }
  });
});

test("212 read-only work show expires a dead lease only in memory", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(await runCli(fixture, ["work", "add", "observer lease"]));
    const path = join(fixture.repo, ".maestro", "maestro.db");
    const database = new Database(path);
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, anchor, scope)
         VALUES ('dead-holder', 2147483647, 'fixture', ?, 'pid', ?)`,
      )
      .run(new Date().toISOString(), fixture.repo);
    database
      .query("UPDATE work SET state = 'active', held_by = 'dead-holder' WHERE id = ?")
      .run(work);
    database.close();

    const result = await runCli(fixture, ["work", "show", work], readOnly);
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain(`${work} [open]`);

    const stored = new Database(path, { readonly: true });
    try {
      expect(
        stored.query<{ held_by: string | null; state: string }, [string]>(
          "SELECT state, held_by FROM work WHERE id = ?",
        ).get(work),
      ).toEqual({ state: "active", held_by: "dead-holder" });
      expect(
        stored.query<{ count: number }, []>(
          "SELECT count(*) AS count FROM event_log WHERE type = 'work.lease.expire'",
        ).get()?.count,
      ).toBe(0);
    } finally {
      stored.close();
    }
  });
});

test("213 read-only status downgrades shared PID anchors only in memory", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["version"]);
    const path = join(fixture.repo, ".maestro", "maestro.db");
    const stale = "2020-01-01T00:00:00.000Z";
    const database = new Database(path);
    const insert = database.query(
      `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
       VALUES (?, ?, 'fixture', ?, 'codex', 'pid', ?)`,
    );
    insert.run("shared-a", process.pid, stale, fixture.repo);
    insert.run("shared-b", process.pid, stale, fixture.repo);
    database.close();

    const result = await runCli(fixture, ["status", "--json"], readOnly);
    expect(result.exitCode).toBe(0);
    const envelope = JSON.parse(result.stdout) as {
      data: { sessions: Array<{ anchor: string; id: string }> };
    };
    expect(
      envelope.data.sessions
        .filter((session) => session.id.startsWith("shared-"))
        .map((session) => session.anchor),
    ).toEqual(["ttl", "ttl"]);

    const stored = new Database(path, { readonly: true });
    try {
      expect(
        stored.query<{ anchor: string; last_seen: string }, []>(
          "SELECT anchor, last_seen FROM sessions WHERE id = 'shared-a'",
        ).get(),
      ).toEqual({ anchor: "pid", last_seen: stale });
      expect(
        stored.query<{ anchor: string; last_seen: string }, []>(
          "SELECT anchor, last_seen FROM sessions WHERE id = 'shared-b'",
        ).get(),
      ).toEqual({ anchor: "pid", last_seen: stale });
    } finally {
      stored.close();
    }
  });
});

test("214 read-only mode rejects a durable command before it writes", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["version"]);
    const result = await runCli(fixture, ["work", "add", "must not exist"], readOnly);

    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain('"code":"READ_ONLY"');
    expect(result.stderr).toContain("remove MAESTRO_READ_ONLY");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    try {
      expect(
        database.query<{ count: number }, []>("SELECT count(*) AS count FROM work").get()?.count,
      ).toBe(0);
    } finally {
      database.close();
    }
  });
});

test("215 read-only mode rejects future plugin commands by default", async () => {
  await withFixture(async (fixture) => {
    const marker = join(fixture.repo, "external-plugin-loaded");
    await writePlugin(
      fixture,
      "repo",
      "future-observer",
      `import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(marker)}, "loaded");
export default {
  name: "future-observer",
  apply(context) {
    context.effect(() => context.cli.register("future inspect", () => "future"));
  },
};\n`,
    );
    await writeConfig(fixture, [{ name: "future-observer" }]);

    const result = await runCli(fixture, ["future", "inspect"], readOnly);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain('"code":"READ_ONLY"');
    expect(await Bun.file(marker).exists()).toBe(false);
  });
});

test("216 read-only startup does not repair persisted indexes", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["version"]);
    const path = join(fixture.repo, ".maestro", "maestro.db");
    const database = new Database(path);
    database.run("DELETE FROM search_index_state");
    database.run("INSERT INTO search_index_state(version) VALUES (0)");
    database.run(
      `INSERT INTO bundles
       (id, state, directory, spec, notes, verify, created_at, updated_at)
       VALUES ('observer-bundle', 'archived', '/fixture', 'needle', '', '',
               '2020-01-01T00:00:00.000Z', '2020-01-01T00:00:00.000Z')`,
    );
    database.run(
      `INSERT INTO legacy_cards
       (id, card_type, parent, status, title, record_file, card_yaml, created_at, updated_at, imported_at)
       VALUES ('legacy-observer', 'task', NULL, 'open', 'legacy needle', 'record.yml',
               'id: legacy-observer', '2020-01-01T00:00:00.000Z',
               '2020-01-01T00:00:00.000Z', '2020-01-01T00:00:00.000Z')`,
    );
    database.run("DELETE FROM search_index WHERE surface IN ('bundle', '[legacy]')");
    database.close();

    const result = await runCli(fixture, ["version"], readOnly);
    expect(result.exitCode).toBe(0);

    const stored = new Database(path, { readonly: true });
    try {
      expect(
        stored.query<{ version: number }, []>(
          "SELECT version FROM search_index_state LIMIT 1",
        ).get()?.version,
      ).toBe(0);
      expect(
        stored.query<{ count: number }, []>(
          "SELECT count(*) AS count FROM search_index WHERE surface IN ('bundle', '[legacy]')",
        ).get()?.count,
      ).toBe(0);
    } finally {
      stored.close();
    }
  });
});

test("217 read-only mode blocks file-first commands before filesystem effects", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["plugin", "new", "blocked-plugin"], readOnly);

    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain('"code":"READ_ONLY"');
    expect(await Bun.file(join(fixture.repo, ".maestro", "plugins", "blocked-plugin.ts")).exists())
      .toBe(false);
  });
});

test("218 read-only mode guards lifecycle commands before their special dispatch", async () => {
  await withFixture(async (fixture) => {
    for (const command of ["uninstall", "update"]) {
      const result = await runCli(fixture, [command], readOnly);
      expect(result.exitCode, command).toBe(1);
      expect(result.stderr, command).toContain('"code":"READ_ONLY"');
    }
  });
});

test("304 read-only search fails closed on a stale index and serves a fresh index", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["work", "add", "observer search needle"]);
    const path = join(fixture.repo, ".maestro", "maestro.db");
    const database = new Database(path);
    database.run("DELETE FROM search_index WHERE surface = 'work'");
    database.run("DELETE FROM search_index_state");
    database.run("INSERT INTO search_index_state(version) VALUES (0)");
    database.close();
    const before = Buffer.from(await Bun.file(path).arrayBuffer()).toString("base64");

    const stale = await runCli(fixture, ["search", "needle"], readOnly);
    expect(stale.exitCode).not.toBe(0);
    expect(stale.stderr).toContain('"code":"READ_ONLY"');
    expect(Buffer.from(await Bun.file(path).arrayBuffer()).toString("base64")).toBe(before);

    const stored = new Database(path, {
      readonly: true,
    });
    try {
      expect(
        stored.query<{ version: number }, []>(
          "SELECT version FROM search_index_state LIMIT 1",
        ).get()?.version,
      ).toBe(0);
      expect(
        stored.query<{ count: number }, []>(
          "SELECT count(*) AS count FROM search_index WHERE surface = 'work'",
        ).get()?.count,
      ).toBe(0);
    } finally {
      stored.close();
    }

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const fresh = await runCli(fixture, ["search", "needle"], readOnly);
    expect(fresh.exitCode).toBe(0);
    expect(fresh.stdout).toContain("observer search needle");
  });
});

test("286 observer mode follows registered mutability and defaults fail-closed", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["search", "x"], readOnly)).exitCode).toBe(0);
    expect((await runCli(fixture, ["plugin", "list"], readOnly)).exitCode).toBe(0);

    const write = await runCli(fixture, ["work", "add", "x"], readOnly);
    expect(write.exitCode).toBe(1);
    expect(write.stderr).toContain('"code":"READ_ONLY"');
  });

  const previous = process.env.MAESTRO_READ_ONLY;
  process.env.MAESTRO_READ_ONLY = "1";
  try {
    const cli = new Cli(observerMode().cli);
    cli.register("future inspect", () => "future");
    await expect(cli.execute(["future", "inspect"])).rejects.toMatchObject({
      code: "READ_ONLY",
    });
  } finally {
    if (previous === undefined) {
      delete process.env.MAESTRO_READ_ONLY;
    } else {
      process.env.MAESTRO_READ_ONLY = previous;
    }
  }
});
