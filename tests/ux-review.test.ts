import { Database } from "bun:sqlite";
import { expect, setDefaultTimeout, test } from "bun:test";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, initializeGitRepository, runCli, runCliAt, withFixture, writeUntrustedPlugin, type Fixture } from "./helpers.ts";

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
    expect(write.exitCode).toBe(0);
    expect(write.stdout).toContain("dry-run: promoted 0, updated 0, skipped 1, refused 0");

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
    expect(brief.stdout).toContain("enabled policies: policy-breakdown, policy-dispatch, policy-lifecycle, policy-proof\n  see or change: maestro plugin list|enable|disable <name>\n");
    const prompt = await runCli(fixture, ["hook", "record", "--event", "UserPromptSubmit"], session);
    expect(prompt.stdout).not.toContain("see or change");
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
    const held = idFrom(await runCli(fixture, ["work", "add", "Held", "--atomic-reason", "first", "--json"], session));
    expect((await runCli(fixture, ["work", "start", held], session)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", held, "--evidence", "checked", "--atomic-reason", "revised"], session)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "show", held])).stdout).toContain("atomic reason: revised");
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

test("635 memory ingest|retract|render write the Hub from a project cwd through the Hub's own CLI", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    expect((await runCliAt(fixture, room, ["memory", "list"])).exitCode).toBe(0);
    await writeClaudeFact(fixture, "feedback_three.md", "three", "Third fact");
    const ingested = await runCli(fixture, ["memory", "ingest", "--json"], session);
    expect(ingested.exitCode).toBe(0);
    expect(JSON.parse(ingested.stdout).data.counts).toEqual({ promoted: 1, updated: 0, skipped: 0, refused: 0 });
    const plain = await runCli(fixture, ["memory", "ingest"], session);
    expect(plain.stdout).toContain("promoted 0, updated 0, skipped 1, refused 0 (1 facts from");
    const local = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    expect(local.query("SELECT COUNT(*) AS n FROM memory_facts").get()).toEqual({ n: 0 });
    local.close();
    expect((await runCliAt(fixture, room, ["memory", "show", "three"])).stdout).toContain("Third fact");

    const rendered = await runCli(fixture, ["memory", "render"], session);
    expect(rendered.exitCode).toBe(0);
    expect(rendered.stdout).toMatch(/^rendered .*\/maestro\/MEMORY\.md \(1 facts, [0-9a-f]{12}; was missing\)\n$/);
    const check = await runCli(fixture, ["memory", "render", "--check"]);
    expect(check.exitCode).toBe(0);
    expect(check.stdout).toMatch(/ current \(1 facts, [0-9a-f]{12}\)\n$/);

    const retracted = await runCli(fixture, ["memory", "retract", "three", "--reason", "no longer true"], session);
    expect(retracted.exitCode).toBe(0);
    expect(retracted.stdout).toMatch(/^m\d+ three retracted: no longer true\nindex re-rendered: .*\/maestro\/MEMORY\.md \(0 facts\)\n$/);
    const missing = await runCli(fixture, ["memory", "retract", "nope", "--reason", "x"], session);
    expect(missing.exitCode).toBe(1);
    expect(missing.stderr).toContain("NOT_FOUND");
  });
});

test("640 a read verb whose stdout pipe closes early exits 0 with no EPIPE stack (UX F7)", async () => {
  await withFixture(async (fixture) => {
    const long = "x".repeat(3000);
    for (let index = 0; index < 30; index += 1) {
      expect((await runCli(fixture, ["decision", "draft", `${index} ${long}`])).exitCode).toBe(0);
    }
    const full = await runCli(fixture, ["decision", "list"]);
    expect(full.stdout.length).toBeGreaterThan(65_536);

    const cli = join(import.meta.dir, "..", "bin", "maestro.ts");
    const piped = Bun.spawn(
      ["bash", "-c", `set -o pipefail; "${process.execPath}" "${cli}" decision list | head -1`],
      {
        cwd: fixture.repo,
        env: { ...process.env, HOME: fixture.home, ...session },
        stdout: "pipe",
        stderr: "pipe",
      },
    );
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(piped.stdout).text(),
      new Response(piped.stderr).text(),
      piped.exited,
    ]);
    expect(stdout.split("\n")[0]).toMatch(/^d1 \[draft\] 0 x/);
    expect(stderr).toBe("");
    expect(exitCode).toBe(0);
  });
});

test("641 work show prints the last five notes behind a count line; --notes <n|all> widens the window (UX F8, d821)", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "noisy"]));
    for (let index = 1; index <= 8; index += 1) {
      expect((await runCli(fixture, ["work", "note", id, `note ${index}`], session)).exitCode).toBe(0);
    }
    const shown = await runCli(fixture, ["work", "show", id]);
    expect(shown.exitCode).toBe(0);
    const lines = shown.stdout.split("\n");
    expect(lines).toContain("notes: 8, showing the last 5; maestro work show " + id + " --notes all");
    expect(lines.filter((line) => line.startsWith("note: "))).toEqual(
      ["note 4", "note 5", "note 6", "note 7", "note 8"].map((text) => `note: ${text}`),
    );
    const json = JSON.parse((await runCli(fixture, ["work", "show", id, "--json"])).stdout).data;
    expect(json.noteCount).toBe(8);
    expect(json.notes.map((note: { text: string }) => note.text)).toEqual(["note 4", "note 5", "note 6", "note 7", "note 8"]);

    const two = await runCli(fixture, ["work", "show", id, "--notes", "2"]);
    expect(two.stdout.split("\n").filter((line) => line.startsWith("note: "))).toEqual(["note: note 7", "note: note 8"]);
    expect(two.stdout).toContain("notes: 8, showing the last 2;");
    const all = await runCli(fixture, ["work", "show", id, "--notes", "all"]);
    expect(all.stdout.split("\n").filter((line) => line.startsWith("note: "))).toHaveLength(8);
    expect(all.stdout).not.toContain("notes: 8,");
    const few = idFrom(await runCli(fixture, ["work", "add", "quiet"]));
    expect((await runCli(fixture, ["work", "note", few, "only"], session)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "show", few])).stdout).not.toContain("notes: ");
    const bad = await runCli(fixture, ["work", "show", id, "--notes", "many"]);
    expect(bad.exitCode).toBe(1);
    expect(bad.stderr).toContain("INVALID_VALUE");
  });
});

