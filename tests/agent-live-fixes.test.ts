import { expect, test } from "bun:test";
import { chmod, mkdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

test("91 empty ready points at work add", async () => {
  await withFixture(async (fixture) => {
    const empty = await runCli(fixture, ["ready"]);
    expect(empty.exitCode).toBe(0);
    expect(empty.stdout).toContain("no ready work");
    expect(empty.stdout).toContain('maestro work add "<title>"');
  });
});

test("92 gate envelope carries origin without a duplicated reason; lease keeps command", async () => {
  await withFixture(async (fixture) => {
    const gated = idFrom(await runCli(fixture, ["work", "add", "gated root", "--kind", "feature"]));
    const blocked = await runCli(fixture, ["work", "start", gated]);
    expect(blocked.exitCode).not.toBe(0);
    const gateError = (JSON.parse(blocked.stderr) as { error: Record<string, unknown> }).error;
    expect(gateError.code).toBe("GATE_BLOCKED");
    expect(gateError.origin).toBe("policy-breakdown");
    expect(typeof gateError.message).toBe("string");
    expect("reason" in gateError).toBe(false);

    const loose = idFrom(
      await runCli(fixture, ["work", "add", "atomic item", "--kind", "task", "--atomic-reason", "spike"]),
    );
    const unleased = await runCli(fixture, ["work", "done", loose, "--evidence", "done"]);
    expect(unleased.exitCode).not.toBe(0);
    const leaseError = (JSON.parse(unleased.stderr) as { error: Record<string, unknown> }).error;
    expect(leaseError.code).toBe("LEASE_REQUIRED");
    expect(leaseError.command).toBe(`maestro work start ${loose}`);
  });
});

test("93 breakdown structure is visible: show lists children, list indents them, ready annotates the parent", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]));
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child task", "--kind", "task", "--parent", parent]),
    );

    const show = await runCli(fixture, ["work", "show", parent]);
    expect(show.exitCode).toBe(0);
    expect(show.stdout).toContain(`child: ${child} [open] child task`);

    const list = await runCli(fixture, ["work", "list"]);
    expect(list.exitCode).toBe(0);
    expect(list.stdout).toContain(`${parent} [open] parent feature`);
    expect(list.stdout).toContain(`\n  ${child} [open] child task`);

    const ready = await runCli(fixture, ["ready", "--json"]);
    const data = JSON.parse(ready.stdout).data as {
      gated: Array<{ id: string; origin: string }>;
      works: Array<{ id: string }>;
    };
    expect(data.works.map((work) => work.id)).toEqual([child]);
    expect(data.gated.map((work) => work.id)).toEqual([parent]);
    expect(data.gated[0]?.origin).toBe("policy-breakdown");
  });
});

test("94 a parent with open children cannot start or complete", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]));
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child task", "--kind", "task", "--parent", parent]),
    );

    const start = await runCli(fixture, ["work", "start", parent]);
    expect(start.exitCode).not.toBe(0);
    const startError = (JSON.parse(start.stderr) as { error: Record<string, unknown> }).error;
    expect(startError.code).toBe("GATE_BLOCKED");
    expect(startError.origin).toBe("policy-breakdown");
    expect(String(startError.message)).toContain(child);
    expect(String(startError.message)).toContain(`maestro work start ${child}`);

    // Reach the done gate via a parent started before its child existed.
    const grown = idFrom(
      await runCli(fixture, ["work", "add", "grown scope", "--kind", "task", "--atomic-reason", "spike"]),
    );
    expect((await runCli(fixture, ["work", "start", grown])).exitCode).toBe(0);
    const late = idFrom(
      await runCli(fixture, ["work", "add", "late child", "--kind", "task", "--parent", grown]),
    );
    const done = await runCli(fixture, ["work", "done", grown, "--evidence", "done"]);
    expect(done.exitCode).not.toBe(0);
    const doneError = (JSON.parse(done.stderr) as { error: Record<string, unknown> }).error;
    expect(doneError.code).toBe("GATE_BLOCKED");
    expect(String(doneError.message)).toContain(late);

    // Completing the child reopens the parent path.
    expect((await runCli(fixture, ["work", "start", late])).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "done", late, "--evidence", "child done"])).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "done", grown, "--evidence", "done"])).exitCode).toBe(0);
  });
});

