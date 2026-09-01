import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

test("249 persisted session liveness remains PID and TTL based", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["status"])).exitCode).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    const insert = database.query(
      `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
       VALUES (?, ?, 'fixture', ?, 'codex', ?, ?)`,
    );
    insert.run("fresh-ttl", 1, new Date().toISOString(), "ttl", fixture.repo);
    insert.run(
      "stale-ttl",
      1,
      new Date(Date.now() - 61 * 60 * 1000).toISOString(),
      "ttl",
      fixture.repo,
    );
    insert.run("dead-pid", 2_147_483_647, new Date().toISOString(), "pid", fixture.repo);
    database.close();

    const status = await runCli(fixture, ["status", "--all", "--json"], {
      MAESTRO_SESSION_ID: "phase2-observer",
      MAESTRO_SESSION_PID: String(process.pid),
    });
    expect(status.exitCode).toBe(0);
    const sessions = (JSON.parse(status.stdout) as {
      data: { sessions: Array<{ id: string; live: boolean }> };
    }).data.sessions;
    expect(sessions.find((session) => session.id === "fresh-ttl")?.live).toBe(true);
    expect(sessions.find((session) => session.id === "stale-ttl")?.live).toBe(false);
    expect(sessions.find((session) => session.id === "dead-pid")?.live).toBe(false);
  });
});

test("330 [lint] session liveness keeps a closed runtime dependency boundary", async () => {
  // AST dependency lint: proves sessions.ts has no runtime imports, not that dynamic driver invocation is absent.
  const sessions = await readFile(join(import.meta.dir, "..", "src", "kernel", "sessions.ts"), "utf8");
  const imports = new Bun.Transpiler({ loader: "ts" }).scanImports(sessions);
  expect(imports.map((dependency) => dependency.path)).toEqual([]);
});
