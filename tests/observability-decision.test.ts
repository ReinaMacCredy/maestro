import { expect, test } from "bun:test";
import { idFrom, runCli, withFixture } from "./helpers.ts";

test("16 search JSON binds unique work, decision, and note tokens to explicit results", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "work-surface-ember", "--kind", "idea"]),
    );
    const decision = idFrom(
      await runCli(fixture, ["decision", "draft", "decision-surface-cobalt"]),
    );
    expect((await runCli(fixture, ["work", "note", work, "note-surface-violet"])).exitCode)
      .toBe(0);

    for (const [token, expected] of [
      ["work-surface-ember", { id: work, kind: "idea" }],
      ["decision-surface-cobalt", { id: decision, kind: "decision" }],
      ["note-surface-violet", { id: work, kind: "idea" }],
    ] satisfies Array<[string, { id: string; kind: string }]>) {
      const result = await runCli(fixture, ["search", token, "--json"]);
      expect(result.exitCode).toBe(0);
      const matches = (JSON.parse(result.stdout) as {
        data: { matches: Array<{ id: string; kind: string }> };
      }).data.matches;
      expect(matches).toContainEqual(expect.objectContaining(expected));
    }
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

test("298 a replacement supersedes its predecessor only when the replacement locks", async () => {
  await withFixture(async (fixture) => {
    const predecessor = idFrom(
      await runCli(fixture, ["decision", "draft", "binding strategy A"]),
    );
    expect((await runCli(fixture, ["decision", "lock", predecessor])).exitCode).toBe(0);
    const firstReplacement = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "candidate strategy B",
        "--supersedes",
        predecessor,
      ]),
    );
    const secondReplacement = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "candidate strategy C",
        "--supersedes",
        predecessor,
      ]),
    );

    const beforeLockResult = await runCli(fixture, ["decision", "show", predecessor]);
    expect(beforeLockResult.exitCode).toBe(0);
    expect(beforeLockResult.stdout).toContain(`${predecessor} [locked]`);
    expect(beforeLockResult.stdout).not.toContain("superseded by:");

    expect((await runCli(fixture, ["decision", "lock", firstReplacement])).exitCode).toBe(0);
    const afterLockResult = await runCli(fixture, ["decision", "show", predecessor]);
    expect(afterLockResult.exitCode).toBe(0);
    expect(afterLockResult.stdout).toContain(`${predecessor} [superseded]`);
    expect(afterLockResult.stdout).toContain(`superseded by: ${firstReplacement}`);

    const conflict = await runCli(fixture, ["decision", "lock", secondReplacement]);
    expect(conflict.exitCode).not.toBe(0);
    expect(conflict.stderr).toContain("SUPERSESSION_CONFLICT");
    expect(conflict.stderr).toContain(firstReplacement);
    const losingDraftResult = await runCli(fixture, [
      "decision",
      "show",
      secondReplacement,
    ]);
    expect(losingDraftResult.exitCode).toBe(0);
    expect(losingDraftResult.stdout).toContain(`${secondReplacement} [draft]`);
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
    expect(error.error.message).toBeString();
    expect("reason" in error.error).toBe(false);
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