test("95 status reflects completion instead of pinning the last start", async () => {
  await withFixture(async (fixture) => {
    const item = idFrom(
      await runCli(fixture, ["work", "add", "atomic item", "--kind", "task", "--atomic-reason", "spike"]),
    );
    expect((await runCli(fixture, ["work", "start", item])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", item, "--evidence", "done"])).exitCode).toBe(0);

    const status = await runCli(fixture, ["status"]);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain("work.done");
    expect(status.stdout).not.toContain("work.start pid");
  });
});

test("96 installed harness docs name the stderr JSON failure envelope", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const block = await readFile(join(fixture.repo, "CLAUDE.md"), "utf8");
    expect(block).toContain("JSON error envelope");
    expect(block).toContain("stderr");
    expect(block).toContain("when the fix is mechanical");
  });
});

test("97 work recipe disambiguates claim tagging and evidence forms", async () => {
  await withFixture(async (fixture) => {
    const recipe = await runCli(fixture, ["recipe", "show", "work"]);
    expect(recipe.exitCode).toBe(0);
    expect(recipe.stdout).toContain("the tag prefix goes on the claim");
    expect(recipe.stdout).toContain("opaque");
  });
});

test("98 session lastEvent records what happened, not what was attempted", async () => {
  await withFixture(async (fixture) => {
    const session = { MAESTRO_SESSION_ID: "honest-session", MAESTRO_SESSION_PID: String(process.pid) };
    expect(
      (await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session)).exitCode,
    ).toBe(0);

    const gated = idFrom(
      await runCli(fixture, ["work", "add", "gated root", "--kind", "feature"], session),
    );
    expect((await runCli(fixture, ["work", "start", gated], session)).exitCode).not.toBe(0);
    const afterRefusedStart = await runCli(fixture, ["status"], session);
    expect(afterRefusedStart.stdout).toContain("SessionStart");
    expect(afterRefusedStart.stdout).not.toContain("work.start");

    const item = idFrom(
      await runCli(
        fixture,
        ["work", "add", "atomic item", "--kind", "task", "--atomic-reason", "spike"],
        session,
      ),
    );
    expect((await runCli(fixture, ["work", "start", item], session)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", item], session)).exitCode).not.toBe(0);
    const afterRefusedDone = await runCli(fixture, ["status"], session);
    expect(afterRefusedDone.stdout).toContain("work.start");
    expect(afterRefusedDone.stdout).not.toContain("work.done");

    expect(
      (await runCli(fixture, ["work", "done", item, "--evidence", "done"], session)).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["status"], session)).stdout).toContain("work.done");
  });
});

test("99 status renders held work per session", async () => {
  await withFixture(async (fixture) => {
    const session = { MAESTRO_SESSION_ID: "holder-session", MAESTRO_SESSION_PID: String(process.pid) };
    expect(
      (await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session)).exitCode,
    ).toBe(0);
    const item = idFrom(
      await runCli(
        fixture,
        ["work", "add", "held item", "--kind", "task", "--atomic-reason", "spike"],
        session,
      ),
    );
    expect((await runCli(fixture, ["work", "start", item], session)).exitCode).toBe(0);

    const holding = await runCli(fixture, ["status"], session);
    expect(holding.exitCode).toBe(0);
    expect(holding.stdout).toContain(`holds: ${item}`);

    expect(
      (await runCli(fixture, ["work", "done", item, "--evidence", "done"], session)).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["status"], session)).stdout).toContain("holds: none");
  });
});