test("642 work note --file <path> records the file body, so a long note never passes through the shell (UX F4)", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "long note"]));
    const body = "agents dir -> dotfiles/.agents\n\nsecond paragraph with `backticks` and $HOME.\n";
    const path = join(fixture.root, "note.md");
    await writeFile(path, body);
    const noted = await runCli(fixture, ["work", "note", id, "--file", path], session);
    expect(noted.exitCode).toBe(0);
    const shown = JSON.parse((await runCli(fixture, ["work", "show", id, "--json"])).stdout).data;
    expect(shown.notes.map((note: { text: string }) => note.text)).toEqual([body.trimEnd()]);
    const both = await runCli(fixture, ["work", "note", id, "inline", "--file", path], session);
    expect(both.exitCode).toBe(1);
    expect(both.stderr).toContain("INVALID_OPTION");
    const missing = await runCli(fixture, ["work", "note", id, "--file", join(fixture.root, "absent.md")], session);
    expect(missing.exitCode).toBe(1);
    expect(missing.stderr).toContain("NOT_FOUND");
    expect((await runCli(fixture, ["help", "work", "note"])).stdout).toContain("--file");
  });
});

test("643 memory retract and ingest re-render the injected index, or name the render command when they cannot (UX F9)", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    const index = join(room, "MEMORY.md");
    await writeClaudeFact(fixture, "feedback_one.md", "one", "First fact");
    // No render exists yet: ingest cannot refresh it and says what to run.
    const first = await runCliAt(fixture, room, ["memory", "ingest"], session);
    expect(first.exitCode).toBe(0);
    expect(first.stdout).toContain("index missing; run: maestro memory render");
    expect((await runCliAt(fixture, room, ["memory", "render"], session)).exitCode).toBe(0);
    expect(await readFile(index, "utf8")).toContain("- one: First fact");

    await writeClaudeFact(fixture, "feedback_two.md", "two", "Second fact");
    const second = await runCli(fixture, ["memory", "ingest"], session);
    expect(second.exitCode).toBe(0);
    expect(second.stdout).toMatch(/index re-rendered: .*\/maestro\/MEMORY\.md \(2 facts\)/);
    expect(await readFile(index, "utf8")).toContain("- two: Second fact");
    // Nothing promoted: the index is left alone and nothing is claimed about it.
    const idle = await runCliAt(fixture, room, ["memory", "ingest"], session);
    expect(idle.stdout).not.toContain("index");

    const retracted = await runCli(fixture, ["memory", "retract", "two", "--reason", "gone"], session);
    expect(retracted.exitCode).toBe(0);
    expect(retracted.stdout).toMatch(/index re-rendered: .*\/maestro\/MEMORY\.md \(1 facts\)/);
    expect(await readFile(index, "utf8")).not.toContain("Second fact");
    expect((await runCli(fixture, ["memory", "render", "--check"])).exitCode).toBe(0);

    // A hand-edited render is never overwritten silently.
    await appendFile(index, "\n- hand edit\n");
    const guarded = await runCliAt(fixture, room, ["memory", "retract", "one", "--reason", "gone too"], session);
    expect(guarded.exitCode).toBe(0);
    expect(guarded.stdout).toContain("index not re-rendered: edited by hand; run: maestro memory render --force");
    expect(await readFile(index, "utf8")).toContain("- hand edit");
  });
});

test("644 a forwarded memory failure keeps its code and details behind stderr noise, and a Hub room that resolves elsewhere fails fast (UX F code findings)", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    expect((await runCliAt(fixture, room, ["memory", "list"])).exitCode).toBe(0);
    // An untrusted plugin makes every process print a warning line on stderr
    // before any JSON envelope; the child's envelope must still be found.
    await writeUntrustedPlugin(fixture, "global", "noise", "export default { name: 'noise', apply() {} };");
    const missing = await runCli(fixture, ["memory", "retract", "nope", "--reason", "x"], session);
    expect(missing.exitCode).toBe(1);
    expect(missing.stderr).toContain("untrusted plugin");
    const envelope = JSON.parse(missing.stderr.trim().split("\n").at(-1) ?? "{}");
    expect(envelope.error).toMatchObject({ code: "NOT_FOUND", key: "nope" });
    expect(envelope.error.room).toBe(room);

    const reentered = await runCli(fixture, ["memory", "ingest"], { ...session, MAESTRO_MEMORY_FORWARDED: "1" });
    expect(reentered.exitCode).toBe(1);
    expect(reentered.stderr).toContain("HUB_UNAVAILABLE");
    expect(reentered.stderr).toContain("forwarded once already");

    // A git-managed home makes ~/maestro resolve its store to $HOME/.maestro:
    // the child would forward again, unbounded, unless the parent refuses.
    await initializeGitRepository(fixture.home);
    const mismatch = await runCli(fixture, ["memory", "ingest"], session);
    expect(mismatch.exitCode).toBe(1);
    expect(mismatch.stderr).toContain("HUB_UNAVAILABLE");
    expect(mismatch.stderr).toContain("resolves its store to");
  });
});
