import { expect, test } from "bun:test";
import { chmod, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

const recipeNames = [
  "design",
  "work",
  "audit",
  "ship",
  "unattended",
  "learning",
  "worktree",
  "conflict-handoff",
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
  const visit = async (directory: string): Promise<void> => {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        await visit(path);
      } else if (entry.isFile()) {
        snapshot.set(relative(root, path), (await readFile(path)).toString("base64"));
      }
    }
  };
  await visit(root);
  return snapshot;
}

function isolatedPath(localBin: string): string {
  return [localBin, dirname(process.execPath), "/usr/bin", "/bin"].join(":");
}

test("1 recipe list prints the shipped catalog with one-line descriptions", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["recipe", "list"]);
    const lines = result.stdout.trim().split("\n");

    expect(result.exitCode).toBe(0);
    expect(lines.map((line) => line.split("\t", 1)[0])).toEqual(recipeNames);
    for (const line of lines) expect(line).toMatch(/^[^\t]+\t[^\n]+$/);
  });
});

test("2 recipe show serves design markdown byte-identically across repos", async () => {
  await withFixture(async (first) => {
    await withFixture(async (second) => {
      const firstResult = await runCli(first, ["recipe", "show", "design"]);
      const secondResult = await runCli(second, ["recipe", "show", "design"]);

      expect(firstResult.exitCode).toBe(0);
      expect(firstResult.stdout).toContain("# Design");
      expect(firstResult.stdout).toContain("## Loop anatomy");
      for (const phase of ["Perceive", "Choose", "Act", "Observe", "Learn", "Continue"]) {
        expect(firstResult.stdout).toContain(`### ${phase}`);
      }
      expect(secondResult.stdout).toBe(firstResult.stdout);
    });
  });
});

test("3 unknown recipe errors list every valid recipe name", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["recipe", "show", "nosuch"]);

    expect(result.exitCode).not.toBe(0);
    for (const name of recipeNames) expect(result.stderr).toContain(name);
  });
});

test("4 recipe show leaves the repository tree unchanged", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["status"])).exitCode).toBe(0);
    const before = await snapshotTree(fixture.repo);
    const shown = await runCli(fixture, ["recipe", "show", "work"]);
    const after = await snapshotTree(fixture.repo);

    expect(shown.exitCode).toBe(0);
    expect(after).toEqual(before);
  });
});

test("5 the recipe plugin contributes and removes its brief pointer with enablement", async () => {
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

test("6 policy-tdd blocks untagged write completion and passes a test-tagged pair", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["plugin", "enable", "policy-tdd"])).exitCode).toBe(0);
    const id = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "write regression",
        "--kind",
        "bug",
        "--atomic-reason",
        "small fix",
      ]),
    );
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);

    const blocked = await runCli(fixture, [
      "work",
      "done",
      id,
      "--claim",
      "regression covered",
      "--proof",
      "bun test passes",
    ]);
    const passed = await runCli(fixture, [
      "work",
      "done",
      id,
      "--claim",
      "test: regression covered",
      "--proof",
      "bun test passes",
    ]);
    const shown = await runCli(fixture, ["work", "show", id]);

    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("policy-tdd");
    expect(blocked.stderr).toContain(`maestro work done ${id}`);
    expect(blocked.stderr).toContain('--claim "test:');
    expect(passed.exitCode).toBe(0);
    expect(shown.stdout).toContain("claim: test: regression covered");
    expect(shown.stdout).toContain("proof: bun test passes");
  });
});

test("7 policy-qa blocks an untagged parent close and passes a qa-tagged pair", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["plugin", "enable", "policy-qa"])).exitCode).toBe(0);
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "release slice", "--kind", "feature"]),
    );
    idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "implemented child",
        "--kind",
        "idea",
        "--parent",
        parent,
      ]),
    );
    expect((await runCli(fixture, ["work", "start", parent])).exitCode).toBe(0);

    const blocked = await runCli(fixture, [
      "work",
      "done",
      parent,
      "--claim",
      "manual path checked",
      "--proof",
      "observed expected output",
    ]);
    const passed = await runCli(fixture, [
      "work",
      "done",
      parent,
      "--claim",
      "qa: manual path checked",
      "--proof",
      "observed expected output",
    ]);

    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("policy-qa");
    expect(blocked.stderr).toContain(`maestro work done ${parent}`);
    expect(blocked.stderr).toContain('--claim "qa:');
    expect(passed.exitCode).toBe(0);
  });
});