test("100 completing already-done work fails INVALID_STATE, not a lease dead-end", async () => {
  await withFixture(async (fixture) => {
    const item = idFrom(
      await runCli(fixture, ["work", "add", "one shot", "--kind", "task", "--atomic-reason", "spike"]),
    );
    expect((await runCli(fixture, ["work", "start", item])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", item, "--evidence", "done"])).exitCode).toBe(0);

    const again = await runCli(fixture, ["work", "done", item, "--evidence", "again"]);
    expect(again.exitCode).not.toBe(0);
    const error = (JSON.parse(again.stderr) as { error: Record<string, unknown> }).error;
    expect(error.code).toBe("INVALID_STATE");
    expect(String(error.message)).toContain("already done");
    expect(String(error.message)).not.toContain("work start");
  });
});

test("101 NOT_FOUND names the listing command", async () => {
  await withFixture(async (fixture) => {
    const missing = await runCli(fixture, ["work", "start", "w99"]);
    expect(missing.exitCode).not.toBe(0);
    const error = (JSON.parse(missing.stderr) as { error: Record<string, unknown> }).error;
    expect(error.code).toBe("NOT_FOUND");
    expect(String(error.message)).toContain("run: maestro work list");
    expect(error.command).toBe("maestro work list");
  });
});

test("102 UNKNOWN_FLAG names the help command for its verb", async () => {
  await withFixture(async (fixture) => {
    const bogus = await runCli(fixture, ["work", "list", "--bogus"]);
    expect(bogus.exitCode).not.toBe(0);
    const error = (JSON.parse(bogus.stderr) as { error: Record<string, unknown> }).error;
    expect(error.code).toBe("UNKNOWN_FLAG");
    expect(String(error.message)).toContain("run: maestro help work");
    expect(error.command).toBe("maestro help work");
  });
});

test("103 plugin list states what each policy requires", async () => {
  await withFixture(async (fixture) => {
    const list = await runCli(fixture, ["plugin", "list", "--json"]);
    expect(list.exitCode).toBe(0);
    const plugins = (JSON.parse(list.stdout) as {
      data: { plugins: Array<{ name: string; requires?: string }> };
    }).data.plugins;
    const requiresFor = (name: string) =>
      plugins.find((plugin) => plugin.name === name)?.requires ?? "";
    expect(requiresFor("policy-proof")).toContain("--evidence");
    expect(requiresFor("policy-proof")).toContain("--claim");
    expect(requiresFor("policy-breakdown")).toContain("--atomic-reason");
    expect(requiresFor("policy-tdd")).toContain("test:");
    expect(requiresFor("policy-qa")).toContain("qa:");

    const text = await runCli(fixture, ["plugin", "list"]);
    expect(text.stdout).toContain("--atomic-reason");
    expect(text.stdout).toContain("test:");
  });
});

test("104 a cancelled child does not satisfy the breakdown gate", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]));
    const child = idFrom(
      await runCli(fixture, ["work", "add", "throwaway", "--kind", "task", "--parent", parent]),
    );
    expect(
      (await runCli(fixture, ["work", "cancel", child, "--reason", "not doing this"])).exitCode,
    ).toBe(0);

    const blocked = await runCli(fixture, ["work", "start", parent]);
    expect(blocked.exitCode).not.toBe(0);
    const error = (JSON.parse(blocked.stderr) as { error: Record<string, unknown> }).error;
    expect(error.code).toBe("GATE_BLOCKED");
    expect(error.origin).toBe("policy-breakdown");

    const ready = await runCli(fixture, ["ready"]);
    expect(ready.stdout).toContain(`${parent} parent feature [gated by policy-breakdown`);
  });
});

test("105 trace on an unknown id fails NOT_FOUND instead of empty success", async () => {
  await withFixture(async (fixture) => {
    const missing = await runCli(fixture, ["trace", "w99"]);
    expect(missing.exitCode).not.toBe(0);
    const error = (JSON.parse(missing.stderr) as { error: Record<string, unknown> }).error;
    expect(error.code).toBe("NOT_FOUND");
    expect(error.command).toBe("maestro work list");
  });
});

test("106 empty ready with held work points at finishing it", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(
      await runCli(fixture, ["work", "add", "solo task", "--kind", "task", "--atomic-reason", "single edit"]),
    );
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);

    const ready = await runCli(fixture, ["ready"]);
    expect(ready.exitCode).toBe(0);
    expect(ready.stdout).toContain(`you hold ${id}`);
    expect(ready.stdout).toContain(`maestro work done ${id}`);
    expect(ready.stdout).not.toContain('work add "<title>"');
  });
});

test("107 install reports what it wrote and names the dual-harness mirror", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    expect(installed.stdout).toContain(".claude/");
    expect(installed.stdout).toContain(".codex/");
    expect(installed.stdout).toContain("AGENTS.md");
    expect(installed.stdout).toContain("CLAUDE.md");
    expect(installed.stdout).toContain("same maestro block");
  });
});

test("108 help documents the kind vocabulary and ready's gated listing", async () => {
  await withFixture(async (fixture) => {
    const help = await runCli(fixture, ["help", "work"]);
    expect(help.exitCode).toBe(0);
    expect(help.stdout).toContain("feature|task|bug|chore|implement|idea|research");

    const root = await runCli(fixture, ["help"]);
    expect(root.exitCode).toBe(0);
    expect(root.stdout).toContain("gated");
  });
});

