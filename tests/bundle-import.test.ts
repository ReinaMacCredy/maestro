import { Database } from "bun:sqlite";
import { expect, setDefaultTimeout, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture, type Fixture } from "./helpers.ts";

setDefaultTimeout(15_000);

async function trio(directory: string, id: string, extra: Record<string, string> = {}): Promise<void> {
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, "SPEC.md"), `# SPEC — ${id}\n\n## Problem\n${id} problem text\n`);
  await writeFile(join(directory, "NOTES.md"), `# NOTES — ${id}\n`);
  await writeFile(join(directory, "VERIFY.md"), `# VERIFY — ${id}\n`);
  for (const [name, text] of Object.entries(extra)) await writeFile(join(directory, name), text);
}

// N=2 active, M=1 paused, K=2 archived, one ADR, a MEMORY.md in both shapes,
// a CONTEXT.md glossary, and the stray files real trees carry.
async function waymarkFixture(fixture: Fixture): Promise<string> {
  const root = join(fixture.repo, ".waymark");
  await trio(join(root, "active", "plugin-engine-slp"), "plugin-engine-slp", { "PROMPT-codex.md": "prompt\n" });
  await trio(join(root, "active", "upstream-sync"), "upstream-sync");
  await trio(join(root, "paused", "orchestrator-mode"), "orchestrator-mode", { "manifest.json": "{}\n" });
  await trio(join(root, "archive", "message-fork"), "message-fork", { "LIVE-TEST.md": "live\n" });
  await trio(join(root, "archive", "diff-system-port"), "diff-system-port");
  await mkdir(join(root, "adr"), { recursive: true });
  await writeFile(
    join(root, "adr", "0001-spawned-seat-runtime-policy.md"),
    "# ADR 0001: Spawned seats use target role permissions\n\nStatus: Accepted\n\nDate: 2026-08-20\n\n## Decision\n\nDerive runtimeMode from the target role policy.\n",
  );
  await writeFile(
    join(root, "MEMORY.md"),
    [
      "# Spec workflow memory",
      "",
      "- [message-fork](archive/message-fork/): bootstrap imported history only on the fork's first turn.",
      "- [ADR 0001](adr/0001-spawned-seat-runtime-policy.md) — a spawned seat derives runtime mode from the target role policy.",
      "- Verify sticky rows on both transcripts at a scrolled viewport.",
      "",
      "## Recent Outcomes",
      "",
      "- Work ID: `irina-foundation`",
      "",
      "<!-- memory-unit:work:irina-foundation -->",
      "",
      "##### Decisions and Rationale",
      "",
      "- `LOCKED D-001`: Irina is the owner's delegated operational embodiment.",
      "",
    ].join("\n"),
  );
  await writeFile(
    join(root, "CONTEXT.md"),
    "# Vocabulary\n\n## Language\n\n**Kernel**:\nThe mechanism-only core.\n_Avoid_: engine\n\n**Work item**:\nOne tracked unit of work.\n",
  );
  await writeFile(join(root, "README.md"), "# Spec Workflow Home\n");
  await writeFile(join(root, "REPO_EVOLUTION.md"), "# Repository Evolution\n");
  await writeFile(join(root, ".gitignore"), "active/\n");
  await mkdir(join(root, "tmp", "rustup-shim"), { recursive: true });
  await writeFile(join(root, "tmp", "rustup-shim", "rustup"), "#!/bin/sh\n");
  return root;
}

function rows(fixture: Fixture, sql: string): number {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
  try {
    return (database.query<{ n: number }, []>(sql).get()?.n) ?? 0;
  } finally {
    database.close();
  }
}

