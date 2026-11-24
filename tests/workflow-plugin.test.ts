import { Database } from "bun:sqlite";
import { expect, setDefaultTimeout, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { laneVocabulary } from "../src/plugins/coordination.ts";
import { idFrom, prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

setDefaultTimeout(15_000);

test("287 [lint] maestro-work skill lane line matches the brief lane vocabulary", async () => {
  // Proves source documentation parity, not independent runtime lane acceptance or rejection.
  const skill = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "skills", "maestro-work", "SKILL.md"),
    "utf8",
  );
  const laneLine = skill.split("\n").find((line) => line.startsWith("Lane: "));
  expect(laneLine).toBeDefined();
  expect(laneLine?.slice("Lane: ".length).split("|").map((name) => name.trim())).toEqual(
    laneVocabulary.map(({ name }) => name),
  );
});

test("288 no legacy .spec-workflow or .waymark tree remains in the repo", async () => {
  // Both lifecycles were imported into the store on 2026-09-04 (maestro-v3, A2/A4);
  // a returning tree would restart the second lifecycle.
  const repo = join(import.meta.dir, "..");
  for (const legacy of [".spec-workflow", ".waymark"]) {
    expect(existsSync(join(repo, legacy))).toBe(false);
  }
});

test("122 decision draft stores a rationale body and show renders it", async () => {
  await withFixture(async (fixture) => {
    const created = await runCli(fixture, [
      "decision",
      "draft",
      "scaffold dir is .maestro/bundle/<id>",
      "--rationale",
      "keeps bundle artifacts inside the maestro scope dir; repos decide gitignore",
    ]);
    expect(created.exitCode).toBe(0);
    const id = idFrom(created);
    expect(created.stdout).toContain(
      "rationale: keeps bundle artifacts inside the maestro scope dir; repos decide gitignore",
    );

    const shown = await runCli(fixture, ["decision", "show", id]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(
      "rationale: keeps bundle artifacts inside the maestro scope dir; repos decide gitignore",
    );

    const help = await runCli(fixture, ["help", "decision"]);
    expect(help.stdout).toContain("--rationale");
  });
});

test("123 decision without rationale stays valid and renders no rationale line", async () => {
  await withFixture(async (fixture) => {
    const created = await runCli(fixture, ["decision", "draft", "plain decision"]);
    expect(created.exitCode).toBe(0);
    const shown = await runCli(fixture, ["decision", "show", idFrom(created)]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).not.toContain("rationale:");
  });
});

test("124 bundle open scaffolds the trio, records an active row, and links work", async () => {
  await withFixture(async (fixture) => {
    const work = await runCli(fixture, ["work", "add", "wire the amp", "--atomic-reason", "test"]);
    const workId = idFrom(work);
    const opened = await runCli(fixture, ["bundle", "open", "amp-wiring", "--work", workId]);
    expect(opened.exitCode).toBe(0);
    for (const name of ["SPEC.md", "NOTES.md", "VERIFY.md"]) {
      const file = Bun.file(join(fixture.repo, ".maestro", "bundle", "amp-wiring", name));
      expect(await file.exists()).toBe(true);
    }
    const listed = await runCli(fixture, ["bundle", "list"]);
    expect(listed.stdout).toContain("amp-wiring [active]");
  });
});

test("124b bundle open links every repeated --work flag, not only the last", async () => {
  await withFixture(async (fixture) => {
    const first = await runCli(fixture, ["work", "add", "wire the amp", "--atomic-reason", "test"]);
    const second = await runCli(fixture, ["work", "add", "tune the amp", "--atomic-reason", "test"]);
    const firstId = idFrom(first);
    const secondId = idFrom(second);
    const opened = await runCli(fixture, [
      "bundle",
      "open",
      "amp-wiring",
      "--work",
      firstId,
      "--work",
      secondId,
    ]);
    expect(opened.exitCode).toBe(0);
    expect(opened.stdout).toContain(`work: ${firstId}, ${secondId}`);
    const shown = await runCli(fixture, ["bundle", "show", "amp-wiring"]);
    expect(shown.stdout).toContain(firstId);
    expect(shown.stdout).toContain(secondId);
  });
});

test("125 bundle close snapshots text into the store and search hits it after the dir dies", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["bundle", "open", "amp-wiring"]);
    const directory = join(fixture.repo, ".maestro", "bundle", "amp-wiring");
    await writeFile(join(directory, "NOTES.md"), "# NOTES\n\nultraviolet copper handoff\n");
    const closed = await runCli(fixture, ["bundle", "close", "amp-wiring"]);
    expect(closed.exitCode).toBe(0);
    expect(closed.stdout).toContain("archived");
    expect(closed.stdout).toContain("hint:");
    await rm(directory, { recursive: true, force: true });
    const found = await runCli(fixture, ["search", "copper"]);
    expect(found.exitCode).toBe(0);
    expect(found.stdout).toContain("amp-wiring");
    expect(found.stdout).toContain("(bundle, archived)");
  });
});

