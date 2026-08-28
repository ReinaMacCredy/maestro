import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

function dispatchIdOf(result: { stdout: string }): string {
  const match = result.stdout.match(/^(x\d+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return match[1];
}

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

test("428 decision dissent and review date round-trip through draft and lock", async () => {
  await withFixture(async (fixture) => {
    const draftedReview = "2026-09-01T00:00:00.000Z";
    const lockedReview = "2027-03-01T00:00:00.000Z";
    const decision = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "adopt the reviewable boundary",
        "--dissent",
        "prefer the narrower boundary",
        "--review-at",
        draftedReview,
      ]),
    );

    const drafted = await runCli(fixture, ["decision", "show", decision]);
    expect(drafted.exitCode).toBe(0);
    expect(drafted.stdout).toContain("dissent: prefer the narrower boundary");
    expect(drafted.stdout).toContain(`review at: ${draftedReview}`);

    const locked = await runCli(fixture, [
      "decision",
      "lock",
      decision,
      "--dissent",
      "retain the narrower alternative as dissent",
      "--review-at",
      lockedReview,
    ]);
    expect(locked.exitCode).toBe(0);
    expect(locked.stdout).toContain("dissent: retain the narrower alternative as dissent");
    expect(locked.stdout).toContain(`review at: ${lockedReview}`);

    const shown = await runCli(fixture, ["decision", "show", decision]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("dissent: retain the narrower alternative as dissent");
    expect(shown.stdout).toContain(`review at: ${lockedReview}`);
  });
});

test("429 decision review dates reject non-ISO values on draft and lock", async () => {
  await withFixture(async (fixture) => {
    const invalidDraft = await runCli(fixture, [
      "decision",
      "draft",
      "invalid review date",
      "--review-at",
      "next Tuesday",
    ]);
    expect(invalidDraft.exitCode).not.toBe(0);
    expect(invalidDraft.stderr).toContain('"code":"INVALID_VALUE"');
    expect(invalidDraft.stderr).toContain("--review-at must be an ISO date");

    const decision = idFrom(await runCli(fixture, ["decision", "draft", "valid draft"]));
    const invalidLock = await runCli(fixture, [
      "decision",
      "lock",
      decision,
      "--review-at",
      "not-a-date",
    ]);
    expect(invalidLock.exitCode).not.toBe(0);
    expect(invalidLock.stderr).toContain('"code":"INVALID_VALUE"');
    expect(invalidLock.stderr).toContain("--review-at must be an ISO date");
  });
});

test("451 decision withdraw preserves draft storage while every renderer reports withdrawn", async () => {
  await withFixture(async (fixture) => {
    const decision = idFrom(
      await runCli(fixture, ["decision", "draft", "duplicate council view"]),
    );

    const withdrawn = await runCli(fixture, [
      "decision",
      "withdraw",
      decision,
      "--reason",
      "duplicate of the locked reconciliation",
    ]);
    expect(withdrawn.exitCode).toBe(0);
    expect(withdrawn.stdout).toContain(`${decision} [withdrawn] duplicate council view`);
    expect(withdrawn.stdout).toContain("withdraw reason: duplicate of the locked reconciliation");

    const shown = await runCli(fixture, ["decision", "show", decision]);
    const listed = await runCli(fixture, ["decision", "list"]);
    const listedJson = await runCli(fixture, ["decision", "list", "--json"]);
    expect(shown.stdout).toContain(`${decision} [withdrawn]`);
    expect(shown.stdout).toContain("withdraw reason: duplicate of the locked reconciliation");
    expect(listed.stdout).toContain(
      `${decision} [withdrawn] duplicate council view | withdraw reason: duplicate of the locked reconciliation`,
    );
    expect({ exitCode: listedJson.exitCode, stderr: listedJson.stderr }).toEqual({
      exitCode: 0,
      stderr: "",
    });

    const listedDecision = (JSON.parse(listedJson.stdout).data.decisions as Array<{
      id: string;
      state: string;
      withdrawReason: string | null;
      withdrawnAt: string | null;
    }>).find((candidate) => candidate.id === decision);
    expect(listedDecision).toEqual(expect.objectContaining({
      state: "withdrawn",
      withdrawReason: "duplicate of the locked reconciliation",
    }));
    expect(listedDecision?.withdrawnAt).toBeString();

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    try {
      expect(database.query<{
        state: string;
        withdraw_reason: string | null;
        withdrawn_at: string | null;
      }, [string]>(
        "SELECT state, withdraw_reason, withdrawn_at FROM decisions WHERE id = ?",
      ).get(decision)).toEqual(expect.objectContaining({
        state: "draft",
        withdraw_reason: "duplicate of the locked reconciliation",
        withdrawn_at: expect.any(String),
      }));
      const event = database.query<{ payload: string }, [string]>(
        "SELECT payload FROM event_log WHERE type = 'decision.withdraw' AND entity_id = ?",
      ).get(decision);
      expect(JSON.parse(event?.payload ?? "{}")).toEqual({
        reason: "duplicate of the locked reconciliation",
      });
    } finally {
      database.close();
    }
  });
});

