import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { idFrom, runCli, setPlugin, withFixture, type Fixture } from "./helpers.ts";

const featureId = "feature-legacy-aurora";

function sha256(bytes: Uint8Array | string): string {
  return createHash("sha256").update(bytes).digest("hex");
}

async function seedLegacyRows(fixture: Fixture, extraCards = 0): Promise<void> {
  expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { strict: true });
  const insertCard = database.query(
    `INSERT INTO legacy_cards
      (id, card_type, parent, status, title, record_file, card_yaml, created_at, updated_at, imported_at)
     VALUES (?, ?, ?, ?, ?, 'card.yaml', ?, ?, ?, ?)`,
  );
  const now = "2026-08-24T00:00:00Z";
  const rootDecisionYaml =
    `schema_version: "4"\nid: dec-root-sqlite\ncard_type: decision\nstatus: locked\ntitle: Keep SQLite archive\ndecision: Preserve the quorum history\n`;
  const featureDecisionYaml =
    `schema_version: "4"\nid: dec-aurora-color\ncard_type: decision\nparent: ${featureId}\nstatus: locked\ntitle: Choose violet telemetry\ndecision: Use ultraviolet signals\n`;
  insertCard.run(
    featureId,
    "feature",
    null,
    "closed",
    "Legacy Aurora",
    `schema_version: "4"\nid: ${featureId}\ncard_type: feature\nstatus: closed\ntitle: Legacy Aurora\ndescription: Historic constellation plan\n`,
    now,
    now,
    now,
  );
  insertCard.run("dec-root-sqlite", "decision", null, "locked", "Keep SQLite archive", rootDecisionYaml, now, now, now);
  insertCard.run("dec-aurora-color", "decision", featureId, "locked", "Choose violet telemetry", featureDecisionYaml, now, now, now);
  for (let index = 0; index < extraCards; index += 1) {
    const id = `feature-context-bomb-${String(index).padStart(2, "0")}`;
    insertCard.run(
      id,
      "feature",
      null,
      "closed",
      `Context bomb ${index}`,
      `schema_version: "4"\nid: ${id}\ncard_type: feature\nstatus: closed\ntitle: Context bomb ${index}\ndescription: ${"contextbomb ".repeat(80)}\n`,
      now,
      now,
      now,
    );
  }

  const insertFile = database.query(
    `INSERT INTO legacy_files (card_id, path, sha256, size, text_content) VALUES (?, ?, ?, ?, ?)`,
  );
  const design = "# Aurora design\nThe quasar archive remains searchable.\n";
  const decisions =
    "- id: dec-aurora-color\n  title: Choose violet telemetry\n  status: locked\n  decision: Use ultraviolet signals\n";
  const binary = new Uint8Array([0xff, 0xfe, 0x00, 0x80]);
  insertFile.run(featureId, "design.md", sha256(design), design.length, design);
  insertFile.run(featureId, "decisions.yaml", sha256(decisions), decisions.length, decisions);
  insertFile.run(featureId, "diagram.bin", sha256(binary), binary.byteLength, null);

  const insertDecision = database.query(
    `INSERT INTO legacy_decisions (id, card_id, title, status, source_path, decision_yaml)
     VALUES (?, ?, ?, 'locked', 'card.yaml', ?)`,
  );
  insertDecision.run("dec-root-sqlite", "dec-root-sqlite", "Keep SQLite archive", rootDecisionYaml);
  insertDecision.run("dec-aurora-color", featureId, "Choose violet telemetry", featureDecisionYaml);
  database.close();
}