test("109 current holders can abandon active work and dead holders recover before cancellation", async () => {
  await withFixture(async (fixture) => {
    const current = {
      MAESTRO_SESSION_ID: "current-holder",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const peer = {
      MAESTRO_SESSION_ID: "other-session",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const staleHolder = Bun.spawn(["sleep", "30"]);
    try {
      const stale = idFrom(
        await runCli(fixture, ["work", "add", "stale lease", "--kind", "idea"], {
          MAESTRO_SESSION_ID: "stale-holder",
          MAESTRO_SESSION_PID: String(staleHolder.pid),
        }),
      );
      expect(
        (await runCli(fixture, ["work", "start", stale], {
          MAESTRO_SESSION_ID: "stale-holder",
          MAESTRO_SESSION_PID: String(staleHolder.pid),
        })).exitCode,
      ).toBe(0);
      staleHolder.kill();
      await staleHolder.exited;
      expect(
        (await runCli(fixture, ["work", "cancel", stale, "--reason", "holder died"], current))
          .exitCode,
      ).toBe(0);

      const active = idFrom(
        await runCli(fixture, ["work", "add", "abandoned attempt", "--kind", "idea"], current),
      );
      expect((await runCli(fixture, ["work", "start", active], current)).exitCode).toBe(0);

      const foreign = await runCli(
        fixture,
        ["work", "cancel", active, "--reason", "not mine"],
        peer,
      );
      expect(foreign.exitCode).not.toBe(0);
      expect((JSON.parse(foreign.stderr) as { error: { code: string } }).error.code).toBe(
        "LEASE_HELD",
      );

      const abandoned = await runCli(
        fixture,
        ["work", "cancel", active, "--reason", "approach is invalid"],
        current,
      );
      expect(abandoned.exitCode).toBe(0);
      const shown = await runCli(fixture, ["work", "show", active], current);
      expect(shown.stdout).toContain(`[cancelled] abandoned attempt`);
      expect(shown.stdout).toContain("cancel reason: approach is invalid");
      expect(shown.stdout).not.toContain("held by:");
      expect((await runCli(fixture, ["status"], current)).stdout).toContain("holds: none");
    } finally {
      staleHolder.kill();
    }
  });
});

test("110 terminal parents refuse new children and point at top-level work", async () => {
  await withFixture(async (fixture) => {
    const done = idFrom(await runCli(fixture, ["work", "add", "finished parent", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", done])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", done, "--evidence", "finished"])).exitCode).toBe(0);

    const cancelled = idFrom(
      await runCli(fixture, ["work", "add", "cancelled parent", "--kind", "idea"]),
    );
    expect(
      (await runCli(fixture, ["work", "cancel", cancelled, "--reason", "obsolete"])).exitCode,
    ).toBe(0);

    for (const [parent, state] of [[done, "done"], [cancelled, "cancelled"]] as const) {
      const child = await runCli(fixture, [
        "work",
        "add",
        `child of ${state}`,
        "--parent",
        parent,
        "--kind",
        "task",
      ]);
      expect(child.exitCode).not.toBe(0);
      const error = (JSON.parse(child.stderr) as { error: { code: string; message: string } }).error;
      expect(error.code).toBe("INVALID_STATE");
      expect(error.message).toContain(state);
      expect(error.message).toContain("top-level work");
    }

    const list = await runCli(fixture, ["work", "list"]);
    expect(list.stdout).not.toContain("child of done");
    expect(list.stdout).not.toContain("child of cancelled");
  });
});

test("111 work show renders recorded notes", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "learning loop", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "note", id, "first correction"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "note", id, "second correction"])).exitCode).toBe(0);

    const shown = await runCli(fixture, ["work", "show", id]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("note: first correction");
    expect(shown.stdout).toContain("note: second correction");
  });
});

test("112 help documents --json only for commands that accept it", async () => {
  await withFixture(async (fixture) => {
    const supported = [
      ["status"],
      ["ready"],
      ["msg", "send"],
      ["search"],
      ["work", "list"],
      ["decision", "list"],
      ["recipe", "list"],
      ["plugin", "list"],
    ];
    for (const command of supported) {
      const help = await runCli(fixture, ["help", ...command]);
      expect(help.exitCode).toBe(0);
      expect(help.stdout).toContain("--json");
    }

    for (const command of [["work", "show"], ["msg", "read"], ["trace"]]) {
      const help = await runCli(fixture, ["help", ...command]);
      expect(help.exitCode).toBe(0);
      expect(help.stdout).not.toContain("--json");
    }
  });
});