test("452 withdrawn decisions refuse edits, locks, supersession, and invalid withdrawal states", async () => {
  await withFixture(async (fixture) => {
    const draft = idFrom(await runCli(fixture, ["decision", "draft", "losing first view"]));
    for (const args of [
      ["decision", "withdraw", draft],
      ["decision", "withdraw", draft, "--reason", ""],
    ]) {
      const missing = await runCli(fixture, args);
      expect(missing.exitCode).not.toBe(0);
      expect(JSON.parse(missing.stderr).error).toEqual(expect.objectContaining({
        code: "MISSING_ARGUMENT",
        message: expect.stringContaining("decision withdraw requires --reason <text>; run: maestro decision withdraw"),
      }));
    }

    expect((await runCli(fixture, [
      "decision",
      "withdraw",
      draft,
      "--reason",
      "another decision won",
    ])).exitCode).toBe(0);

    const refused = [
      await runCli(fixture, ["decision", "draft", draft, "edited losing view"]),
      await runCli(fixture, ["decision", "lock", draft]),
      await runCli(fixture, [
        "decision",
        "draft",
        "replacement targeting a withdrawal",
        "--supersedes",
        draft,
      ]),
      await runCli(fixture, ["decision", "withdraw", draft, "--reason", "again"]),
    ];
    for (const result of refused) {
      expect(result.exitCode).not.toBe(0);
      expect(JSON.parse(result.stderr).error).toEqual(expect.objectContaining({
        code: "INVALID_STATE",
        message: expect.stringContaining(`${draft} is withdrawn`),
      }));
    }

    const locked = idFrom(await runCli(fixture, ["decision", "draft", "locked choice"]));
    expect((await runCli(fixture, ["decision", "lock", locked])).exitCode).toBe(0);
    const lockedWithdrawal = await runCli(fixture, [
      "decision",
      "withdraw",
      locked,
      "--reason",
      "retire it",
    ]);
    expect(JSON.parse(lockedWithdrawal.stderr).error).toEqual(expect.objectContaining({
      code: "INVALID_STATE",
      message: expect.stringContaining(
        `maestro decision draft "<replacement>" --supersedes ${locked}`,
      ),
    }));

    const replacement = idFrom(await runCli(fixture, [
      "decision",
      "draft",
      "replacement choice",
      "--supersedes",
      locked,
    ]));
    expect((await runCli(fixture, ["decision", "lock", replacement])).exitCode).toBe(0);
    const supersededWithdrawal = await runCli(fixture, [
      "decision",
      "withdraw",
      locked,
      "--reason",
      "retire it again",
    ]);
    const supersededError = JSON.parse(supersededWithdrawal.stderr).error;
    expect(supersededError).toEqual(expect.objectContaining({
      code: "INVALID_STATE",
      command: `maestro decision show ${replacement}`,
      message: expect.stringContaining(`${locked} is superseded by ${replacement}`),
      supersededById: replacement,
    }));
    expect(supersededError.message).not.toContain("--supersedes");
  });
});