test("38 search returns tagged legacy title, file, and decision hits alongside native hits", async () => {
  await withFixture(async (fixture) => {
    await seedLegacyRows(fixture);
    const native = await runCli(fixture, ["work", "add", "Native Aurora follow-up"]);
    expect(native.exitCode).toBe(0);

    const title = await runCli(fixture, ["search", "Aurora"]);
    const file = await runCli(fixture, ["search", "quasar"]);
    const decision = await runCli(fixture, ["search", "quorum"]);
    const perCardDecision = await runCli(fixture, ["search", "ultraviolet"]);

    expect(title.stdout).toContain(`${idFrom(native)} (task, open): Native Aurora follow-up`);
    expect(title.stdout).toContain(`[legacy] ${featureId}`);
    expect(file.stdout).toContain(`[legacy] ${featureId}`);
    expect(decision.stdout).toContain("[legacy] dec-root-sqlite");
    expect(perCardDecision.stdout).toContain(`[legacy] ${featureId}`);
  });
});

test("39 legacy show prints one card, selects one file, and suggests a near miss", async () => {
  await withFixture(async (fixture) => {
    await seedLegacyRows(fixture);

    const shown = await runCli(fixture, ["legacy", "show", featureId]);
    const selected = await runCli(fixture, [
      "legacy",
      "show",
      featureId,
      "--file",
      "design.md",
    ]);
    const missing = await runCli(fixture, ["legacy", "show", "feature-legacy-auror"]);

    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("title: Legacy Aurora");
    expect(shown.stdout).toContain("design.md");
    expect(shown.stdout).toContain("diagram.bin");
    expect(shown.stdout).toContain(`diagram.bin sha256=${sha256(new Uint8Array([0xff, 0xfe, 0x00, 0x80]))} size=4`);
    expect(shown.stdout).not.toContain("�");
    expect(selected.exitCode).toBe(0);
    expect(selected.stdout).toContain("The quasar archive remains searchable.");
    expect(selected.stdout).not.toContain("diagram.bin");
    expect(missing.exitCode).not.toBe(0);
    expect(missing.stderr).toContain("LEGACY_NOT_FOUND");
    expect(missing.stderr).toContain(featureId);
  });
});

test("41 legacy ids remain unknown to work and work list", async () => {
  await withFixture(async (fixture) => {
    await seedLegacyRows(fixture);

    const started = await runCli(fixture, ["work", "start", featureId]);
    const done = await runCli(fixture, ["work", "done", featureId]);
    const json = await runCli(fixture, ["work", "list", "--all", "--json"]);
    expect(json.exitCode).toBe(0);
    const works = (JSON.parse(json.stdout).data as { works: Array<{ id: string }> }).works;
    const listed = await runCli(fixture, ["work", "list", "--all"]);

    expect(started.exitCode).not.toBe(0);
    expect(started.stderr).toContain("NOT_FOUND");
    expect(done.exitCode).not.toBe(0);
    expect(done.stderr).toContain("NOT_FOUND");
    expect(works.map((work) => work.id)).not.toContain(featureId);
    expect(listed.stdout).not.toContain(featureId);
  });
});

test("52 legacy search summaries and result count stay bounded", async () => {
  await withFixture(async (fixture) => {
    await seedLegacyRows(fixture, 60);

    const result = await runCli(fixture, ["search", "contextbomb"]);
    const lines = result.stdout.trim().split("\n");
    const hits = lines.filter((line) => line.startsWith("[legacy]"));

    expect(result.exitCode).toBe(0);
    expect(hits).toHaveLength(5);
    expect(lines.at(-1)).toBe("55 more; raise --limit to see them");
    for (const hit of hits) {
      expect(hit).toMatch(/^\[legacy\] \S+ \(feature, closed\): Context bomb \d+ — /);
      expect(hit.split(" — ").at(-1)?.length).toBeLessThanOrEqual(200);
      expect(hit).not.toContain("schema_version:");
    }
  });
});

test("303 legacy show stays available when the native work plugin is disabled", async () => {
  await withFixture(async (fixture) => {
    await seedLegacyRows(fixture);
    await setPlugin(fixture, "work", true);

    const shown = await runCli(fixture, ["legacy", "show", featureId]);

    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("title: Legacy Aurora");
  });
});
