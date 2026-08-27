import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { dispatchLaneVocabulary } from "../src/plugins/dispatch.ts";
import { idFrom, prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

test("287 maestro-work skill lane line matches the dispatch vocabulary", async () => {
  const skill = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "skills", "maestro-work", "SKILL.md"),
    "utf8",
  );
  const laneLine = skill.split("\n").find((line) => line.startsWith("Lane: "));
  expect(laneLine).toBeDefined();
  expect(laneLine?.slice("Lane: ".length).split("|").map((name) => name.trim())).toEqual(
    dispatchLaneVocabulary.map(({ name }) => name),
  );
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
    await runCli(fixture, [
      "decision",
      "draft",
      "use copper traces",
      "--rationale",
      "cheaper than silver",
      "--work",
      workId,
    ]);
    const directory = join(fixture.repo, ".maestro", "bundle", "amp-wiring");
    await writeFile(join(directory, "SPEC.md"), "# SPEC\n\namplifier wiring contract\n");
    const live = await runCli(fixture, ["bundle", "show", "amp-wiring"]);
    expect(live.exitCode).toBe(0);
    expect(live.stdout).toContain("amplifier wiring contract");
    expect(live.stdout).toContain(workId);
    expect(live.stdout).toContain("wire the amp");
    expect(live.stdout).toContain("use copper traces");
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

test("128 bundle save ingests a foreign trio dir straight to archived", async () => {
  await withFixture(async (fixture) => {
    const foreign = join(fixture.repo, "old-bundles", "card-sprawl");
    await mkdir(foreign, { recursive: true });
    await writeFile(join(foreign, "SPEC.md"), "# SPEC\n\ndissolve the card state machine\n");
    await writeFile(join(foreign, "NOTES.md"), "# NOTES\n\ndone long ago\n");
    await writeFile(join(foreign, "VERIFY.md"), "# VERIFY\n\nall rows pass\n");
    const saved = await runCli(fixture, ["bundle", "save", foreign]);
    expect(saved.exitCode).toBe(0);
    expect(saved.stdout).toContain("card-sprawl [archived]");
    const found = await runCli(fixture, ["search", "dissolve"]);
    expect(found.stdout).toContain("card-sprawl");
    expect(found.stdout).toContain("(bundle, archived)");
  });
});

test("129 install materializes the 4 maestro skills with a version stamp and refs", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const skillsRoot = join(fixture.home, "maestro", "skills");
    for (const name of ["maestro-bundle", "maestro-design", "maestro-work", "maestro-verify"]) {
      const skill = await Bun.file(join(skillsRoot, name, "SKILL.md")).text();
      expect(skill).toMatch(/<!-- maestro-skill-version: [0-9a-f]{40} -->/);
    }
    for (const reference of [
      join("maestro-design", "references", "unattended.md"),
      join("maestro-work", "references", "worktree.md"),
      join("maestro-work", "references", "conflict-handoff.md"),
      join("maestro-work", "references", "tdd-antipatterns.md"),
      join("maestro-verify", "references", "audit.md"),
      join("maestro-verify", "references", "learning.md"),
    ]) {
      expect(await Bun.file(join(skillsRoot, reference)).exists()).toBe(true);
    }
    const antipatterns = await Bun.file(
      join(skillsRoot, "maestro-work", "references", "tdd-antipatterns.md"),
    ).text();
    const entries = antipatterns.split("\n").filter((line) => /^\| .+ \| .+ \| .+ \|$/.test(line));
    expect(entries.length).toBeGreaterThan(4);
    expect(entries.length).toBeLessThanOrEqual(21);
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

test("132 policy-lifecycle ships dark: disabled by default with an honest requires string", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await runCli(fixture, ["install"], { PATH: path });
    const config = JSON.parse(
      await readFile(join(fixture.repo, ".maestro", "config"), "utf8"),
    ) as { plugins: Array<{ disabled: boolean; name: string }> };
    expect(config.plugins).toContainEqual({ name: "policy-lifecycle", disabled: true });
    const listed = await runCli(fixture, ["plugin", "list"]);
    expect(listed.stdout).toContain("policy-lifecycle\tbuilt-in\tdisabled");
    expect(listed.stdout).toContain("reserved");
  });
});
