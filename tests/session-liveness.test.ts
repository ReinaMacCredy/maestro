import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

const OWNER = { MAESTRO_SESSION_ID: "reused-owner", MAESTRO_SESSION_PID: String(process.pid) };
const PEER = { MAESTRO_SESSION_ID: "reused-peer", MAESTRO_SESSION_PID: "1" };

function backdate(store: string, id: string, lastSeen: string): void {
  const database = new Database(store, { readwrite: true });
  try {
    database.query("UPDATE sessions SET last_seen = ? WHERE id = ?").run(lastSeen, id);
  } finally {
    database.close();
  }
}

test("a lease expires when its holder's pid now belongs to a process started after the session was last seen", async () => {
  await withFixture(async (fixture) => {
    const store = join(fixture.repo, ".maestro", "maestro.db");
    expect((await runCli(fixture, ["work", "add", "held across a pid reuse", "--kind", "research"], OWNER)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "start", "w1"], OWNER)).exitCode).toBe(0);

    const held = await runCli(fixture, ["work", "start", "w1"], PEER);
    expect(held.stdout + held.stderr).toContain("LEASE_HELD");

    backdate(store, "reused-owner", new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString());

    const taken = await runCli(fixture, ["work", "start", "w1"], PEER);
    expect(taken.stdout + taken.stderr).toContain("w1 started by reused-peer");
    expect(taken.exitCode).toBe(0);
  });
});
