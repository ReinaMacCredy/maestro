import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

function session(id: string, pid = process.pid): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(pid) };
}

async function add(fixture: Fixture, title: string, extra: string[] = []): Promise<string> {
  return idFrom(await runCli(fixture, ["work", "add", title, ...extra]));
}

interface Seeded {
  chore: string;
  dropped: string;
  offByOne: string;
  parent: string;
  reader: string;
  waiting: string;
}

async function seed(fixture: Fixture): Promise<Seeded> {
  const parent = await add(fixture, "parser rewrite", ["--kind", "feature"]);
  const reader = await add(fixture, "split the reader", ["--kind", "task", "--parent", parent]);
  const offByOne = await add(fixture, "fix the off-by-one", ["--kind", "bug", "--parent", parent]);
  const waiting = await add(fixture, "wait for the reader", [
    "--kind", "task", "--parent", parent, "--blocked-by", reader, "--blocked-by", offByOne,
  ]);
  const chore = await add(fixture, "old chore", ["--kind", "idea"]);
  const dropped = await add(fixture, "dropped idea", ["--kind", "idea"]);
  expect((await runCli(fixture, ["work", "start", reader], session("holder-live"))).exitCode).toBe(0);
  expect((await runCli(fixture, ["work", "done", chore, "--evidence", "finished"])).exitCode).toBe(0);
  expect((await runCli(fixture, ["work", "cancel", dropped, "--reason", "not needed"])).exitCode).toBe(0);
  return { parent, reader, offByOne, waiting, chore, dropped };
}

test("work list prints open and active items in tree order with holders, blockers, gates and a count line (d893, d896)", async () => {
  await withFixture(async (fixture) => {
    const { parent, reader, offByOne, waiting } = await seed(fixture);

    const listed = await runCli(fixture, ["work", "list"]);
    expect(listed.exitCode).toBe(0);
    const lines = listed.stdout.split("\n");
    expect(lines[1]).toStartWith(
      `  gated by policy-breakdown: ${parent} has open children: ${reader}, ${offByOne}, ${waiting}; finish them first: `,
    );
    expect([lines[0], ...lines.slice(2)]).toEqual([
      `${parent} [open] parser rewrite`,
      `  ${reader} [active] split the reader  held by holder-live (live)`,
      `  ${offByOne} [open] fix the off-by-one`,
      `  ${waiting} [open] wait for the reader  blocked by ${reader} [active], ${offByOne} [open]`,
      "1 done, 1 cancelled not shown; maestro work list --all",
      "",
    ]);
  });
});

test("work list --all lists every item with the same detail and no count line (d893)", async () => {
  await withFixture(async (fixture) => {
    const { parent, reader, waiting, chore, dropped } = await seed(fixture);

    const listed = await runCli(fixture, ["work", "list", "--all"]);
    expect(listed.exitCode).toBe(0);
    const lines = listed.stdout.trimEnd().split("\n");
    expect(lines).toContain(`${chore} [done] old chore`);
    expect(lines).toContain(`${dropped} [cancelled] dropped idea`);
    expect(lines).toContain(`  ${reader} [active] split the reader  held by holder-live (live)`);
    expect(lines.some((line) => line.startsWith(`  ${waiting} [open] wait for the reader  blocked by`))).toBe(true);
    expect(lines[1]).toStartWith(`  gated by policy-breakdown: ${parent} has open children`);
    expect(listed.stdout).not.toContain("not shown");
  });
});

