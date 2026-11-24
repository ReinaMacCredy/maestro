import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { installFakeHerdr } from "./helpers-herdr.ts";
import { runCliAt, withFixture } from "./helpers.ts";
import { envelope, phaseFree } from "./helpers-slp.ts";

test("SLP v2 records one-step immutable decisions and preserves the replaced record", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Decide the technical boundary", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = startData.team.roles.find((role) => role.role === "lead")?.paneId;
    const environment = { ...fake.env, HERDR_PANE_ID: leadPane };

    const original = await runCliAt(
      fixture,
      fixture.repo,
      [
        "decide",
        "Use the direct adapter",
        "--why",
        "It is the narrowest stable seam",
        "--work",
        startData.work.id,
        "--json",
      ],
      environment,
    );
    expect(phaseFree(original.stderr)).toBe("");
    expect(original.exitCode).toBe(0);
    const originalDecision = envelope<{
      decision: { choice: string; id: string; scope: string };
    }>(original.stdout).decision;
    expect(originalDecision).toMatchObject({
      choice: "Use the direct adapter",
      scope: "technical",
    });

    const replacement = await runCliAt(
      fixture,
      fixture.repo,
      [
        "decide",
        "Use the bounded runtime adapter",
        "--why",
        "The runtime boundary needs explicit failure handling",
        "--work",
        startData.work.id,
        "--replaces",
        originalDecision.id,
        "--json",
      ],
      environment,
    );
    expect(phaseFree(replacement.stderr)).toBe("");
    const replacementDecision = envelope<{
      decision: { id: string; replaces: string };
    }>(replacement.stdout).decision;
    expect(replacementDecision.replaces).toBe(originalDecision.id);
    expect(replacementDecision.id).not.toBe(originalDecision.id);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const rows = database
      .query<{ choice: string; id: string; replaces_id: string | null }, []>(
        "SELECT id, choice, replaces_id FROM slp_decisions ORDER BY id",
      )
      .all();
    database.close();
    expect(rows).toEqual([
      { choice: "Use the direct adapter", id: originalDecision.id, replaces_id: null },
      {
        choice: "Use the bounded runtime adapter",
        id: replacementDecision.id,
        replaces_id: originalDecision.id,
      },
    ]);
  });
}, 15_000);

test("SLP v2 serializes ten concurrent decision ids without dropping a decision", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    expect((await runCliAt(fixture, room, ["status", "--json"])).exitCode).toBe(0);

    const results = await Promise.all(
      Array.from({ length: 10 }, (_, index) =>
        runCliAt(fixture, room, [
          "decide",
          `Concurrent choice ${index}`,
          "--why",
          `Concurrent reason ${index}`,
          "--json",
        ])
      ),
    );

    expect(results.map((result) => result.exitCode)).toEqual(Array(10).fill(0));
    const ids = results.map(
      (result) => envelope<{ decision: { id: string } }>(result.stdout).decision.id,
    );
    expect(new Set(ids).size).toBe(10);
  });
}, 15_000);

