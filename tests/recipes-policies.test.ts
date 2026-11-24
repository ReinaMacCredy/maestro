import { expect, test } from "bun:test";
import { readFile, readdir } from "node:fs/promises";
import { join, relative } from "node:path";
import { idFrom, prepareInstallFixture, runCli, withFixture, writeConfig } from "./helpers.ts";

const recipeNames = [
  "investigation",
  "bug-fix",
  "feature",
  "refactoring",
  "prototype",
  "perf-issue",
  "hillclimb",
  "forensics",
  "eval",
  "authoring-a-skill",
  "visual-parity",
  "multi-phase-plan",
  "opening-a-pr",
  "babysit",
  "shipping",
  "figure-it-out",
  "no-comments",
  "verification-skill",
  "unattended",
  "worktree",
  "slp",
  "style-cpp",
  "style-csharp",
  "style-dart",
  "style-general",
  "style-go",
  "style-html-css",
  "style-javascript",
  "style-python",
  "style-rust",
  "style-typescript",
] as const;

async function snapshotTree(root: string): Promise<Map<string, string>> {
  const snapshot = new Map<string, string>();
  const runtimeStoreFiles = new Set([
    ".maestro/maestro.db",
    ".maestro/maestro.db-shm",
    ".maestro/maestro.db-wal",
  ]);
  const visit = async (directory: string): Promise<void> => {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        await visit(path);
      } else if (entry.isFile()) {
        const repoPath = relative(root, path);
        if (!runtimeStoreFiles.has(repoPath)) {
          snapshot.set(repoPath, (await readFile(path)).toString("base64"));
        }
      }
    }
  };
  await visit(root);
  return snapshot;
}

test("621 recipe list prints the shipped catalog with one-line descriptions", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["recipe", "list"]);
    const lines = result.stdout.trim().split("\n");

    expect(result.exitCode).toBe(0);
    expect(lines.map((line) => line.split("\t", 1)[0])).toEqual([...recipeNames]);
    for (const line of lines) expect(line).toMatch(/^[^\t]+\t[^\n]+$/);
  });
});

test("622 recipe show serves investigation markdown byte-identically across repos", async () => {
  await withFixture(async (first) => {
    await withFixture(async (second) => {
      const firstResult = await runCli(first, ["recipe", "show", "investigation"]);
      const secondResult = await runCli(second, ["recipe", "show", "investigation"]);

      expect(firstResult.exitCode).toBe(0);
      expect(firstResult.stdout).toContain("# Investigation");
      expect(firstResult.stdout).toContain("## Steps");
      expect(firstResult.stdout).toContain("## Reply");
      expect(secondResult.stdout).toBe(firstResult.stdout);
    });
  });
});

test("623 unknown recipe errors list every valid recipe name", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["recipe", "show", "nosuch"]);

    expect(result.exitCode).not.toBe(0);
    for (const name of recipeNames) expect(result.stderr).toContain(name);
  });
});

test("624 recipe show leaves the repository tree unchanged", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["status"])).exitCode).toBe(0);
    const before = await snapshotTree(fixture.repo);
    const shown = await runCli(fixture, ["recipe", "show", "bug-fix"]);
    const after = await snapshotTree(fixture.repo);

    expect(shown.exitCode).toBe(0);
    expect(after).toEqual(before);
  });
});

test("625 the recipe plugin contributes and removes its brief pointer with enablement", async () => {
  await withFixture(async (fixture) => {
    const enabled = await runCli(fixture, ["hook", "record", "--event", "SessionStart"]);
    expect((await runCli(fixture, ["plugin", "disable", "recipe"])).exitCode).toBe(0);
    const disabled = await runCli(fixture, ["hook", "record", "--event", "SessionStart"]);

    expect(enabled.exitCode).toBe(0);
    expect(enabled.stdout).toContain("maestro recipe list");
    expect(disabled.exitCode).toBe(0);
    expect(disabled.stdout).not.toContain("maestro recipe list");
  });
});