test("8 policy-research blocks a fresh feature and passes after a research note", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["plugin", "enable", "policy-research"])).exitCode).toBe(0);
    const id = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "new capability",
        "--kind",
        "feature",
        "--atomic-reason",
        "single boundary",
      ]),
    );

    const blocked = await runCli(fixture, ["work", "start", id]);
    const noted = await runCli(fixture, ["work", "note", id, "research: precedent inspected"]);
    const passed = await runCli(fixture, ["work", "start", id]);

    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("policy-research");
    expect(blocked.stderr).toContain(`maestro work note ${id}`);
    expect(blocked.stderr).toContain('"research:');
    expect(noted.exitCode).toBe(0);
    expect(passed.exitCode).toBe(0);
  });
});

test("9 policy-witness requires a witness note from a different session", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["plugin", "enable", "policy-witness"])).exitCode).toBe(0);
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "independent close", "--kind", "feature"]),
    );
    idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "child scope",
        "--kind",
        "idea",
        "--parent",
        parent,
      ]),
    );
    const ownerEnv = {
      MAESTRO_SESSION_ID: "owner-session",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    expect((await runCli(fixture, ["work", "start", parent], ownerEnv)).exitCode).toBe(0);

    const blocked = await runCli(
      fixture,
      ["work", "done", parent, "--evidence", "owner verification"],
      ownerEnv,
    );
    const witnessed = await runCli(
      fixture,
      ["work", "note", parent, "witness: independent review passed"],
      {
        MAESTRO_SESSION_ID: "witness-session",
        MAESTRO_SESSION_PID: String(process.pid),
      },
    );
    const passed = await runCli(
      fixture,
      ["work", "done", parent, "--evidence", "owner verification"],
      ownerEnv,
    );

    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("policy-witness");
    expect(blocked.stderr).toContain(`maestro work note ${parent}`);
    expect(blocked.stderr).toContain('"witness:');
    expect(blocked.stderr).toContain("different session");
    expect(witnessed.exitCode).toBe(0);
    expect(passed.exitCode).toBe(0);
  });
});

test("10 fresh install ships four disabled policies and honest enable activates tdd", async () => {
  await withFixture(async (fixture) => {
    const localBin = join(fixture.home, ".local", "bin");
    const shim = join(localBin, "maestro");
    await mkdir(localBin, { recursive: true });
    await writeFile(shim, "#!/bin/sh\necho legacy-maestro\n");
    await chmod(shim, 0o755);

    const installed = await runCli(fixture, ["install"], {
      PATH: isolatedPath(localBin),
    });
    const config = JSON.parse(
      await readFile(join(fixture.repo, ".maestro", "config"), "utf8"),
    ) as { plugins: Array<{ disabled: boolean; name: string }> };
    const listed = await runCli(fixture, ["plugin", "list"]);
    for (const name of ["policy-tdd", "policy-qa", "policy-research", "policy-witness"]) {
      expect(config.plugins).toContainEqual({ name, disabled: true });
      expect(listed.stdout).toContain(`${name}\tbuilt-in\tdisabled`);
      expect(
        await Bun.file(join(fixture.home, ".maestro", "runtime", "src", "plugins", `${name}.ts`)).exists(),
      ).toBe(true);
    }

    const parent = idFrom(
      await runCli(fixture, ["work", "add", "disabled gates", "--kind", "feature"]),
    );
    idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "child",
        "--kind",
        "idea",
        "--parent",
        parent,
      ]),
    );
    const disabledStart = await runCli(fixture, ["work", "start", parent]);
    const disabledDone = await runCli(fixture, [
      "work",
      "done",
      parent,
      "--claim",
      "untagged close",
      "--proof",
      "still allowed",
    ]);

    const tddWork = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "enabled tdd",
        "--kind",
        "bug",
        "--atomic-reason",
        "small fix",
      ]),
    );
    expect((await runCli(fixture, ["work", "start", tddWork])).exitCode).toBe(0);
    expect((await runCli(fixture, ["plugin", "enable", "policy-tdd"])).exitCode).toBe(0);
    const enabledBlock = await runCli(fixture, [
      "work",
      "done",
      tddWork,
      "--claim",
      "untagged test",
      "--proof",
      "must now block",
    ]);

    expect(installed.exitCode).toBe(0);
    expect(disabledStart.exitCode).toBe(0);
    expect(disabledDone.exitCode).toBe(0);
    expect(enabledBlock.exitCode).not.toBe(0);
    expect(enabledBlock.stderr).toContain("policy-tdd");
  });
});