test("126 bundle show composes prose, linked work, and decisions; snapshot survives close", async () => {
  await withFixture(async (fixture) => {
    const work = await runCli(fixture, ["work", "add", "wire the amp", "--atomic-reason", "test"]);
    const workId = idFrom(work);
    await runCli(fixture, ["bundle", "open", "amp-wiring", "--work", workId]);
    const decision = await runCli(fixture, [
      "decision",
      "draft",
      "use copper traces",
      "--rationale",
      "cheaper than silver",
      "--work",
      workId,
    ]);
    const decisionId = idFrom(decision);
    const directory = join(fixture.repo, ".maestro", "bundle", "amp-wiring");
    await writeFile(join(directory, "SPEC.md"), "# SPEC\n\namplifier wiring contract\n");
    const live = await runCli(fixture, ["bundle", "show", "amp-wiring"]);
    expect(live.exitCode).toBe(0);
    expect(live.stdout).toContain("amplifier wiring contract");
    expect(live.stdout).toContain(workId);
    expect(live.stdout).toContain("wire the amp");
    expect(live.stdout).toContain("use copper traces");
    expect(live.stdout.match(new RegExp(decisionId, "g"))).toHaveLength(1);
    const json = await runCli(fixture, ["bundle", "show", "amp-wiring", "--json"]);
    expect(json.exitCode).toBe(0);
    const envelope = JSON.parse(json.stdout) as {
      data: { decisions: Array<{ id: string }> };
    };
    expect(envelope.data.decisions.filter((entry) => entry.id === decisionId)).toHaveLength(1);
    await runCli(fixture, ["bundle", "close", "amp-wiring"]);
    await rm(directory, { recursive: true, force: true });
    const archived = await runCli(fixture, ["bundle", "show", "amp-wiring"]);
    expect(archived.exitCode).toBe(0);
    expect(archived.stdout).toContain("amplifier wiring contract");
  });
});

test("127 anti-sprawl law: the bundles table only accepts active or archived", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["bundle", "open", "amp-wiring"]);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      expect(() =>
        database.run("UPDATE bundles SET state = 'doing' WHERE id = 'amp-wiring'"),
      ).toThrow();
    } finally {
      database.close();
    }
  });
});

test("pstack-port: install retires the managed maestro-diagnose and maestro-explore copies and their links, and keeps a user's own (d863)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const stamped = "---\nname: retired\n---\n<!-- maestro-skill-version: dev -->\nold text\n";
    const managed = join(fixture.home, "maestro", "skills", "maestro-diagnose");
    await mkdir(managed, { recursive: true });
    await writeFile(join(managed, "SKILL.md"), stamped);
    await mkdir(join(fixture.home, ".claude", "skills"), { recursive: true });
    await mkdir(join(fixture.home, ".codex", "skills"), { recursive: true });
    await symlink(managed, join(fixture.home, ".claude", "skills", "maestro-diagnose"));
    await symlink(managed, join(fixture.home, ".codex", "skills", "maestro-diagnose"));
    const own = join(fixture.home, ".claude", "skills", "maestro-explore");
    await mkdir(own, { recursive: true });
    await writeFile(join(own, "SKILL.md"), "---\nname: maestro-explore\n---\nmine\n");

    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    expect(existsSync(managed)).toBe(false);
    for (const harness of [".claude", ".codex"]) {
      expect(existsSync(join(fixture.home, harness, "skills", "maestro-diagnose"))).toBe(false);
    }
    expect(await readFile(join(own, "SKILL.md"), "utf8")).toContain("mine");
  });
});