test("SLP v2 Hub decisions link unique work and require team qualification when ambiguous", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const first = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>((
      await runCliAt(
        fixture,
        room,
        ["team", "start", fixture.repo, "First Hub-linked work", "--json"],
        fake.env,
      )
    ).stdout);
    const ownerDecision = envelope<{ decision: { id: string } }>((await runCliAt(
      fixture,
      room,
      ["decide", "Owner baseline", "--why", "Owner authority", "--json"],
      fake.env,
    )).stdout).decision;
    const linked = await runCliAt(
      fixture,
      room,
      [
        "decide",
        "Hub choice",
        "--why",
        "Owner authority",
        "--work",
        first.work.id,
        "--replaces",
        ownerDecision.id,
        "--json",
      ],
      fake.env,
    );
    expect(phaseFree(linked.stderr)).toBe("");
    expect(linked.exitCode).toBe(0);
    const linkedDecision = envelope<{
      decision: { id: string; replaces: string; workId: string };
    }>(linked.stdout).decision;
    expect(linkedDecision.workId).toBe(first.work.id);
    expect(linkedDecision.replaces).toBe(ownerDecision.id);
    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(
      roomDatabase
        .query<{ team_id: string; work_id: string }, [string]>(
          "SELECT team_id, work_id FROM slp_decisions WHERE id = ?",
        )
        .get(linkedDecision.id),
    ).toEqual({ team_id: first.team.teamId, work_id: first.work.id });
    roomDatabase.close();
    const firstProject = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    expect(
      firstProject
        .query<{ actor: string; work_id: string }, [string]>(
          "SELECT actor, work_id FROM slp_decisions WHERE id = ?",
        )
        .get(linkedDecision.id),
    ).toBeNull();
    firstProject.close();
    const firstLead = first.team.roles.find((role) => role.role === "lead")!;
    const linkedStatus = await runCliAt(
      fixture,
      fixture.repo,
      ["status", first.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: firstLead.paneId },
    );
    expect(
      envelope<{ decisions: Array<Record<string, unknown>> }>(linkedStatus.stdout).decisions,
    ).toContainEqual({
      actor: "hub-supervisor",
      choice: "Hub choice",
      createdAt: expect.any(String),
      id: linkedDecision.id,
      replaces: ownerDecision.id,
      scope: "owner",
      why: "Owner authority",
    });

    const secondRepo = join(fixture.root, "repo-two");
    await mkdir(join(secondRepo, ".maestro", "plugins"), { recursive: true });
    const second = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>((
      await runCliAt(
        fixture,
        room,
        ["team", "start", secondRepo, "Second Hub-linked work", "--json"],
        fake.env,
      )
    ).stdout);
    expect(second.work.id).toBe(first.work.id);
    const ambiguous = await runCliAt(
      fixture,
      room,
      ["decide", "Ambiguous choice", "--why", "Must identify the team", "--work", "w1", "--json"],
      fake.env,
    );
    expect(ambiguous.exitCode).toBe(1);
    expect((JSON.parse(ambiguous.stderr) as { error: { code: string } }).error.code)
      .toBe("AMBIGUOUS_WORK");
    const qualified = await runCliAt(
      fixture,
      room,
      [
        "decide",
        "Second team choice",
        "--why",
        "Qualified owner authority",
        "--work",
        `${second.team.teamId}:${second.work.id}`,
        "--json",
      ],
      fake.env,
    );
    expect(phaseFree(qualified.stderr)).toBe("");
    expect(qualified.exitCode).toBe(0);
    const secondProject = new Database(join(secondRepo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      secondProject
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_decisions WHERE actor = 'hub-supervisor' AND work_id = 'w1'",
        )
        .get()?.count,
    ).toBe(0);
    secondProject.close();
    const secondDecision = envelope<{ decision: { id: string } }>(qualified.stdout).decision;
    const secondLead = second.team.roles.find((role) => role.role === "lead")!;
    const secondStatus = await runCliAt(
      fixture,
      secondRepo,
      ["status", second.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: secondLead.paneId },
    );
    expect(envelope<{ decisions: Array<{ id: string }> }>(secondStatus.stdout).decisions)
      .toContainEqual(expect.objectContaining({ id: secondDecision.id }));
    expect((await runCliAt(
      fixture,
      room,
      ["team", "stop", first.team.teamId, "--emergency", "--json"],
      fake.env,
    )).exitCode).toBe(0);
    const stoppedTarget = await runCliAt(
      fixture,
      room,
      [
        "decide",
        "Must not link stopped work",
        "--why",
        "Only running generations are eligible",
        "--work",
        `${first.team.teamId}:${first.work.id}`,
        "--json",
      ],
      fake.env,
    );
    expect(stoppedTarget.exitCode).toBe(1);
    // w736 F3: still refused, but as a state and not as an absence - the row is
    // there and the Hub can read it; what it cannot do is write to a generation
    // whose seats are gone.
    const refusal = JSON.parse(stoppedTarget.stderr).error as { code: string; message: string };
    expect(refusal.code).toBe("NO_ACTIVE_TEAM");
    expect(refusal.message).toContain("STOPPED");
    expect(refusal.message).toContain(first.team.teamId);
    const readBack = await runCliAt(
      fixture,
      room,
      ["status", `${first.team.teamId}:${first.work.id}`],
      fake.env,
    );
    expect(readBack.exitCode).toBe(0);
    expect(readBack.stdout).toContain(first.work.id);
  });
}, 25_000);

