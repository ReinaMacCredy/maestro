import { expect, test } from "bun:test";
import { idFrom, runCli, withFixture } from "./helpers.ts";

test("16 search finds matching work, decision, and note text", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(await runCli(fixture, ["work", "add", "nebula work", "--kind", "idea"]));
    expect((await runCli(fixture, ["decision", "draft", "nebula decision"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "note", work, "nebula note"])).exitCode).toBe(0);

    const result = await runCli(fixture, ["search", "nebula"]);

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("work");
    expect(result.stdout).toContain("decision");
    expect(result.stdout).toContain("note");
  });
});

test("17 trace reconstructs ordered start, note, done, and evidence history", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(await runCli(fixture, ["work", "add", "traceable", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", work])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "note", work, "middle note"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", work, "--evidence", "final evidence"])).exitCode).toBe(0);

    const result = await runCli(fixture, ["trace", work]);

    expect(result.exitCode).toBe(0);
    const start = result.stdout.indexOf("work.start");
    const note = result.stdout.indexOf("work.note");
    const done = result.stdout.indexOf("work.done");
    expect(start).toBeGreaterThanOrEqual(0);
    expect(note).toBeGreaterThan(start);
    expect(done).toBeGreaterThan(note);
    expect(result.stdout).toContain("final evidence");
  });
});

test("18 watch --once renders the work tree and live sessions then exits", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "parent work", "--kind", "idea"]));
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child work", "--kind", "idea", "--parent", parent]),
    );
    expect(
      (
        await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
          MAESTRO_SESSION_ID: "watch-session",
          MAESTRO_SESSION_PID: String(process.pid),
        })
      ).exitCode,
    ).toBe(0);

    const result = await runCli(fixture, ["watch", "--once"]);

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain(parent);
    expect(result.stdout).toContain(child);
    expect(result.stdout).toContain("watch-session");
  });
});

test("19 locked decisions reject edits while superseding links and child visibility remain", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["decision", "draft", "parent choice"]));
    const first = idFrom(
      await runCli(fixture, ["decision", "draft", "first child", "--parent", parent]),
    );
    const second = idFrom(
      await runCli(fixture, ["decision", "draft", "second child", "--parent", parent]),
    );
    expect((await runCli(fixture, ["decision", "lock", first])).exitCode).toBe(0);

    const edit = await runCli(fixture, ["decision", "draft", first, "changed child"]);
    const supersede = await runCli(fixture, [
      "decision",
      "draft",
      "replacement child",
      "--parent",
      parent,
      "--supersedes",
      first,
    ]);
    const listed = await runCli(fixture, ["decision", "list"]);

    expect(edit.exitCode).not.toBe(0);
    expect(edit.stderr).toContain("locked");
    expect(supersede.exitCode).toBe(0);
    expect(listed.stdout).toContain(first);
    expect(listed.stdout).toContain(second);
    expect(listed.stdout).toContain("replacement child");
  });
});

test("20 gate blocks return nonzero with a structured reason on stderr", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(await runCli(fixture, ["work", "add", "gate", "--kind", "idea"]));
    expect((await runCli(fixture, ["work", "start", work])).exitCode).toBe(0);

    const result = await runCli(fixture, ["work", "done", work, "--claim", "verified"]);
    const error = JSON.parse(result.stderr.trim());

    expect(result.exitCode).not.toBe(0);
    expect(error.ok).toBe(false);
    expect(error.error.code).toBe("GATE_BLOCKED");
    expect(error.error.origin).toBe("policy-proof");
    expect(error.error.reason).toBeString();
  });
});

test("21 --json list output is one compact single-line envelope", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["work", "add", "dense", "--kind", "idea"])).exitCode).toBe(0);

    const result = await runCli(fixture, ["work", "list", "--json"]);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.trim().split("\n")).toHaveLength(1);
    expect(result.stdout.trim()).toBe(JSON.stringify(JSON.parse(result.stdout)));
    expect(JSON.parse(result.stdout).ok).toBe(true);
    expect(JSON.parse(result.stdout).data.works).toBeArray();
  });
});
