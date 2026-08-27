import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

function dispatchOpenArgs(work: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Return an independent council view",
    "--owned-scope",
    "decision boundary",
    "--excluded-scope",
    "implementation",
    "--mutation",
    "no-write",
    "--stop-condition",
    "view returned",
    "--lane",
    "decision",
    "--evidence-required",
    "source: decision record",
    "--pane",
    "w1:pA",
  ];
}

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

test("417 decision draft --needs-owner round-trips an explicit owner requirement", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(await runCli(fixture, ["work", "add", "owner choice", "--kind", "idea"]));
    const marked = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "the owner must choose",
        "--needs-owner",
        "--work",
        work,
      ]),
    );
    const ordinary = idFrom(
      await runCli(fixture, ["decision", "draft", "ordinary technical choice", "--work", work]),
    );

    for (const [id, needsOwner] of [[marked, true], [ordinary, false]] as const) {
      const shown = await runCli(fixture, ["decision", "show", id]);
      expect(shown.exitCode).toBe(0);
      expect(shown.stdout.includes("needs owner: yes")).toBe(needsOwner);
    }
  });
});

test("410 decision draft warns and records the generation when its work council is sealed", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "sealed decision council", "--atomic-reason", "fixture"]),
    );
    const firstDispatch = await runCli(fixture, dispatchOpenArgs(work));
    const secondDispatch = await runCli(fixture, dispatchOpenArgs(work));
    const generationAnchor = firstDispatch.stdout.match(/^(x\d+) \[open\]/)?.[1];
    expect(firstDispatch.exitCode).toBe(0);
    expect(secondDispatch.exitCode).toBe(0);
    expect(generationAnchor).toBeString();

    const drafted = await runCli(fixture, [
      "decision",
      "draft",
      "ask the Supervisor without breaking the council seal",
      "--work",
      work,
    ]);

    expect(drafted.exitCode).toBe(0);
    expect(drafted.stderr).toBe(
      `[sealed] ${work} council is sealed; this draft is readable by its lanes\n`,
    );
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const payload = database
      .query<{ payload: string }, []>(
        "SELECT payload FROM event_log WHERE type = 'decision.draft' ORDER BY id DESC LIMIT 1",
      )
      .get()?.payload;
    database.close();
    expect(payload).toBeString();
    expect(JSON.parse(payload as string)).toEqual(
      expect.objectContaining({ sealedCouncil: generationAnchor }),
    );
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