test("410 decision draft warns and records the generation when its work council is sealed", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "sealed decision council", "--atomic-reason", "fixture"]),
    );
    const firstDispatch = await runCli(
      fixture,
      [...dispatchOpenArgs(work), "--council-members", "2"],
    );
    const secondDispatch = await runCli(fixture, [
      ...dispatchOpenArgs(work),
      "--council-anchor",
      dispatchIdOf(firstDispatch),
    ]);
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

test("460 decision edit refuses a lost conditional update without appending an event", async () => {
  await withFixture(async (fixture) => {
    const decision = idFrom(await runCli(fixture, ["decision", "draft", "stable draft text"]));
    const path = join(fixture.repo, ".maestro", "maestro.db");
    const database = new Database(path);
    database.run(`
      CREATE TRIGGER ignore_decision_edit
      BEFORE UPDATE OF text ON decisions
      BEGIN
        SELECT RAISE(IGNORE);
      END
    `);
    const beforeEvents = database
      .query<{ count: number }, [string]>(
        "SELECT count(*) AS count FROM event_log WHERE type = 'decision.draft' AND entity_id = ?",
      )
      .get(decision)?.count;
    database.close();

    const edited = await runCli(fixture, ["decision", "draft", decision, "racing edit"]);
    expect(edited.exitCode).toBe(1);
    expect(edited.stderr).toContain('"code":"INVALID_STATE"');

    const stored = new Database(path, { readonly: true });
    try {
      expect(
        stored.query<{ text: string }, [string]>("SELECT text FROM decisions WHERE id = ?").get(decision),
      ).toEqual({ text: "stable draft text" });
      expect(
        stored
          .query<{ count: number }, [string]>(
            "SELECT count(*) AS count FROM event_log WHERE type = 'decision.draft' AND entity_id = ?",
          )
          .get(decision)?.count,
      ).toBe(beforeEvents);
    } finally {
      stored.close();
    }
  });
});

test("461 decision edit warns and records the generation while its work council is sealed", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "sealed edit council", "--atomic-reason", "fixture"]),
    );
    const decision = idFrom(
      await runCli(fixture, ["decision", "draft", "harmless first text", "--work", work]),
    );
    const firstDispatch = await runCli(
      fixture,
      [...dispatchOpenArgs(work), "--council-members", "2"],
    );
    expect(firstDispatch.exitCode).toBe(0);
    expect(
      (await runCli(fixture, [
        ...dispatchOpenArgs(work),
        "--council-anchor",
        dispatchIdOf(firstDispatch),
      ])).exitCode,
    ).toBe(0);
    const generationAnchor = firstDispatch.stdout.match(/^(x\d+) \[open\]/)?.[1];
    expect(generationAnchor).toBeString();

    const edited = await runCli(fixture, [
      "decision",
      "draft",
      decision,
      "sensitive first view",
    ]);
    expect(edited.exitCode, edited.stderr).toBe(0);
    expect(edited.stderr).toBe(
      `[sealed] ${work} council is sealed; this draft is readable by its lanes\n`,
    );

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    try {
      const payload = database
        .query<{ payload: string }, [string]>(
          "SELECT payload FROM event_log WHERE type = 'decision.draft' AND entity_id = ? ORDER BY id DESC LIMIT 1",
        )
        .get(decision)?.payload;
      expect(JSON.parse(payload ?? "{}")).toEqual({
        edit: true,
        sealedCouncil: generationAnchor,
        text: "sensitive first view",
      });
    } finally {
      database.close();
    }
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
