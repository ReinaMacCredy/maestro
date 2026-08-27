import { expect, test } from "bun:test";
import { readFile, readdir } from "node:fs/promises";
import { join, relative } from "node:path";
import { idFrom, prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

const recipeNames = [
  "design",
  "work",
  "audit",
  "ship",
  "unattended",
  "learning",
  "worktree",
  "conflict-handoff",
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

function errorMessage(result: { stderr: string }): string {
  const parsed = JSON.parse(result.stderr) as { error?: { message?: unknown } };
  if (typeof parsed.error?.message !== "string") throw new Error("missing CLI error message");
  return parsed.error.message;
}

test("1 recipe list prints the shipped catalog with one-line descriptions", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["recipe", "list"]);
    const lines = result.stdout.trim().split("\n");

    expect(result.exitCode).toBe(0);
    expect(lines.map((line) => line.split("\t", 1)[0])).toEqual([...recipeNames]);
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
    const blockedMessage = errorMessage(blocked);

    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("policy-tdd");
    expect(blockedMessage).toContain(`maestro work done ${id}`);
    expect(blockedMessage).toContain('--claim "test:');
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
    const blockedMessage = errorMessage(blocked);

    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("policy-qa");
    expect(blockedMessage).toContain(`maestro work done ${parent}`);
    expect(blockedMessage).toContain('--claim "qa:');
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
    const blockedMessage = errorMessage(blocked);

    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("policy-research");
    expect(blockedMessage).toContain(`maestro work note ${id}`);
    expect(blockedMessage).toContain('"research:');
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

    const selfWitness = await runCli(
      fixture,
      ["work", "note", parent, "witness: owner self-review"],
      ownerEnv,
    );
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
    const blockedMessage = errorMessage(blocked);

    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("policy-witness");
    expect(blockedMessage).toContain(`maestro work note ${parent}`);
    expect(blockedMessage).toContain('"witness:');
    expect(blockedMessage).toContain("different session");
    expect(selfWitness.exitCode).toBe(0);
    expect(witnessed.exitCode).toBe(0);
    expect(passed.exitCode).toBe(0);
  });
});

test("10 fresh install ships four disabled policies and honest enable activates tdd", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], {
      PATH: path,
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

test("36 stacked prefixed gates explain one-invocation claim and proof pairs", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["plugin", "enable", "policy-tdd"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["plugin", "enable", "policy-qa"])).exitCode).toBe(0);
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "stacked gates", "--kind", "feature"]),
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

    const tddBlocked = await runCli(fixture, [
      "work",
      "done",
      parent,
      "--claim",
      "qa: manual path checked",
      "--proof",
      "observed expected output",
    ]);
    const qaBlocked = await runCli(fixture, [
      "work",
      "done",
      parent,
      "--claim",
      "test: regression covered",
      "--proof",
      "bun test passes",
    ]);
    const combined =
      `maestro work done ${parent} ` +
      `--claim "test: <test claim>" --proof "<test output>" ` +
      `--claim "qa: <checked behavior>" --proof "<QA evidence>"`;

    for (const [result, origin] of [
      [tddBlocked, "policy-tdd"],
      [qaBlocked, "policy-qa"],
    ] as const) {
      const error = JSON.parse(result.stderr).error as {
        code: string;
        message: string;
        origin: string;
      };
      expect(result.exitCode).not.toBe(0);
      expect(error.code).toBe("GATE_BLOCKED");
      expect(error.origin).toBe(origin);
      expect(error.message).toContain("multiple --claim/--proof pairs");
      expect(error.message).toContain(combined);
    }
  });
});

test("370 [lint] recipe slp and lane.md state the cross-role messaging convention (d687)", async () => {
  const recipe = await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8");
  const section = recipe.split("## Talking across roles")[1]?.split("\n## ")[0] ?? "";
  expect(section).toContain("[from <role>]");
  expect(section).toContain("[ask d<id>]");
  expect(section).toContain("maestro decision draft");
  expect(section).toContain("supervisor default, not owner instruction");
  expect(section).toContain("maestro work note");
  expect(section).toMatch(/answer is the record/i);

  // room.ts holds lane.md inside a template literal, so backticks are escaped there.
  const room = (await readFile(join(import.meta.dir, "..", "src", "plugins", "room.ts"), "utf8")).replace(/\\`/g, "`");
  expect(room).toMatch(/One lane per `herdr agent prompt` call/);
  expect(room).toMatch(/confirm `working` before briefing the next lane/);
  expect(room).toContain("[from <role>]");
});

test("371 [lint] recipe slp, IDENTITY.md and lane.md carry the cross-examination, Lead-per-scope handoff, and Supervisor binding text", async () => {
  const recipe = await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8");
  const cross = recipe.split("## Cross-examination")[1]?.split("\n## ")[0] ?? "";
  expect(cross).toMatch(/second\s+generation/i);
  expect(cross).toContain("CONFIRM");
  expect(cross).toContain("REOPEN_REQUEST");
  expect(cross).toMatch(/never prompt each other/i);
  const scope = recipe.split("## One Lead per scope")[1]?.split("\n## ")[0] ?? "";
  expect(scope).toContain("packet_ready");
  expect(scope).toContain("successor_acknowledged");
  expect(scope).toContain("predecessor_released");
  expect(scope).toMatch(/failed approaches/i);
  const binding = recipe.split("## Supervisor binding")[1]?.split("\n## ")[0] ?? "";
  expect(binding).toMatch(/recovery.*lease/i);
  expect(binding).toMatch(/human decision needed: yes/i);
  expect(binding).toMatch(/notebook/i);

  const room = (await readFile(join(import.meta.dir, "..", "src", "plugins", "room.ts"), "utf8")).replace(/\\`/g, "`");
  expect(room).toContain("Raw transcript access: denied");
  expect(room).toContain("Recovery or replacement lease: none");
  expect(room).toMatch(/cross-examination/i);
});

test("372 [lint] recipe slp and lane.md state the one-dispatch-one-handback boundary (d697)", async () => {
  const recipe = await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8");
  const boundary = recipe.split("## Handback boundary")[1]?.split("\n## ")[0] ?? "";
  expect(boundary).toContain("exactly one handback");
  expect(boundary).toMatch(/does not reopen/i);
  expect(boundary).toMatch(/new sequential dispatch/i);
  expect(boundary).toMatch(/accepts the new dispatch before continuing/i);
  expect(boundary).toContain('"after h<id>: <evidence>"');
  expect(boundary).toMatch(/never starts with `failed:`/);

  const room = (await readFile(join(import.meta.dir, "..", "src", "plugins", "room.ts"), "utf8")).replace(/\\`/g, "`");
  expect(room).toMatch(/file exactly once, when the stop condition is met/);
  expect(room).toMatch(/second stop point needs a second dispatch/);
  expect(room).toMatch(/never changes an assignment/);
  expect(room).toContain('"after h<id>: <evidence>"');
});

test("413 [lint] recipe slp records the Lead view before a council is sealed", async () => {
  const recipe = await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8");
  const cross = recipe.split("## Cross-examination")[1]?.split("\n## ")[0] ?? "";
  expect(cross.replace(/\s+/g, " ")).toContain(
    "A council's first views stay sealed until every member returns (blind design). " +
      "The Lead writes its own first view outside the store (NOTES or a private file) and drafts it as a decision only after the seal opens; a draft on the council's work item while it is sealed is visible to every lane.",
  );
});