test("d902 a checkout config that still names a deleted policy loads quietly", async () => {
  await withFixture(async (fixture) => {
    const deleted = [
      "policy-tdd",
      "policy-layer",
      "policy-witness",
      "policy-qa",
      "policy-research",
      "policy-lifecycle",
    ];
    await writeConfig(fixture, deleted.map((name) => ({ name, disabled: false })));
    const parent = idFrom(await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]));
    const id = idFrom(
      await runCli(fixture, ["work", "add", "plain fix", "--kind", "bug", "--parent", parent]),
    );
    const results = [
      await runCli(fixture, ["work", "start", id]),
      await runCli(fixture, ["work", "done", id, "--evidence", "suite green"]),
      await runCli(fixture, ["work", "start", parent]),
      await runCli(fixture, ["work", "done", parent, "--evidence", "child closed"]),
      await runCli(fixture, ["hook", "record", "--event", "SessionStart"]),
      await runCli(fixture, ["work", "list"]),
    ];
    const listed = await runCli(fixture, ["plugin", "list"]);

    for (const result of results) {
      expect(result.stderr).toBe("");
      expect(result.exitCode).toBe(0);
      for (const name of deleted) expect(result.stdout).not.toContain(name);
    }
    expect(listed.exitCode).toBe(0);
    expect(listed.stderr).toBe("");
  });
});

test("10 install keeps a policy choice once and drops the entries of deleted policies", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    await writeConfig(fixture, [
      { name: "policy-proof", disabled: true },
      { name: "policy-tdd", disabled: false },
      { name: "policy-lifecycle", disabled: true },
    ]);

    const installed = await runCli(fixture, ["install"], { PATH: path });
    const reinstalled = await runCli(fixture, ["install"], { PATH: path });
    const config = JSON.parse(
      await readFile(join(fixture.repo, ".maestro", "config"), "utf8"),
    ) as { plugins: Array<{ disabled: boolean; name: string }> };
    const names = config.plugins.map((entry) => entry.name);

    expect(installed.exitCode).toBe(0);
    expect(reinstalled.exitCode).toBe(0);
    expect(config.plugins).toContainEqual({ name: "policy-proof", disabled: true });
    expect(config.plugins).toContainEqual({ name: "policy-breakdown", disabled: false });
    expect(names.filter((name) => name === "policy-proof")).toHaveLength(1);
    expect(names).not.toContain("policy-tdd");
    expect(names).not.toContain("policy-lifecycle");
  });
}, 30_000);

test("559 [lint] the site carries the self-improvement loop end to end and links both ways", async () => {
  const docs = join(import.meta.dir, "..", "site", "src", "content", "docs");
  const guide = await readFile(join(docs, "guides", "self-improvement.md"), "utf8");
  const flatGuide = guide.replace(/\s+/g, " ");
  for (const phrase of [
    "administrative Maestro workflow outside the nine SLP operations",
    "maestro lesson file",
    "work return or note",
    "smallest doctrine edit",
    "maestro work add",
    "--to peer-improver",
    "maestro work take",
    "maestro work return",
    "golden replay",
    "--to peer-challenge",
    "maestro work accept",
    "maestro decide",
    "maestro lesson process",
    "Nothing deletes a lesson",
    "maestro lesson render",
    "~/maestro/PROJECT/",
    "never hand-edit",
  ]) {
    expect(flatGuide).toContain(phrase);
  }

  const sidebar = await readFile(join(import.meta.dir, "..", "site", "astro.config.mjs"), "utf8");
  expect(sidebar).toContain("slug: 'guides/self-improvement'");

  const neighbours = [
    { dir: "guides", file: "recipes-skills-plugins.md" },
    { dir: "reference", file: "cli.md" },
  ];
  for (const { dir, file } of neighbours) {
    const page = await readFile(join(docs, dir, file), "utf8");
    expect(page).toContain("/guides/self-improvement/");
  }
});
