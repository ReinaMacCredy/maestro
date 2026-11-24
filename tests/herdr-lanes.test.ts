import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

test("249 persisted session liveness remains PID and TTL based", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["work", "list"])).exitCode).toBe(0);
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
    const hold = database.query(
      `INSERT INTO work (id, title, kind, state, held_by, created_at, updated_at)
       VALUES (?, ?, 'task', 'active', ?, ?, ?)`,
    );
    for (const [id, holder] of [["w1", "fresh-ttl"], ["w2", "stale-ttl"], ["w3", "dead-pid"]] as const) {
      hold.run(id, `held by ${holder}`, holder, new Date().toISOString(), new Date().toISOString());
    }
    database.close();

    const listed = await runCli(fixture, ["work", "list", "--json"], {
      MAESTRO_READ_ONLY: "1",
      MAESTRO_SESSION_ID: "phase2-observer",
      MAESTRO_SESSION_PID: String(process.pid),
    });
    expect(listed.exitCode).toBe(0);
    const holders = (JSON.parse(listed.stdout) as {
      data: { works: Array<{ holder: { id: string; live: boolean } | null }> };
    }).data.works.flatMap((work) => (work.holder ? [work.holder] : []));
    expect(holders.find((holder) => holder.id === "fresh-ttl")?.live).toBe(true);
    expect(holders.find((holder) => holder.id === "stale-ttl")?.live).toBe(false);
    expect(holders.find((holder) => holder.id === "dead-pid")?.live).toBe(false);
  });
});

test("330 [lint] session liveness keeps a closed runtime dependency boundary", async () => {
  // AST dependency lint: proves sessions.ts has no runtime imports, not that dynamic driver invocation is absent.
  const sessions = await readFile(join(import.meta.dir, "..", "src", "kernel", "sessions.ts"), "utf8");
  const imports = new Bun.Transpiler({ loader: "ts" }).scanImports(sessions);
  expect(imports.map((dependency) => dependency.path)).toEqual([]);
});
