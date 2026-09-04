import { Database } from "bun:sqlite";
import { expect, setDefaultTimeout, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, initializeGitRepository, runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

setDefaultTimeout(20_000);

const session = { MAESTRO_SESSION_ID: "ux-review", MAESTRO_SESSION_PID: String(process.pid) };

async function hub(fixture: Fixture): Promise<string> {
  const room = join(fixture.home, "maestro");
  await mkdir(room, { recursive: true });
  return room;
}

async function writeClaudeFact(fixture: Fixture, file: string, name: string, description: string): Promise<void> {
  const directory = join(fixture.home, ".claude", "projects", "-Users-x", "memory");
  await mkdir(directory, { recursive: true });
  await writeFile(
    join(directory, file),
    `---\nname: ${name}\ndescription: ${description}\nmetadata:\n  type: feedback\n  modified: 2026-07-01T00:00:00.000Z\n---\n\nBody.\n`,
  );
}

test("601 memory list|show read the Hub from a project cwd; the SessionStart brief counts buffer facts the Hub lacks", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    await writeClaudeFact(fixture, "feedback_one.md", "one", "First fact");
    expect((await runCliAt(fixture, room, ["memory", "ingest"])).exitCode).toBe(0);

    const list = await runCli(fixture, ["memory", "list", "--json"]);
    expect(list.exitCode).toBe(0);
    expect(JSON.parse(list.stdout).data.facts.map((fact: { slug: string }) => fact.slug)).toEqual(["one"]);
    const show = await runCli(fixture, ["memory", "show", "one"]);
    expect(show.exitCode).toBe(0);
    expect(show.stdout).toContain("First fact");
    // The project store's own memory_facts table is empty; the read went to the Hub file.
    const local = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    expect(local.query("SELECT COUNT(*) AS n FROM memory_facts").get()).toEqual({ n: 0 });
    local.close();

    const write = await runCli(fixture, ["memory", "ingest", "--dry-run"]);
    expect(write.exitCode).not.toBe(0);
    expect(write.stderr).toContain("NOT_HUB_STORE");

    const quiet = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session);
    expect(quiet.stdout).not.toContain("buffer facts not yet in the Hub");
    await writeClaudeFact(fixture, "feedback_two.md", "two", "Second fact");
    const pending = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session);
    expect(pending.stdout).toContain("memory: 1 buffer facts not yet in the Hub; run: maestro memory ingest --dry-run");
  });
});

test("602 work show states what done will require while the work is open", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(fixture, ["work", "add", "Gate preview", "--atomic-reason", "one edit", "--json"], session);
    const id = idFrom(added);
    const open = await runCli(fixture, ["work", "show", id]);
    expect(open.stdout).toContain('gate: policy-proof gates work done: requires --evidence "<evidence>" or paired --claim "<claim>" --proof "<proof>"');
    expect(open.stdout).toContain("gate: policy-breakdown gates work start/ready/done");
    expect((await runCli(fixture, ["work", "done", id, "--evidence", "checked"], session)).exitCode).toBe(0);
    const done = await runCli(fixture, ["work", "show", id]);
    expect(done.stdout).not.toContain("gate:");
  });
});

test("603 status carries the age of each session's last hook event", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session);
    const status = await runCli(fixture, ["status"], session);
    expect(status.stdout).toMatch(/^ux-review \[live\] SessionStart \d+s ago pid=/m);
  });
});

test("604 help marks the verbs observer mode admits", async () => {
  await withFixture(async (fixture) => {
    const root = await runCli(fixture, ["help"]);
    expect(root.stdout).toMatch(/^  term {2,}.* \*$/m);
    expect(root.stdout).toMatch(/^  install {2,}.*[^*]$/m);
    expect(root.stdout).toContain("  *  a verb it admits (on a root verb: at least one of its subverbs)");
    const term = await runCli(fixture, ["help", "term"]);
    expect(term.stdout).toMatch(/^  list {2,}.* \*$/m);
    expect(term.stdout).toMatch(/^  add {2,}.*[^*]$/m);
    expect(term.stdout).toContain("* runs under MAESTRO_READ_ONLY=1");
    const memory = await runCli(fixture, ["help", "memory"]);
    expect(memory.stdout).toMatch(/^  render {2,}.*[^*]$/m);
    const unknown = await runCli(fixture, ["policy", "list"], { MAESTRO_READ_ONLY: "1" });
    expect(unknown.stderr).toContain("is not one of them (it may not be a verb at all)");
  });
});

