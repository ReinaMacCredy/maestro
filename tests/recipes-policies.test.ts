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

test("454 [lint] recipe slp withdraws losing first views and duplicates instead of locking them", async () => {
  const recipe = await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8");
  const section = recipe.split("## Talking across roles")[1]?.split("\n## ")[0] ?? "";

  expect(section).toContain(
    "A first view or a duplicate that lost is withdrawn with its reason, never locked",
  );
  expect(section).toContain('maestro decision withdraw d<id> --reason "<why>"');
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

test("422 [lint] recipe slp and the scenarios page carry the intake contract (d700)", async () => {
  const recipe = await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8");
  const section = recipe.split("## Reading the owner's prompt")[1]?.split("\n## ")[0] ?? "";
  const flat = section.replace(/\s+/g, " ");
  for (const phrase of [
    "never asks the owner which shape to use",
    "With no signal both score 0",
    "about the outcome, never about the route",
    "the adjacent route it did not take",
    "without a time estimate",
    "score (0-10) and the problem in one sentence",
    "The announcement never blocks",
  ]) {
    expect(flat).toContain(phrase);
  }
  expect(recipe.indexOf("## Reading the owner's prompt")).toBeLessThan(recipe.indexOf("## Topology invariants"));
  const docs = await readFile(
    join(import.meta.dir, "..", "site", "src", "content", "docs", "guides", "slp-scenarios.md"),
    "utf8",
  );
  const page = docs.split("## How the Lead reads a prompt")[1]?.split("\n## ")[0] ?? "";
  const flatPage = page.replace(/\s+/g, " ");
  expect(flatPage).toContain("You never name a shape");
  expect(flatPage).toContain("It never asks how many lanes to open");
  expect(flatPage).toContain("the route it did not take");
  expect(flatPage).toMatch(/Score (?:10|[0-9])\./);
});

test("425 [lint] recipe slp, lanes.md and roles.md say which boundaries are enforced and which are soft-audited (w494)", async () => {
  const docs = join(import.meta.dir, "..", "site", "src", "content", "docs", "concepts");
  const recipe = await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8");
  const lanes = await readFile(join(docs, "lanes.md"), "utf8");
  const roles = await readFile(join(docs, "roles.md"), "utf8");
  for (const text of [recipe, lanes, roles]) expect(text).toContain("soft-audited");
  expect(recipe.replace(/\s+/g, " ")).toContain(
    "A full-access process under a no-write lease is no-write by contract; maestro enforces the lease (LEASE_HELD, the lane gate on work start), not the filesystem.",
  );
  expect(lanes).toContain("| id | Boundary | Enforced by | Proof | Soft-audited |");
  expect(lanes).toContain(
    "| B10 | role identity | nothing | soft-audited | the pane name the opener set (d709) |",
  );
  expect(lanes).not.toContain("performs no-write discovery and reports state.");
  expect(roles.replace(/\s+/g, " ")).toContain("Write authority and acceptance authority are soft-audited");
});

test("437 [lint] recipe slp names the repository Workspace Protocol surface", async () => {
  const recipe = await readFile(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"), "utf8");
  const workspaceProtocol =
    recipe.split("\n").find((line) => line.startsWith("| Workspace protocol |")) ?? "";

  expect(workspaceProtocol).toContain("`AGENTS.md` and `CLAUDE.md` text outside the managed block");
  expect(workspaceProtocol).toContain("Workspace Protocol");
  for (const localRule of ["protected areas", "hotspots", "restart rules", "local verification"]) {
    expect(workspaceProtocol).toContain(localRule);
  }
});

test("443 [lint] recipe slp defines the Supervisor episode-to-rule review loop", async () => {
  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const binding = recipe.split("## Supervisor binding")[1]?.split("\n## ")[0] ?? "";
  const flat = binding.replace(/\s+/g, " ");

  expect(flat).toContain("An episode is a REPEATED_FAILURE packet plus its work trace.");
  expect(flat).toContain("The Supervisor aggregates recurring mechanisms in room notes or decisions.");
  expect(flat).toContain("A rule it promotes records owner, review date, evidence, and removal trigger.");
  expect(flat).toContain("A rule past its review date is reviewed or deleted.");
});

test("445 [lint] handoff receipts are exact, searchable decisions with soft-audited ordering", async () => {
  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const roles = await readFile(
    join(import.meta.dir, "..", "site", "src", "content", "docs", "concepts", "roles.md"),
    "utf8",
  );
  const scope = recipe.split("## One Lead per scope")[1]?.split("\n## ")[0] ?? "";
  const handoff = roles.split("## Lead handoff")[1]?.split("\n## ")[0] ?? "";
  const draft = 'maestro decision draft "<receipt> <bundle-id>" --work <id>';

  for (const text of [scope, handoff]) {
    expect(text).toContain(draft);
    for (const receipt of [
      "packet_ready",
      "successor_authorized",
      "successor_acknowledged",
      "predecessor_released",
    ]) {
      expect(text).toContain(receipt);
    }
  }
  expect(scope).toContain('maestro search "packet_ready"');
  expect(handoff.replace(/\s+/g, " ")).toContain(
    "Packet completeness, receipt order, and break-before-make are soft-audited.",
  );
});

test("446 [lint] recipe slp explains the global unreturned-dispatch threshold", async () => {
  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const feed = recipe.split("## Supervisor feed and packet")[1]?.split("\n## ")[0] ?? "";
  const flat = feed.replace(/\s+/g, " ");

  expect(flat).toContain("`DISPATCH_UNRETURNED` fires after `--dispatch-stale` hours");
  expect(flat).toContain(
    "A lane expected to run longer is opened with its expected duration in the stop condition",
  );
  expect(flat).toContain("the Lead reads `maestro attention --dispatch-stale <h>` for it");
  expect(flat).toContain("`maestro brief` in the room uses the default.");
});

test("449 [lint] recipe slp states the harness boundary for topology invariant 4", async () => {
  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const topology = recipe.split("## Topology invariants")[1]?.split("\n## ")[0] ?? "";
  expect(topology.replace(/\s+/g, " ")).toContain(
    "For Claude panes, the `PreToolUse` hook enforces invariant 4 when a session holds an open dispatch; Codex has no `PreToolUse` hook and stays bound by this text.",
  );
});

test("500 [lint] recipe slp treats model routing as guidance-only two-harness reference (d711)", async () => {
  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const model = recipe.split("## Model")[1]?.split("\n## ")[0] ?? "";
  const guidance = model.trim().split("\n\n")[0] ?? "";
  const flat = guidance.replace(/\s+/g, " ");
  const rungTable = model.split("### Thinking level by lane")[0] ?? "";
  const thinking = model.split("### Thinking level by lane")[1] ?? "";

  expect(guidance).not.toContain("\n\n");
  expect(flat).toContain("The Lead picks a lane's model the way it picks a sub-agent's");
  expect(flat).toContain("the room picks the Lead's model");
  expect(flat).toContain("Nothing records, enforces, or prints the choice.");
  expect(flat).toContain("Model names rot");
  expect(flat).toContain("the owner keeps the current examples for those columns in `OWNER.md`");
  expect(model).toContain("These examples are dated 2026-08-28 and owner-editable.");
  expect(rungTable.split("\n").filter((line) => line.startsWith("|"))).toEqual([
    "| rung | use it for | example Claude Code | example Codex CLI |",
    "|---|---|---|---|",
    "| cheap | no-write lanes (scout, shadow), mechanical work, short brief, inline verify | Sonnet 5 (`--model sonnet`); Haiku 4.5 is cheaper but has no effort dial | gpt-5.6-luna (`-m gpt-5.6-luna`) |",
    "| strong | delivery with red/green, long brief, kernel or store, decision lanes | Opus 5 (`--model opus`) | gpt-5.6-terra (`-m gpt-5.6-terra`); gpt-5.5 is the fallback many still trust |",
    "| diverse | challenge and council: a different model family from the lane that produced the view; Claude and Codex are the two harnesses maestro wires today; a third family (Grok 4.6, Gemini 3.7 Flash) needs a third harness, which is a repository change (`sessions.harness` accepts `claude | codex`, `src/kernel/sessions.ts`) | Claude | Codex |",
    "| lead | reviews handbacks, closes cards, settles forks | Fable 5 (`--model fable`) | gpt-5.6-sol (`-m gpt-5.6-sol`) |",
  ]);
  expect(thinking.split("\n").filter((line) => line.startsWith("|"))).toEqual([
    "| lane | Claude | Codex |",
    "|---|---|---|",
    "| scout | medium | medium |",
    "| decision | xhigh | xhigh |",
    "| delivery | high | high |",
    "| challenge | xhigh | xhigh |",
    "| shadow | low | low |",
  ]);
  expect(thinking).toContain(
    "Keep one effort level for a whole session: the level sits in the prompt prefix cache, so changing it mid-session drops the cache; `max` is for one genuinely hard fork, not a default (community measurement: about 2.2x time and 1.7x tokens versus `high`).",
  );
  expect(thinking).toContain(
    "Pass the level with Claude Code's `--effort <level>` or Codex's `-c model_reasoning_effort=<level>`.",
  );
});