test("129 [lint] install materializes the 7 maestro skills with a version stamp and refs", async () => {
  await withFixture(async (fixture) => {
    // Proves installed artifact shape, not harness discovery or loading of those skills.
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const skillsRoot = join(fixture.home, "maestro", "skills");
    for (const name of [
      "maestro-bundle",
      "maestro-coach",
      "maestro-council",
      "maestro-design",
      "maestro-questionnaire",
      "maestro-work",
      "maestro-verify",
    ]) {
      const skill = await Bun.file(join(skillsRoot, name, "SKILL.md")).text();
      expect(skill).toMatch(/<!-- maestro-skill-version: [0-9a-f]{40} -->/);
      const source = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "skills", name, "SKILL.md")).text();
      expect(skill.replace(/<!-- maestro-skill-version: [0-9a-f]{40} -->/, "<!-- maestro-skill-version: dev -->")).toBe(source);
    }
    for (const reference of [
      join("maestro-design", "references", "grilling.md"),
      join("maestro-design", "references", "domain-modeling.md"),
      join("maestro-design", "references", "wayfinder.md"),
    ]) {
      expect(await Bun.file(join(skillsRoot, reference)).exists()).toBe(true);
    }
  });
});

test("130 skills re-materialize only on version change", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    const skillPath = join(fixture.home, "maestro", "skills", "maestro-bundle", "SKILL.md");
    const original = await Bun.file(skillPath).text();
    const edited = `${original}\nlocal drift marker\n`;
    await writeFile(skillPath, edited);
    await runCli(fixture, ["install"], { PATH: path });
    expect(await Bun.file(skillPath).text()).toBe(edited);
    const stale = edited.replace(
      /<!-- maestro-skill-version: [0-9a-f]{40} -->/,
      `<!-- maestro-skill-version: ${"0".repeat(40)} -->`,
    );
    await writeFile(skillPath, stale);
    await runCli(fixture, ["install"], { PATH: path });
    const refreshed = await Bun.file(skillPath).text();
    expect(refreshed).not.toContain("local drift marker");
    expect(refreshed).not.toContain("0".repeat(40));
  });
});

test("131 an unstamped legacy skill dir is preserved while the room copy is installed", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const foreign = join(fixture.home, ".agents", "skills", "maestro-design");
    await mkdir(foreign, { recursive: true });
    await writeFile(join(foreign, "SKILL.md"), "# user-owned design skill\n");
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    expect(await Bun.file(join(foreign, "SKILL.md")).text()).toBe("# user-owned design skill\n");
    expect(
      await Bun.file(join(fixture.home, "maestro", "skills", "maestro-design", "SKILL.md")).exists(),
    ).toBe(true);
    expect(installed.stdout).toContain("legacy skill preserved: maestro-design");
  });
});

test("634 the brief routes to canonical tiers after recon, without making a session change a Full trigger", async () => {
  const skill = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "skills", "maestro-bundle", "SKILL.md"),
    "utf8",
  );
  const tierRule = skill.split("## Tier rule")[1]?.split("\n## ")[0] ?? "";
  expect(tierRule).toContain("~/maestro/WORKFLOW.md#tiers");
  expect(tierRule).toContain("maestro bundle open");
  expect(tierRule).not.toMatch(/before .*recon/i);

  await withFixture(async (fixture) => {
    const start = await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
      MAESTRO_SESSION_ID: "tier-brief",
    });
    expect(start.exitCode).toBe(0);
    expect(start.stdout).toContain("~/maestro/WORKFLOW.md");
    expect(start.stdout).toContain("recon before tier selection");
    expect(start.stdout).not.toContain("multi-session, shared scope, high risk, or repeat fix");
    expect(start.stdout).not.toContain("forks: settle before editing");
  });
});
