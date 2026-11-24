import { expect, test } from "bun:test";
import { chmod, mkdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { hostEnvironment, idFrom, prepareInstallFixture, runCli, withFixture, workEvents } from "./helpers.ts";

test("91 an empty work list points at work add", async () => {
  await withFixture(async (fixture) => {
    const empty = await runCli(fixture, ["work", "list"]);
    expect(empty.exitCode).toBe(0);
    expect(empty.stdout).toContain("no tracked work");
    expect(empty.stdout).toContain('maestro work add "<title>"');
  });
});

test("92 gate envelope carries origin without a duplicated reason; lease keeps command", async () => {
  await withFixture(async (fixture) => {
    const gated = idFrom(await runCli(fixture, ["work", "add", "gated root", "--kind", "feature"]));
    const blocked = await runCli(fixture, ["work", "done", gated]);
    expect(blocked.exitCode).not.toBe(0);
    const gateError = (JSON.parse(blocked.stderr) as { error: Record<string, unknown> }).error;
    expect(gateError.code).toBe("GATE_BLOCKED");
    expect(gateError.origin).toBe("policy-proof");
    expect(typeof gateError.message).toBe("string");
    expect("reason" in gateError).toBe(false);
    expect("command" in gateError).toBe(false);

    const loose = idFrom(
      await runCli(fixture, ["work", "add", "atomic item", "--kind", "task"]),
    );
    const unleased = await runCli(fixture, ["work", "reclaim", loose, "--reason", "nobody holds it"]);
    expect(unleased.exitCode).not.toBe(0);
    const leaseError = (JSON.parse(unleased.stderr) as { error: Record<string, unknown> }).error;
    expect(leaseError.code).toBe("LEASE_REQUIRED");
    expect(leaseError.command).toBe(`maestro work start ${loose}`);
  });
});

test("93 breakdown structure is visible: show lists children, list indents them and gates the parent", async () => {
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

    const listed = await runCli(fixture, ["work", "list", "--json"]);
    const works = (JSON.parse(listed.stdout).data as {
      works: Array<{ gate: { origin: string } | null; id: string }>;
    }).works;
    expect(works.filter((work) => work.gate === null).map((work) => work.id)).toEqual([child]);
    expect(works.filter((work) => work.gate !== null).map((work) => work.id)).toEqual([parent]);
    expect(works.find((work) => work.id === parent)?.gate?.origin).toBe("policy-breakdown");
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
      await runCli(fixture, ["work", "add", "grown scope", "--kind", "task"]),
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
      await runCli(fixture, ["work", "add", "atomic item", "--kind", "task"]),
    );
    expect((await runCli(fixture, ["work", "start", item])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", item, "--evidence", "done"])).exitCode).toBe(0);

    const status = await runCli(fixture, ["status"]);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain("work.done");
    expect(status.stdout).not.toContain("work.start pid");
  });
});

test("96 [lint] public docs name the stderr JSON failure envelope", async () => {
  const root = join(import.meta.dir, "..");
  const readme = await readFile(join(root, "README.md"), "utf8");
  const troubleshooting = await readFile(
    join(root, "site", "src", "content", "docs", "troubleshooting.md"),
    "utf8",
  );
  for (const text of [readme, troubleshooting]) {
    expect(text).toContain("JSON error envelope");
    expect(text).toContain("stderr");
  }
});

test("97 [lint] maestro-work names the opaque evidence form", async () => {
  const skill = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "skills", "maestro-work", "SKILL.md")).text();
  const unwrapped = skill.replaceAll(/\s+/g, " ");
  expect(unwrapped).toContain("opaque");
});