// w697: decisions were recorded with no way back to them. A work-linked
// decision could be reached only through `status <work-id> --json`, an
// unlinked one not at all, and the retired `decision show` pointed the reader
// at `decide`, a write verb. `status <decision-id>` is that read path; the
// nine-operation surface gains no verb.
test("SLP v2 status <decision-id> returns a decision by its own id, linked or not, and refuses an unknown id", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Read decisions back", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };

    const linked = envelope<{ decision: { id: string } }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          [
            "decide",
            "Cache the rendered profile per digest",
            "--why",
            "rendering twice cost more than the cache",
            "--work",
            data.work.id,
            "--json",
          ],
          leadEnvironment,
        )
      ).stdout,
    ).decision;
    const unlinked = envelope<{ decision: { id: string } }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["decide", "Keep the store the truth", "--why", "panes lose lines", "--json"],
          leadEnvironment,
        )
      ).stdout,
    ).decision;

    for (
      const expected of [
        {
          actor: lead.paneId ? expect.stringMatching(/^lead-/) : "",
          choice: "Cache the rendered profile per digest",
          id: linked.id,
          scope: "technical",
          store: "project",
          why: "rendering twice cost more than the cache",
          workId: data.work.id,
        },
        {
          actor: expect.stringMatching(/^lead-/),
          choice: "Keep the store the truth",
          id: unlinked.id,
          scope: "technical",
          store: "project",
          why: "panes lose lines",
          workId: null,
        },
      ]
    ) {
      const read = await runCliAt(
        fixture,
        fixture.repo,
        ["status", expected.id, "--json"],
        leadEnvironment,
      );
      expect(phaseFree(read.stderr)).toBe("");
      expect(read.exitCode).toBe(0);
      expect(envelope<{ decisions: Array<Record<string, unknown>> }>(read.stdout).decisions)
        .toEqual([expect.objectContaining(expected)]);
    }

    // The plain-text view carries the content, not just the id.
    const text = await runCliAt(fixture, fixture.repo, ["status", linked.id], leadEnvironment);
    expect(text.exitCode).toBe(0);
    expect(text.stdout).toContain(`${linked.id} [technical]`);
    expect(text.stdout).toContain("choice: Cache the rendered profile per digest");
    expect(text.stdout).toContain("why: rendering twice cost more than the cache");

    // A Peer reads team doctrine too.
    const peerWork = envelope<{ role: { paneId: string } }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "add", "Peer reads doctrine", "--to", "reader", "--json"],
          leadEnvironment,
        )
      ).stdout,
    );
    const peerRead = await runCliAt(
      fixture,
      fixture.repo,
      ["status", unlinked.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: peerWork.role.paneId },
    );
    expect(peerRead.exitCode).toBe(0);

    const missing = await runCliAt(fixture, fixture.repo, ["status", "d987", "--json"], leadEnvironment);
    expect(missing.exitCode).toBe(1);
    expect((JSON.parse(missing.stderr) as { error: { code: string } }).error.code).toBe("NOT_FOUND");
  });
}, 20_000);

// w697: search hit the legacy `decisions` ledger only, so no decision recorded
// by `maestro decide` was findable by id or by text.
test("SLP v2 search matches an slp_decisions row by its id and by text from choice and why", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Find decisions again", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{ team: { roles: Array<{ paneId: string; role: string }> } }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const decision = envelope<{ decision: { id: string } }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          [
            "decide",
            "Pin the runtime to surface-tourmaline",
            "--why",
            "the alternative surface-zircon drifts per machine",
            "--json",
          ],
          leadEnvironment,
        )
      ).stdout,
    ).decision;

    for (const term of [decision.id, "surface-tourmaline", "surface-zircon"]) {
      const found = await runCliAt(
        fixture,
        fixture.repo,
        ["search", term, "--local", "--json"],
        leadEnvironment,
      );
      expect(found.exitCode).toBe(0);
      expect(
        envelope<{ matches: Array<{ id: string; kind: string }> }>(found.stdout).matches,
      ).toContainEqual(expect.objectContaining({ id: decision.id, kind: "decision" }));
    }
  });
}, 20_000);

// w697: an owner-scope ruling has no work id and lives in the Hub store, so
// the one working read path could not reach it and the Hub had to paste it by
// hand into the pane.
test("SLP v2 status <decision-id> reaches an owner-scope decision recorded in the Hub store", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Reach owner rulings", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{ team: { roles: Array<{ paneId: string; role: string }> } }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const owner = envelope<{ decision: { id: string; scope: string } }>(
      (
        await runCliAt(
          fixture,
          room,
          [
            "decide",
            "Do not resync the runtime while another team is live",
            "--why",
            "a mid-flight resync breaks a live team's panes",
            "--json",
          ],
          fake.env,
        )
      ).stdout,
    ).decision;
    expect(owner.scope).toBe("owner");

    const read = await runCliAt(fixture, fixture.repo, ["status", owner.id, "--json"], leadEnvironment);

    expect(phaseFree(read.stderr)).toBe("");
    expect(read.exitCode).toBe(0);
    expect(envelope<{ decisions: Array<Record<string, unknown>> }>(read.stdout).decisions).toEqual([
      expect.objectContaining({
        actor: "hub-supervisor",
        choice: "Do not resync the runtime while another team is live",
        id: owner.id,
        scope: "owner",
        store: "hub",
        why: "a mid-flight resync breaks a live team's panes",
        workId: null,
      }),
    ]);
  });
}, 20_000);