test("574 R2 bundle import: the report names every source item with counts equal to the tree, the store equals the report, dry-run writes nothing, and a second import is idempotent", async () => {
  await withFixture(async (fixture) => {
    const source = await waymarkFixture(fixture);

    const dry = await runCli(fixture, ["bundle", "import", ".waymark", "--dry-run", "--json"]);
    expect(dry.exitCode).toBe(0);
    const dryReport = JSON.parse(dry.stdout).data;
    expect(dryReport.dryRun).toBe(true);
    expect(dryReport.counts.active.imported).toBe(2);
    expect(dryReport.counts.paused.imported).toBe(1);
    expect(dryReport.counts.archive.imported).toBe(2);
    expect(dryReport.counts.memory).toMatchObject({ imported: 4, skipped: 1 });
    expect(dryReport.counts.adr.imported).toBe(1);
    expect(dryReport.counts.term.imported).toBe(2);
    expect(dryReport.counts.extra).toMatchObject({ copied: 3 });
    expect(dryReport.counts.file.skipped).toBe(3);
    expect(dryReport.counts.dir.unknown).toBe(1);
    const names = dryReport.entries.map((entry: { name: string }) => entry.name);
    for (const expected of [
      "active/plugin-engine-slp", "active/plugin-engine-slp/PROMPT-codex.md", "active/upstream-sync",
      "paused/orchestrator-mode", "paused/orchestrator-mode/manifest.json",
      "archive/message-fork", "archive/message-fork/LIVE-TEST.md", "archive/diff-system-port",
      "MEMORY.md:3", "MEMORY.md:4", "MEMORY.md:5", "MEMORY.md:9", "MEMORY.md:15",
      "adr/0001-spawned-seat-runtime-policy.md", "CONTEXT.md Kernel", "CONTEXT.md Work-item",
      "README.md", "REPO_EVOLUTION.md", ".gitignore", "tmp",
    ]) expect(names).toContain(expected);
    expect(rows(fixture, "SELECT count(*) AS n FROM bundles")).toBe(0);
    expect(rows(fixture, "SELECT count(*) AS n FROM decisions")).toBe(0);
    expect(rows(fixture, "SELECT count(*) AS n FROM terms")).toBe(0);
    expect(existsSync(join(fixture.repo, ".maestro", "bundle"))).toBe(false);
    expect(existsSync(source)).toBe(true);

    const real = await runCli(fixture, ["bundle", "import", ".waymark", "--json"]);
    expect(real.exitCode).toBe(0);
    const report = JSON.parse(real.stdout).data;
    expect(report.counts).toEqual(dryReport.counts);
    expect(rows(fixture, "SELECT count(*) AS n FROM bundles WHERE state = 'active' AND paused_at IS NULL")).toBe(2);
    expect(rows(fixture, "SELECT count(*) AS n FROM bundles WHERE state = 'active' AND paused_at IS NOT NULL")).toBe(1);
    expect(rows(fixture, "SELECT count(*) AS n FROM bundles WHERE state = 'archived' AND spec IS NOT NULL")).toBe(2);
    expect(rows(fixture, "SELECT count(*) AS n FROM decisions WHERE state = 'locked'")).toBe(4);
    expect(rows(fixture, "SELECT count(*) AS n FROM decisions WHERE state = 'draft'")).toBe(1);
    expect(rows(fixture, "SELECT count(*) AS n FROM terms")).toBe(2);
    expect(existsSync(join(fixture.repo, ".maestro", "bundle", "plugin-engine-slp", "PROMPT-codex.md"))).toBe(true);
    expect(existsSync(join(fixture.repo, ".maestro", "bundle", "orchestrator-mode", "manifest.json"))).toBe(true);
    expect(existsSync(join(fixture.repo, ".maestro", "bundle", "message-fork", "LIVE-TEST.md"))).toBe(true);
    expect(existsSync(source)).toBe(true);

    const list = await runCli(fixture, ["bundle", "list"]);
    expect(list.stdout).toContain("orchestrator-mode [paused]");
    expect(list.stdout).toContain("message-fork [archived]");
    const shown = await runCli(fixture, ["bundle", "show", "message-fork"]);
    expect(shown.stdout).toContain("message-fork problem text");
    const adr = await runCli(fixture, ["decision", "show", "d5"]);
    expect(adr.stdout).toContain("Spawned seats use target role permissions");
    expect(adr.stdout).toContain("Derive runtimeMode from the target role policy");
    const found = await runCli(fixture, ["search", "Irina", "--local"]);
    expect(found.stdout).toContain("(decision, locked)");

    const again = await runCli(fixture, ["bundle", "import", ".waymark", "--json"]);
    const againReport = JSON.parse(again.stdout).data;
    expect(againReport.counts.active).toMatchObject({ exists: 2, imported: 0 });
    expect(againReport.counts.paused).toMatchObject({ exists: 1, imported: 0 });
    expect(againReport.counts.archive).toMatchObject({ exists: 2, imported: 0 });
    expect(againReport.counts.memory).toMatchObject({ exists: 4, imported: 0 });
    expect(againReport.counts.adr).toMatchObject({ exists: 1, imported: 0 });
    expect(againReport.counts.term).toMatchObject({ exists: 2, imported: 0 });
    expect(rows(fixture, "SELECT count(*) AS n FROM decisions")).toBe(5);
    expect(rows(fixture, "SELECT count(*) AS n FROM bundles")).toBe(5);
  });
});

test("575 bundle pause and resume stamp an active bundle without leaving the state machine", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["bundle", "open", "slice"])).exitCode).toBe(0);
    const paused = await runCli(fixture, ["bundle", "pause", "slice", "--reason", "waiting on owner"]);
    expect(paused.exitCode).toBe(0);
    expect(paused.stdout).toContain("slice [paused]");
    const twice = await runCli(fixture, ["bundle", "pause", "slice"]);
    expect(twice.exitCode).not.toBe(0);
    expect(twice.stderr).toContain("INVALID_STATE");
    const resumed = await runCli(fixture, ["bundle", "resume", "slice"]);
    expect(resumed.stdout).toContain("slice [active]");
    const missing = await runCli(fixture, ["bundle", "resume", "slice"]);
    expect(missing.stderr).toContain("INVALID_STATE");
    const archivedRefused = await runCli(fixture, ["bundle", "pause", "nope"]);
    expect(archivedRefused.stderr).toContain("NOT_FOUND");
  });
});