test("98 session lastEvent records what happened, not what was attempted", async () => {
  await withFixture(async (fixture) => {
    const session = { MAESTRO_SESSION_ID: "honest-session", MAESTRO_SESSION_PID: String(process.pid) };
    const lastEvent = async (): Promise<string | undefined> => {
      const status = await runCli(fixture, ["status", "--json"], session);
      expect(status.exitCode).toBe(0);
      const sessions = (JSON.parse(status.stdout) as {
        data: { sessions: Array<{ id: string; lastEvent: string }> };
      }).data.sessions;
      return sessions.find((candidate) => candidate.id === session.MAESTRO_SESSION_ID)?.lastEvent;
    };
    expect(
      (await runCli(fixture, ["hook", "record", "--event", "SessionStart"], session)).exitCode,
    ).toBe(0);

    const gated = idFrom(
      await runCli(fixture, ["work", "add", "gated root", "--kind", "feature"], session),
    );
    await runCli(fixture, ["work", "add", "open child", "--kind", "task", "--parent", gated], session);
    expect((await runCli(fixture, ["work", "start", gated], session)).exitCode).not.toBe(0);
    expect(await lastEvent()).toBe("work.add");

    const item = idFrom(
      await runCli(
        fixture,
        ["work", "add", "atomic item", "--kind", "task"],
        session,
      ),
    );
    expect((await runCli(fixture, ["work", "start", item], session)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", item], session)).exitCode).not.toBe(0);
    expect(await lastEvent()).toBe("work.start");

    expect(
      (await runCli(fixture, ["work", "done", item, "--evidence", "done"], session)).exitCode,
    ).toBe(0);
    expect(await lastEvent()).toBe("work.done");
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
        ["work", "add", "held item", "--kind", "task"],
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
      await runCli(fixture, ["work", "add", "one shot", "--kind", "task"]),
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

// d855 rule 6: a mistyped --seat-token=<value> echoed the value in the
// unknown-flag error, so the error redacts everything after the first =.
// The parser stops at the first unknown flag, so each flag gets its own run;
// the value is pinned absent from the message, the flag field and both streams.
test("102b UNKNOWN_FLAG redacts a value glued to the flag with = in the message and the flag field on stdout and stderr", async () => {
  await withFixture(async (fixture) => {
    for (const [flag, value] of [["--seat-token=", "abc123secret"], ["--foo=", "bar"]] as const) {
      const glued = await runCli(fixture, ["install", `${flag}${value}`]);
      expect({ flag, exitCode: glued.exitCode }).toEqual({ flag, exitCode: 2 });
      const error = (JSON.parse(glued.stderr) as { error: Record<string, unknown> }).error;
      expect({ flag, code: error.code }).toEqual({ flag, code: "UNKNOWN_FLAG" });
      expect(String(error.message)).toContain(flag);
      expect(String(error.message)).not.toContain(value);
      expect(error.flag).toBe(flag);
      expect(String(error.flag)).not.toContain(value);
      expect(glued.stdout).not.toContain(value);
      expect(glued.stderr).not.toContain(value);
    }
  });
});

// SEC-7 (d855): --seat-token takes no value, so `maestro install --seat-token
// <token>` made the token a positional and UNKNOWN_ARGUMENT echoed it.
test("102c UNKNOWN_ARGUMENT never echoes a positional that follows a valueless flag, naming the flag instead", async () => {
  await withFixture(async (fixture) => {
    const spaced = await runCli(fixture, ["install", "--seat-token", "sk-ant-oat01-SEC7-fixture"]);
    expect(spaced.exitCode).toBe(2);
    const error = (JSON.parse(spaced.stderr) as { error: Record<string, unknown> }).error;
    expect(error.code).toBe("UNKNOWN_ARGUMENT");
    expect(String(error.message)).toContain("--seat-token");
    expect(String(error.message)).toContain("takes no value");
    expect(error.after).toBe("--seat-token");
    expect("argument" in error).toBe(false);
    expect(spaced.stdout).not.toContain("SEC7");
    expect(spaced.stderr).not.toContain("SEC7");

    const plain = await runCli(fixture, ["install", "stray"]);
    expect(plain.exitCode).toBe(2);
    const plainError = (JSON.parse(plain.stderr) as { error: Record<string, unknown> }).error;
    expect(plainError.code).toBe("UNKNOWN_ARGUMENT");
    expect(plainError.argument).toBe("stray");
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
    expect(requiresFor("policy-breakdown")).toContain("open write-like children");

    const text = await runCli(fixture, ["plugin", "list"]);
    expect(text.stdout).toContain("open write-like children");
    expect(text.stdout).toContain("--evidence");
  });
});

test("107 install reports harness wiring and the SLP v2 mirror removal", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    expect(installed.stdout).toContain(".claude/hooks/");
    expect(installed.stdout).toContain(".codex/hooks/");
    expect(installed.stdout).toContain("AGENTS.md");
    expect(installed.stdout).toContain("CLAUDE.md");
    expect(installed.stdout).toContain("removed only the legacy managed block");
    expect(await Bun.file(join(fixture.repo, "AGENTS.md")).exists()).toBe(false);
    expect(await Bun.file(join(fixture.repo, "CLAUDE.md")).exists()).toBe(false);
  });
});

test("108 help documents the kind vocabulary and the gates work list shows", async () => {
  await withFixture(async (fixture) => {
    const help = await runCli(fixture, ["help", "work"]);
    expect(help.exitCode).toBe(0);
    expect(help.stdout).toContain("feature|task|bug|chore|implement|idea|research");
    expect(help.stdout).toContain("List open and active work with holders, blockers and gates");
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

test("112 help documents --json on every command, read and write alike", async () => {
  await withFixture(async (fixture) => {
    const supported = [
      ["decision", "lock"],
      ["status"],
      ["search"],
      ["work", "list"],
      ["work", "show"],
      ["decision", "list"],
      ["recipe", "list"],
      ["plugin", "list"],
    ];
    for (const command of supported) {
      const help = await runCli(fixture, ["help", ...command]);
      expect(help.exitCode).toBe(0);
      expect(help.stdout).toContain("--json");
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

test("114 decision help shows draft usage and an edit persists the revised title", async () => {
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
    const listed = await runCli(fixture, ["decision", "list", "--json"]);
    expect(listed.exitCode).toBe(0);
    const decisions = (JSON.parse(listed.stdout) as {
      data: { decisions: Array<{ id: string; text: string }> };
    }).data.decisions;
    expect(decisions.find((decision) => decision.id === id)?.text).toBe("revised direction");
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
          ...hostEnvironment(),
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
      expect(await Bun.file(join(secondRepo, "AGENTS.md")).exists()).toBe(false);
      expect(await Bun.file(join(secondRepo, "CLAUDE.md")).exists()).toBe(false);
      expect(await Bun.file(join(secondRepo, ".claude", "hooks", "maestro-record.ts")).exists())
        .toBe(true);
    } finally {
      if (await Bun.file(stamp).exists()) await chmod(stamp, 0o644);
      if (await Bun.file(shim).exists()) await chmod(shim, 0o755);
    }
  });
});

test("117 work list tells a closed ledger and work held elsewhere apart from an empty one", async () => {
  await withFixture(async (fixture) => {
    const done = idFrom(await runCli(fixture, ["work", "add", "finished", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", done])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", done, "--evidence", "finished"])).exitCode).toBe(0);

    const doneOnly = await runCli(fixture, ["work", "list"]);
    expect(doneOnly.exitCode).toBe(0);
    expect(doneOnly.stdout).toBe("1 done not shown; maestro work list --all\n");

    const cancelled = idFrom(
      await runCli(fixture, ["work", "add", "abandoned", "--kind", "idea"]),
    );
    expect(
      (await runCli(fixture, ["work", "cancel", cancelled, "--reason", "obsolete"])).exitCode,
    ).toBe(0);
    const mixedTerminal = await runCli(fixture, ["work", "list"]);
    expect(mixedTerminal.stdout).toBe("1 done, 1 cancelled not shown; maestro work list --all\n");

    const peer = {
      MAESTRO_SESSION_ID: "other-holder",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const active = idFrom(
      await runCli(fixture, ["work", "add", "held elsewhere", "--kind", "idea"], peer),
    );
    expect((await runCli(fixture, ["work", "start", active], peer)).exitCode).toBe(0);
    const held = await runCli(fixture, ["work", "list"]);
    expect(held.stdout).toContain(`${active} [active] held elsewhere  held by other-holder (live)`);
    expect(held.stdout).not.toContain("work add");
  });
});

test("118 explicit blockers prevent start and stay visible in work list", async () => {
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

    const verificationStart = await runCli(fixture, ["work", "start", verification]);
    const verificationError = (JSON.parse(verificationStart.stderr) as {
      error: { blockers: string[]; code: string; command: string };
    }).error;
    expect(verificationError.code).toBe("BLOCKED");
    expect(verificationError.blockers).toEqual([implementation]);
    expect(verificationError.command).toBe("maestro work list");

    const listed = await runCli(fixture, ["work", "list", "--json"]);
    expect(listed.exitCode).toBe(0);
    const works = (JSON.parse(listed.stdout) as {
      data: {
        works: Array<{
          blockedBy: Array<{ id: string; state: string }>;
          gate: { origin: string } | null;
          id: string;
        }>;
      };
    }).data.works;
    const startable = works.filter((work) => work.blockedBy.length === 0 && work.gate === null);
    expect(startable.map((work) => work.id)).toEqual([tests]);
    expect(works.find((work) => work.id === parent)?.gate?.origin).toBe("policy-breakdown");
    expect(works.find((work) => work.id === implementation)?.blockedBy)
      .toEqual([{ id: tests, state: "open" }]);
    expect(works.find((work) => work.id === verification)?.blockedBy)
      .toEqual([{ id: implementation, state: "open" }]);

    const human = await runCli(fixture, ["work", "list"]);
    expect(human.stdout).toContain(`${implementation} [open] implementation  blocked by ${tests} [open]`);
    expect(human.stdout).toContain(`${verification} [open] verification  blocked by ${implementation} [open]`);
    expect(workEvents(fixture, implementation)).not.toContain("work.start");
  });
});

test("119 work list and work start tell the current holder to finish an active child", async () => {
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

    const listed = await runCli(fixture, ["work", "list"]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain(`maestro work done ${implementation}`);
    expect(listed.stdout).not.toContain(`maestro work start ${implementation}`);

    const json = await runCli(fixture, ["work", "list", "--json"]);
    const works = (JSON.parse(json.stdout) as {
      data: { works: Array<{ gate: { reason: string } | null; id: string }> };
    }).data.works;
    expect(works.find((work) => work.id === parent)?.gate?.reason).toContain(
      `maestro work done ${implementation}`,
    );
    const verificationStart = await runCli(fixture, ["work", "start", verification]);
    expect((JSON.parse(verificationStart.stderr) as { error: { command: string } }).error.command)
      .toBe(`maestro work done ${implementation}`);
  });
});

test("120 policy-breakdown exposes structured child blockers and one recovery command", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]),
    );
    const child = idFrom(
      await runCli(fixture, ["work", "add", "active child", "--kind", "task", "--parent", parent]),
    );
    expect((await runCli(fixture, ["work", "start", child])).exitCode).toBe(0);

    const listed = await runCli(fixture, ["work", "list", "--json"]);
    expect(listed.exitCode).toBe(0);
    const parentGate = (JSON.parse(listed.stdout) as {
      data: {
        works: Array<{ gate: { command: string | null; origin: string } | null; id: string }>;
      };
    }).data.works.find((work) => work.id === parent)?.gate;
    expect(parentGate?.origin).toBe("policy-breakdown");
    expect(parentGate?.command).toBe(`maestro work done ${child}`);

    const blocked = await runCli(fixture, ["work", "start", parent]);
    expect(blocked.exitCode).not.toBe(0);
    const error = (JSON.parse(blocked.stderr) as {
      error: {
        blockers: Array<{ id: string; state: string }>;
        code: string;
        command: string;
        origin: string;
      };
    }).error;
    expect(error.code).toBe("GATE_BLOCKED");
    expect(error.origin).toBe("policy-breakdown");
    expect(error.blockers).toEqual([{ id: child, state: "active" }]);
    expect(error.command).toBe(`maestro work done ${child}`);
  });
});