test("113 proof without a claim names the missing matching claim", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "orphan proof", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);

    const result = await runCli(fixture, ["work", "done", id, "--proof", "a test passed"]);
    expect(result.exitCode).not.toBe(0);
    const error = (JSON.parse(result.stderr) as {
      error: { code: string; message: string; origin: string };
    }).error;
    expect(error.code).toBe("GATE_BLOCKED");
    expect(error.origin).toBe("policy-proof");
    expect(error.message).toContain("--proof requires a matching --claim");
    expect(error.message).not.toContain("each claim requires");
  });
});

test("114 decision help shows draft usage and edits name the previous title", async () => {
  await withFixture(async (fixture) => {
    const help = await runCli(fixture, ["help", "decision"]);
    expect(help.exitCode).toBe(0);
    expect(help.stdout).toContain("usage: maestro decision draft <text-or-id> [replacement]");

    const created = await runCli(fixture, ["decision", "draft", "initial direction"]);
    expect(created.exitCode).toBe(0);
    const id = created.stdout.match(/\bd\d+\b/)?.[0] ?? "";
    const edited = await runCli(fixture, ["decision", "draft", id, "revised direction"]);
    expect(edited.exitCode).toBe(0);
    expect(edited.stdout).toContain("revised direction");
    expect(edited.stdout).toContain("previous: initial direction");
  });
});

test("115 msg read renders an explicit empty state", async () => {
  await withFixture(async (fixture) => {
    const empty = await runCli(fixture, ["msg", "read"]);
    expect(empty.exitCode).toBe(0);
    expect(empty.stdout.trim()).toBe("no new messages");
  });
});