test("605 work done warns when tracked files are modified and the evidence names no commit", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    await writeFile(join(fixture.repo, ".maestro", "config"), `${JSON.stringify({ plugins: [] })}\n\n`);
    const first = idFrom(await runCli(fixture, ["work", "add", "Dirty tree", "--atomic-reason", "one edit", "--json"], session));
    const unlanded = await runCli(fixture, ["work", "done", first, "--evidence", "suite 12/0/0 on 2026-09-05"], session);
    expect(unlanded.exitCode).toBe(0);
    expect(unlanded.stdout).toContain("warning: 1 tracked files are modified and the evidence names no commit");
    const second = idFrom(await runCli(fixture, ["work", "add", "Landed", "--atomic-reason", "one edit", "--json"], session));
    const landed = await runCli(fixture, ["work", "done", second, "--evidence", "landed as 4842209b on main"], session);
    expect(landed.exitCode).toBe(0);
    expect(landed.stdout).not.toContain("warning:");
  });
});

test("606 brief --session prints the SessionStart brief the hook delivers", async () => {
  await withFixture(async (fixture) => {
    const hook = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session);
    const brief = await runCli(fixture, ["brief", "--session"], session);
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toBe(hook.stdout);
    expect(brief.stdout).toContain("method: quickfix and Light load no skill");
  });
});

test("607 SessionStart prunes dead sessions older than 30 days that hold nothing and sit in no dispatch", async () => {
  await withFixture(async (fixture) => {
    const held = idFrom(await runCli(fixture, ["work", "add", "Held by a ghost", "--json"], session));
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    const old = new Date(Date.now() - 40 * 86_400_000).toISOString();
    const insert = database.query(
      "INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope) VALUES (?, ?, ?, ?, ?, ?, ?)",
    );
    insert.run("old-idle", 999_999, "work.done", old, "claude", "pid", fixture.repo);
    insert.run("old-holder", 999_998, "work.start", old, "claude", "pid", fixture.repo);
    insert.run("recent-dead", 999_997, "work.done", new Date().toISOString(), "claude", "pid", fixture.repo);
    database.query("UPDATE work SET state = 'active', held_by = 'old-holder' WHERE id = ?").run(held);
    database.close();

    const start = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session);
    expect(start.exitCode).toBe(0);
    const after = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const ids = after.query<{ id: string }, []>("SELECT id FROM sessions ORDER BY id").all().map((row) => row.id);
    after.close();
    expect(ids).toEqual(["old-holder", "recent-dead", "ux-review"]);
    const trace = await runCli(fixture, ["status", "--all"], session);
    expect(trace.stdout).not.toContain("old-idle");
  });
});

test("609 work done closes an unheld parentless item in one command when it declares the atomic reason", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "One command", "--json"], session));
    const blocked = await runCli(fixture, ["work", "done", id, "--evidence", "checked"], session);
    expect(blocked.exitCode).toBe(1);
    expect(blocked.stderr).toContain("GATE_BLOCKED");
    const done = await runCli(fixture, ["work", "done", id, "--evidence", "checked", "--atomic-reason", "one edit"], session);
    expect(done.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", id]);
    expect(shown.stdout).toContain("atomic reason: one edit");
  });
});

test("608 a stale Hub search index names the cause and the remedy", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    expect((await runCliAt(fixture, room, ["decision", "draft", "Seat leases expire", "--rationale", "hub"], session)).exitCode).toBe(0);
    const database = new Database(join(room, ".maestro", "maestro.db"));
    database.run("DELETE FROM search_index_state");
    database.close();
    const stale = await runCli(fixture, ["search", "Seat", "--json"]);
    expect(stale.exitCode).not.toBe(0);
    expect(stale.stderr).toContain("HUB_UNAVAILABLE");
    expect(stale.stderr).toContain("its search index is behind the runtime; run maestro update");
  });
});
