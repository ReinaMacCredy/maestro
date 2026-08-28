import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { rmSync } from "node:fs";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type CliResult, type Fixture } from "./helpers.ts";

// `maestro mcp` is pure but blocks on stdin as a server, so it cannot be swept
// by invocation. Every other verb the CLI advertises is exercised.
const unsweepable = new Set(["mcp", "help"]);

function storePath(fixture: Fixture): string {
  return join(fixture.repo, ".maestro", "maestro.db");
}

async function verbs(fixture: Fixture): Promise<string[][]> {
  const roots = (await runCli(fixture, ["help"])).stdout
    .split("\n")
    .map((line) => line.match(/^ {2}([a-z-]+) {2}/)?.[1])
    .filter((verb): verb is string => Boolean(verb) && !unsweepable.has(verb as string));
  const lines: string[][] = [];
  for (const root of roots) {
    const subverbs = (await runCli(fixture, ["help", root])).stdout
      .split("\n")
      .map((line) => line.match(/^ {2}([a-z][a-z-]*) {2,}[A-Z]/)?.[1])
      .filter((verb): verb is string => Boolean(verb));
    if (subverbs.length === 0) lines.push([root]);
    else for (const subverb of subverbs) lines.push([root, subverb]);
  }
  return lines;
}

function classify(result: CliResult): string {
  if (result.exitCode === 0) return "ok";
  try {
    return (JSON.parse(result.stderr) as { error: { code: string } }).error.code;
  } catch {
    return `unstructured:${result.stderr.trim().split("\n")[0] ?? ""}`;
  }
}

// Session liveness bookkeeping is exempt by d28 and d29: refresh() heartbeats
// a TTL row on every command and a writable caller persists the shared-pid
// downgrade, both deliberately. Everything else is domain content that a
// declared-pure verb must leave alone.
const livenessTables = new Set(["sessions"]);

// The logical content of the store, independent of SQLite's on-disk layout.
// Byte equality cannot be the property: a WAL database is unreadable without
// its -shm, so a reader is sometimes obliged to create one.
function content(path: string): string {
  const database = new Database(path, { readonly: true });
  try {
    const tables = database
      .query<{ name: string }, []>(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
      )
      .all()
      .map((row) => row.name)
      .filter((name) => !livenessTables.has(name));
    return tables
      .map((table) => `${table}:${JSON.stringify(database.query(`SELECT * FROM "${table}"`).all())}`)
      .join("\n");
  } finally {
    database.close();
  }
}

async function populate(fixture: Fixture): Promise<void> {
  const work = idFrom(await runCli(fixture, ["work", "add", "swept", "--atomic-reason", "fixture"]));
  await runCli(fixture, ["hook", "record", "--event", "SessionStart"]);
  await runCli(fixture, ["decision", "draft", "a settled choice", "--rationale", "because"]);
  await runCli(fixture, [
    "dispatch", "open", work,
    "--objective", "return one view",
    "--owned-scope", "read-only fixture",
    "--excluded-scope", "source",
    "--mutation", "no-write",
    "--stop-condition", "one handback",
    "--lane", "scout",
    "--evidence-required", "source: note",
    "--pane", "w1:p1",
  ]);
  // Two sessions on one pid: the shared-pid downgrade is a write that a
  // declared-pure read would otherwise perform.
  await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
    MAESTRO_SESSION_ID: "co-tenant",
    MAESTRO_SESSION_PID: String(process.pid),
  });
}

// A cleanly closed WAL database has no sidecars: SQLite removes them on the
// last connection's close. Checkpoint first so no committed frame is lost.
function closeCleanly(path: string): void {
  const database = new Database(path);
  try {
    database.exec("PRAGMA wal_checkpoint(TRUNCATE)");
  } finally {
    database.close();
  }
  rmSync(`${path}-wal`, { force: true });
  rmSync(`${path}-shm`, { force: true });
}

test("472 a declared-pure verb answers the same whether or not the WAL sidecars are on disk (w507, d706)", async () => {
  await withFixture(async (fixture) => {
    await populate(fixture);
    const readOnly = { MAESTRO_READ_ONLY: "1" };

    const pure = new Map<string, string[]>();
    for (const argv of await verbs(fixture)) {
      const verdict = classify(await runCli(fixture, argv, readOnly));
      if (verdict !== "READ_ONLY") pure.set(argv.join(" "), argv);
    }
    // Guard the guard: if the discovery walk silently found nothing, the sweep
    // below would pass vacuously.
    expect(pure.size).toBeGreaterThan(8);
    for (const name of ["work list", "status", "attention", "decision list", "search"]) {
      expect([...pure.keys()]).toContain(name);
    }

    const before = new Map<string, string>();
    for (const [name, argv] of pure) before.set(name, classify(await runCli(fixture, argv, readOnly)));

    closeCleanly(storePath(fixture));

    const after = new Map<string, string>();
    for (const [name, argv] of pure) after.set(name, classify(await runCli(fixture, argv, readOnly)));

    expect(Object.fromEntries(after)).toEqual(Object.fromEntries(before));
    expect([...after.values()]).not.toContain("READ_ONLY");
  });
}, 120_000);

test("473 a declared-pure verb leaves store content unchanged on a writable connection (w508, d706)", async () => {
  await withFixture(async (fixture) => {
    await populate(fixture);

    const pure: string[][] = [];
    for (const argv of await verbs(fixture)) {
      if (classify(await runCli(fixture, argv, { MAESTRO_READ_ONLY: "1" })) !== "READ_ONLY") {
        pure.push(argv);
      }
    }

    const sessionIds = (): string[] => {
      const database = new Database(storePath(fixture), { readonly: true });
      try {
        return database.query<{ id: string }, []>("SELECT id FROM sessions ORDER BY id").all()
          .map((row) => row.id);
      } finally {
        database.close();
      }
    };

    const before = content(storePath(fixture));
    const beforeSessions = sessionIds();
    for (const argv of pure) await runCli(fixture, argv);
    const after = content(storePath(fixture));

    expect(after).toBe(before);
    // The liveness exemption covers heartbeats on existing rows, not new ones.
    expect(sessionIds()).toEqual(beforeSessions);
  });
}, 120_000);