test("work list --parent lists the children of one item in every state (d893)", async () => {
  await withFixture(async (fixture) => {
    const { parent, reader, offByOne, waiting } = await seed(fixture);
    expect((await runCli(fixture, ["work", "cancel", offByOne, "--reason", "folded in"])).exitCode).toBe(0);

    const listed = await runCli(fixture, ["work", "list", "--parent", parent]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toBe([
      `${reader} [active] split the reader  held by holder-live (live)`,
      `${offByOne} [cancelled] fix the off-by-one`,
      `${waiting} [open] wait for the reader  blocked by ${reader} [active]`,
      "",
    ].join("\n"));

    const childless = await runCli(fixture, ["work", "list", "--parent", reader]);
    expect(childless.stdout).toBe(`${reader} has no children; add one: maestro work add "<title>" --parent ${reader}\n`);

    const missing = await runCli(fixture, ["work", "list", "--parent", "w99"]);
    expect(missing.exitCode).not.toBe(0);
    expect(missing.stderr).toContain("NOT_FOUND");
    expect(missing.stderr).toContain("maestro work list");
  });
});

test("work list names a dead holder without expiring its lease, and only the count line when nothing is open (d896)", async () => {
  await withFixture(async (fixture) => {
    const host = Bun.spawn(["sleep", "30"], { stderr: "ignore", stdout: "ignore" });
    try {
      const held = await add(fixture, "held by a session that dies", ["--kind", "idea"]);
      expect((await runCli(fixture, ["work", "start", held], session("holder-dead", host.pid))).exitCode)
        .toBe(0);
      host.kill();
      await host.exited;

      const listed = await runCli(fixture, ["work", "list"], session("reader"));
      expect(listed.stdout).toBe(`${held} [active] held by a session that dies  held by holder-dead (dead)\n`);
      const json = JSON.parse((await runCli(fixture, ["work", "list", "--json"], session("reader"))).stdout) as {
        data: { works: Array<{ holder: Record<string, unknown> | null }> };
      };
      expect(json.data.works[0]?.holder).toEqual(expect.objectContaining({ id: "holder-dead", live: false }));

      const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
      try {
        expect(database.query("SELECT state, held_by FROM work WHERE id = ?").get(held))
          .toEqual({ state: "active", held_by: "holder-dead" });
      } finally {
        database.close();
      }

      expect((await runCli(fixture, ["work", "done", held, "--evidence", "taken over"], session("reader"))).exitCode)
        .toBe(0);
      expect((await runCli(fixture, ["work", "list"])).stdout).toBe("1 done not shown; maestro work list --all\n");
    } finally {
      host.kill();
    }
  });
});

test("work list --json keeps every work field and adds blockedBy, holder, gate and counts (d893)", async () => {
  await withFixture(async (fixture) => {
    const { parent, reader, waiting, offByOne } = await seed(fixture);

    const result = await runCli(fixture, ["work", "list", "--json"]);
    expect(result.exitCode).toBe(0);
    const data = (JSON.parse(result.stdout) as {
      data: { counts: Record<string, number>; works: Array<Record<string, unknown>> };
    }).data;
    expect(data.counts).toEqual({ open: 3, active: 1, done: 1, cancelled: 1 });
    expect(data.works.map((work) => work.id)).toEqual([parent, reader, offByOne, waiting]);
    for (const work of data.works) {
      expect(Object.keys(work).sort()).toEqual([
        "acceptance", "atomicReason", "blockedBy", "cancelReason", "candidate", "createdAt",
        "evidence", "gate", "heldBy", "holder", "id", "kind", "parentId", "reclaimReason",
        "reclaimedBy", "reclaimedFrom", "state", "title", "updatedAt",
      ]);
    }
    const byId = new Map(data.works.map((work) => [work.id, work]));
    expect(Object.keys(byId.get(reader)?.holder as object).sort()).toEqual(["harness", "id", "lastSeen", "live"]);
    expect(byId.get(reader)?.holder).toEqual(expect.objectContaining({
      id: "holder-live",
      lastSeen: expect.any(String),
      live: true,
    }));
    expect(byId.get(waiting)?.blockedBy).toEqual([
      { id: reader, state: "active" },
      { id: offByOne, state: "open" },
    ]);
    expect(byId.get(parent)?.gate).toEqual({
      command: expect.any(String),
      origin: "policy-breakdown",
      reason: expect.stringContaining(`${parent} has open children`),
    });
    expect(byId.get(offByOne)).toEqual(expect.objectContaining({ blockedBy: [], gate: null, holder: null }));
  });
});

test("work list works under MAESTRO_READ_ONLY=1 and ready is gone (d896)", async () => {
  await withFixture(async (fixture) => {
    const { reader } = await seed(fixture);

    const observed = await runCli(fixture, ["work", "list"], { MAESTRO_READ_ONLY: "1" });
    expect(observed.exitCode).toBe(0);
    expect(observed.stdout).toContain(`${reader} [active] split the reader  held by holder-live (live)`);

    const ready = await runCli(fixture, ["ready"]);
    expect(ready.exitCode).not.toBe(0);
    expect((await runCli(fixture, ["help"])).stdout).not.toMatch(/^ {2}ready /m);

    const brief = await runCli(fixture, ["brief", "--session"]);
    expect(brief.stdout).toContain("maestro work list shows what is open");
    expect(brief.stdout).not.toContain("maestro ready");
  });
});

test("decision list prints the 20 most recently updated with the total; --all and --work reach the rest (d893)", async () => {
  await withFixture(async (fixture) => {
    const work = await add(fixture, "decision anchor", ["--kind", "idea"]);
    for (let index = 1; index <= 22; index += 1) {
      expect((await runCli(fixture, ["decision", "draft", `choice ${index}`])).exitCode).toBe(0);
    }
    const linked = idFrom(await runCli(fixture, ["decision", "draft", "linked choice", "--work", work]));
    expect((await runCli(fixture, ["decision", "draft", "d2", "choice 2 revised"])).exitCode).toBe(0);

    const listed = await runCli(fixture, ["decision", "list"]);
    expect(listed.exitCode).toBe(0);
    const lines = listed.stdout.trimEnd().split("\n");
    expect(lines).toHaveLength(21);
    expect(lines[0]).toBe("d2 [draft] choice 2 revised");
    expect(lines[1]).toBe(`${linked} [draft] linked choice`);
    expect(lines[2]).toBe("d22 [draft] choice 22");
    expect(lines[19]).toBe("d5 [draft] choice 5");
    expect(lines[20]).toBe("showing 20 of 23; maestro decision list --all");

    const json = JSON.parse((await runCli(fixture, ["decision", "list", "--json"])).stdout) as {
      data: { decisions: Array<{ id: string }>; total: number };
    };
    expect(json.data.total).toBe(23);
    expect(json.data.decisions).toHaveLength(20);

    const all = await runCli(fixture, ["decision", "list", "--all"]);
    const allLines = all.stdout.trimEnd().split("\n");
    expect(allLines).toHaveLength(23);
    expect(allLines[0]).toBe("d1 [draft] choice 1");
    expect(allLines[22]).toBe(`${linked} [draft] linked choice`);

    const byWork = await runCli(fixture, ["decision", "list", "--work", work]);
    expect(byWork.stdout).toBe(`${linked} [draft] linked choice\n`);
    const unknown = await runCli(fixture, ["decision", "list", "--work", "w99"]);
    expect(unknown.stderr).toContain("NOT_FOUND");
  });
});