test("121 work show preserves declared blockers after they resolve", async () => {
  await withFixture(async (fixture) => {
    const blocker = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "red tests",
        "--kind",
        "task",
      ]),
    );
    const implementation = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "implementation",
        "--kind",
        "implement",
        "--blocked-by",
        blocker,
      ]),
    );

    const open = await runCli(fixture, ["work", "show", implementation, "--json"]);
    expect(open.exitCode).toBe(0);
    const openBlockers = (JSON.parse(open.stdout) as {
      data: { blockers: Array<{ id: string; state: string; title: string }> };
    }).data.blockers;
    expect(openBlockers).toEqual([
      expect.objectContaining({ id: blocker, state: "open", title: "red tests" }),
    ]);

    expect((await runCli(fixture, ["work", "start", blocker])).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "done", blocker, "--evidence", "red tests"])).exitCode,
    ).toBe(0);
    const resolved = await runCli(fixture, ["work", "show", implementation, "--json"]);
    expect(resolved.exitCode).toBe(0);
    const resolvedBlockers = (JSON.parse(resolved.stdout) as {
      data: { blockers: Array<{ id: string; state: string; title: string }> };
    }).data.blockers;
    expect(resolvedBlockers).toEqual([
      expect.objectContaining({ id: blocker, state: "done", title: "red tests" }),
    ]);

    const rendered = await runCli(fixture, ["work", "show", implementation]);
    expect(rendered.stdout).toContain(`blocker: ${blocker} [done] red tests`);
  });
});
