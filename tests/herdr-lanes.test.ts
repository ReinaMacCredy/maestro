import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

async function sourceFiles(directory: string): Promise<string[]> {
  const files: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...await sourceFiles(path));
    } else if (entry.isFile() && path.endsWith(".ts")) {
      files.push(path);
    }
  }
  return files;
}

test("249 liveness remains PID and TTL based and no binary code path names Herdr", async () => {
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

    const status = await runCli(fixture, ["status", "--json"], {
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

  const root = join(import.meta.dir, "..");
  const files = [
    ...await sourceFiles(join(root, "src")),
    ...await sourceFiles(join(root, "bin")),
  ];
  for (const file of files) {
    if (relative(root, file) === "src/plugins/room.ts") continue;
    expect((await readFile(file, "utf8")).toLowerCase()).not.toContain("herdr");
  }
});