test("116 repository install from the installed runtime avoids machine-global writes", async () => {
  await withFixture(async (fixture) => {
    const { path, shim } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const runtimeRoot = join(fixture.home, ".maestro", "runtime");
    const runtimeCli = join(runtimeRoot, "bin", "maestro.ts");
    const stamp = join(runtimeRoot, ".maestro-install.json");
    const secondRepo = join(fixture.root, "second-repo");
    await mkdir(secondRepo, { recursive: true });
    const stampBefore = await readFile(stamp, "utf8");
    const shimBefore = await readFile(shim, "utf8");

    await chmod(stamp, 0o444);
    await chmod(shim, 0o444);
    try {
      const child = Bun.spawn([process.execPath, runtimeCli, "install"], {
        cwd: secondRepo,
        env: {
          ...process.env,
          HOME: fixture.home,
          PATH: path,
          MAESTRO_SESSION_ID: "installed-runtime-test",
          MAESTRO_SESSION_PID: String(process.pid),
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [stdout, stderr, exitCode] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
        child.exited,
      ]);

      expect(exitCode).toBe(0);
      expect(stderr).toBe("");
      expect(stdout).toContain("maestro installed for");
      expect(await readFile(stamp, "utf8")).toBe(stampBefore);
      expect(await readFile(shim, "utf8")).toBe(shimBefore);
      expect(await readFile(join(secondRepo, "AGENTS.md"), "utf8")).toContain("maestro:begin");
    } finally {
      if (await Bun.file(stamp).exists()) await chmod(stamp, 0o644);
      if (await Bun.file(shim).exists()) await chmod(shim, 0o755);
    }
  });
});

test("117 ready distinguishes terminal and blocked ledgers from an empty ledger", async () => {
  await withFixture(async (fixture) => {
    const done = idFrom(await runCli(fixture, ["work", "add", "finished", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", done])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", done, "--evidence", "finished"])).exitCode).toBe(0);

    const doneOnly = await runCli(fixture, ["ready"]);
    expect(doneOnly.exitCode).toBe(0);
    expect(doneOnly.stdout).toContain("no ready work; all tracked work is closed");
    expect(doneOnly.stdout).not.toContain("work add");

    const cancelled = idFrom(
      await runCli(fixture, ["work", "add", "abandoned", "--kind", "idea"]),
    );
    expect(
      (await runCli(fixture, ["work", "cancel", cancelled, "--reason", "obsolete"])).exitCode,
    ).toBe(0);
    const mixedTerminal = await runCli(fixture, ["ready"]);
    expect(mixedTerminal.stdout).toContain("no ready work; all tracked work is closed");
    expect(mixedTerminal.stdout).not.toContain("work add");

    const peer = {
      MAESTRO_SESSION_ID: "other-holder",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const active = idFrom(
      await runCli(fixture, ["work", "add", "held elsewhere", "--kind", "idea"], peer),
    );
    expect((await runCli(fixture, ["work", "start", active], peer)).exitCode).toBe(0);
    const blocked = await runCli(fixture, ["ready"]);
    expect(blocked.stdout).toContain("no ready work; inspect tracked work: maestro work list");
    expect(blocked.stdout).not.toContain("work add");
  });
});

test("118 explicit blockers prevent start and stay visible in ready", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]),
    );
    const tests = idFrom(
      await runCli(fixture, ["work", "add", "red tests", "--kind", "task", "--parent", parent]),
    );
    const implementation = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "implementation",
        "--kind",
        "implement",
        "--parent",
        parent,
        "--blocked-by",
        tests,
      ]),
    );
    const verification = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "verification",
        "--kind",
        "task",
        "--parent",
        parent,
        "--blocked-by",
        implementation,
      ]),
    );

    const blockedStart = await runCli(fixture, ["work", "start", implementation]);
    expect(blockedStart.exitCode).not.toBe(0);
    const startError = (JSON.parse(blockedStart.stderr) as {
      error: { blockers: string[]; code: string; command: string; message: string };
    }).error;
    expect(startError.code).toBe("BLOCKED");
    expect(startError.blockers).toEqual([tests]);
    expect(startError.command).toBe(`maestro work start ${tests}`);
    expect(startError.message).toContain(tests);

    const ready = await runCli(fixture, ["ready", "--json"]);
    expect(ready.exitCode).toBe(0);
    const data = (JSON.parse(ready.stdout) as {
      data: {
        gated: Array<{
          blockers?: Array<{ id: string; state: string }>;
          command?: string;
          id: string;
          origin: string;
        }>;
        works: Array<{ id: string }>;
      };
    }).data;
    expect(data.works.map((work) => work.id)).toEqual([tests]);
    expect(data.gated.map((work) => work.id)).toEqual([parent, implementation, verification]);
    const gatedImplementation = data.gated.find((work) => work.id === implementation);
    expect(gatedImplementation?.origin).toBe("work-blockers");
    expect(gatedImplementation?.blockers).toEqual([{ id: tests, state: "open" }]);
    expect(gatedImplementation?.command).toBe(`maestro work start ${tests}`);
    const gatedVerification = data.gated.find((work) => work.id === verification);
    expect(gatedVerification?.blockers).toEqual([{ id: implementation, state: "open" }]);
    expect(gatedVerification?.command).toBe("maestro ready");

    const human = await runCli(fixture, ["ready"]);
    expect(human.stdout).toContain(`${implementation} implementation [gated by work-blockers:`);
    expect(human.stdout).toContain(`${verification} verification [gated by work-blockers:`);
    const trace = await runCli(fixture, ["trace", implementation]);
    expect(trace.stdout).not.toContain("work.start");
  });
});

test("119 ready tells the current holder to finish an active child", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]),
    );
    const tests = idFrom(
      await runCli(fixture, ["work", "add", "red tests", "--kind", "task", "--parent", parent]),
    );
    const implementation = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "implementation",
        "--kind",
        "implement",
        "--parent",
        parent,
        "--blocked-by",
        tests,
      ]),
    );
    const verification = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "verification",
        "--kind",
        "task",
        "--parent",
        parent,
        "--blocked-by",
        implementation,
      ]),
    );
    expect((await runCli(fixture, ["work", "start", tests])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", tests, "--evidence", "red tests"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "start", implementation])).exitCode).toBe(0);

    const ready = await runCli(fixture, ["ready"]);
    expect(ready.exitCode).toBe(0);
    expect(ready.stdout).toContain(`maestro work done ${implementation}`);
    expect(ready.stdout).not.toContain(`maestro work start ${implementation}`);

    const json = await runCli(fixture, ["ready", "--json"]);
    const gated = (JSON.parse(json.stdout) as {
      data: { gated: Array<{ command?: string; id: string; reason: string }> };
    }).data.gated;
    expect(gated.find((work) => work.id === parent)?.reason).toContain(
      `maestro work done ${implementation}`,
    );
    expect(gated.find((work) => work.id === verification)?.command).toBe(
      `maestro work done ${implementation}`,
    );
  });
});
