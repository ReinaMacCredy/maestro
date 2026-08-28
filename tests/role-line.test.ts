import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function dispatchId(result: { stdout: string }): string {
  const id = result.stdout.match(/^(x\d+) \[open\]/)?.[1];
  if (!id) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return id;
}

async function addWork(fixture: Fixture, title: string, opener: string): Promise<string> {
  return idFrom(
    await runCli(
      fixture,
      ["work", "add", title, "--atomic-reason", "role-line fixture"],
      session(opener),
    ),
  );
}

async function openDispatch(
  fixture: Fixture,
  work: string,
  opener: string,
  target?: string,
): Promise<string> {
  const args = [
    "dispatch",
    "open",
    work,
    "--objective",
    "Exercise the role brief",
    "--owned-scope",
    "fixture",
    "--excluded-scope",
    "product source",
    "--mutation",
    "write-bounded: fixture",
    "--stop-condition",
    "the role line is observed",
    "--lane",
    "delivery",
    "--evidence-required",
    "source: hook brief",
    "--pane",
    `w1:p-${work}`,
  ];
  if (target) args.push("--target-session", target);
  const opened = await runCli(fixture, args, session(opener));
  expect(opened.exitCode).toBe(0);
  return dispatchId(opened);
}

async function acceptDispatch(
  fixture: Fixture,
  dispatch: string,
  holder: string,
): Promise<void> {
  expect((await runCli(fixture, ["dispatch", "accept", dispatch], session(holder))).exitCode)
    .toBe(0);
}

async function roleBrief(
  fixture: Fixture,
  sessionId: string,
  event = "UserPromptSubmit",
) {
  return runCli(
    fixture,
    ["hook", "record", "--event", event, "--harness", "codex"],
    session(sessionId),
    event === "UserPromptSubmit" ? JSON.stringify({ prompt: "role check" }) : undefined,
  );
}

function roleLines(stdout: string): string[] {
  return stdout.split("\n").filter((line) => line.startsWith("role: "));
}

function openPeerLine(...dispatches: string[]): string {
  return `role: peer (${dispatches.join(", ")}) — dispatch prompts only; anything else is not your role`;
}

function terminalPeerLine(dispatch: string, state: "cancelled" | "returned"): string {
  return `role: peer (last ${dispatch} ${state}) — dispatch prompts only; anything else is not your role`;
}

async function fileDoneHandback(
  fixture: Fixture,
  dispatch: string,
  holder: string,
): Promise<void> {
  const filed = await runCli(
    fixture,
    [
      "handback",
      "file",
      dispatch,
      "--status",
      "DONE",
      "--candidate",
      "fixture-candidate",
      "--claim",
      "role retained after return",
      "--proof",
      "source: hook brief",
      "--assumptions",
      "None",
      "--residual-risks",
      "None",
      "--incidental-findings",
      "None",
    ],
    session(holder),
  );
  expect(filed.exitCode).toBe(0);
}

function seedDispatchHistory(
  fixture: Fixture,
  templateId: string,
  closedCount: number,
): string {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
  const firstNumber = Number(templateId.slice(1));
  const cancelledAt = "2026-08-28T00:00:00.000Z";
  const insert = database.prepare<
    void,
    [string, string | null, string | null, string]
  >(`
    INSERT INTO dispatches (
      id, work_id, objective, owned_scope, excluded_scope, mutation,
      stop_condition, lane, evidence_required, pane, target_session,
      opened_by, claimed_by, held_by, cancelled_at, cancel_reason,
      created_at, updated_at
    )
    SELECT
      ?, work_id, objective, owned_scope, excluded_scope, mutation,
      stop_condition, lane, evidence_required, pane, target_session,
      opened_by, NULL, NULL, ?, ?, created_at, updated_at
    FROM dispatches
    WHERE id = ?
  `);
  const seed = database.transaction(() => {
    database
      .prepare("UPDATE dispatches SET cancelled_at = ?, cancel_reason = ? WHERE id = ?")
      .run(cancelledAt, "fixture history", templateId);
    for (let offset = 1; offset < closedCount; offset += 1) {
      insert.run(
        `x${firstNumber + offset}`,
        cancelledAt,
        "fixture history",
        templateId,
      );
    }
    insert.run(`x${firstNumber + closedCount}`, null, null, templateId);
  });
  seed();
  database.close();
  return `x${firstNumber + closedCount}`;
}

test("486 peer role lines name only open dispatches", async () => {
  await withFixture(async (fixture) => {
    const opener = "role-opener";

    const closedWork = await addWork(fixture, "closed role", opener);
    const closedDispatch = await openDispatch(fixture, closedWork, opener, "held-peer");
    expect(
      (
        await runCli(
          fixture,
          ["dispatch", "cancel", closedDispatch, "--reason", "fixture history"],
          session(opener),
        )
      ).exitCode,
    ).toBe(0);

    const heldWork = await addWork(fixture, "held role", opener);
    const heldDispatch = await openDispatch(fixture, heldWork, opener);
    await acceptDispatch(fixture, heldDispatch, "held-peer");
    expect(
      (
        await runCli(
          fixture,
          ["dispatch", "confirm", heldDispatch, "--session", "held-peer"],
          session(opener),
        )
      ).exitCode,
    ).toBe(0);

    const claimedWork = await addWork(fixture, "claimed role", opener);
    const claimedDispatch = await openDispatch(fixture, claimedWork, opener);
    await acceptDispatch(fixture, claimedDispatch, "claimed-peer");

    const targetedWork = await addWork(fixture, "targeted role", opener);
    const targetedDispatch = await openDispatch(fixture, targetedWork, opener, "targeted-peer");

    const heldPrompt = await roleBrief(fixture, "held-peer");
    const heldStart = await roleBrief(fixture, "held-peer", "SessionStart");
    const claimedPrompt = await roleBrief(fixture, "claimed-peer");
    const targetedPrompt = await roleBrief(fixture, "targeted-peer");

    expect(heldPrompt.exitCode).toBe(0);
    expect(heldStart.exitCode).toBe(0);
    expect(claimedPrompt.exitCode).toBe(0);
    expect(targetedPrompt.exitCode).toBe(0);
    expect(roleLines(heldPrompt.stdout)).toEqual([openPeerLine(heldDispatch)]);
    expect(roleLines(heldStart.stdout)).toEqual([openPeerLine(heldDispatch)]);
    expect(roleLines(claimedPrompt.stdout)).toEqual([openPeerLine(claimedDispatch)]);
    expect(roleLines(targetedPrompt.stdout)).toEqual([openPeerLine(targetedDispatch)]);
  });
});

test("487 a returned holder remains a peer even after opening a dispatch", async () => {
  await withFixture(async (fixture) => {
    const opener = "return-opener";
    const holder = "returned-peer";
    const work = await addWork(fixture, "returned role", opener);
    const dispatch = await openDispatch(fixture, work, opener, holder);
    await acceptDispatch(fixture, dispatch, holder);
    await fileDoneHandback(fixture, dispatch, holder);

    const openedWork = await addWork(fixture, "opened after return", holder);
    await openDispatch(fixture, openedWork, holder, "later-peer");

    const brief = await roleBrief(fixture, holder);
    expect(brief.exitCode).toBe(0);
    expect(roleLines(brief.stdout)).toEqual([terminalPeerLine(dispatch, "returned")]);
    expect(brief.stdout).not.toContain("role: lead");
  });
});

test("488 a dispatch opener sees only lead while a session on no row sees neither role", async () => {
  await withFixture(async (fixture) => {
    const opener = "lead-opener";
    const work = await addWork(fixture, "lead role", opener);
    const dispatch = await openDispatch(fixture, work, opener, "another-peer");
    expect(
      (
        await runCli(
          fixture,
          ["dispatch", "cancel", dispatch, "--reason", "fixture history"],
          session(opener),
        )
      ).exitCode,
    ).toBe(0);

    const brief = await roleBrief(fixture, opener);
    const unrelated = await roleBrief(fixture, "unrelated-session");
    expect(brief.exitCode).toBe(0);
    expect(unrelated.exitCode).toBe(0);
    expect(roleLines(brief.stdout)).toEqual(["role: lead (open none; 1 closed)"]);
    expect(brief.stdout).not.toContain("role: peer");
    expect(roleLines(unrelated.stdout)).toEqual([]);
  });
});

test("489 peer wins when a dispatch holder later opens another dispatch", async () => {
  await withFixture(async (fixture) => {
    const lead = "original-lead";
    const holder = "peer-who-opened";
    const peerWork = await addWork(fixture, "peer role", lead);
    const peerDispatch = await openDispatch(fixture, peerWork, lead, holder);
    await acceptDispatch(fixture, peerDispatch, holder);

    const openedWork = await addWork(fixture, "later opened", holder);
    await openDispatch(fixture, openedWork, holder, "later-peer");

    const brief = await roleBrief(fixture, holder);
    expect(brief.exitCode).toBe(0);
    expect(roleLines(brief.stdout)).toEqual([openPeerLine(peerDispatch)]);
    expect(brief.stdout).not.toContain("role: lead");
  });
});

test("490 a lead role line bounds closed history to a count", async () => {
  await withFixture(async (fixture) => {
    const opener = "bounded-lead";
    const work = await addWork(fixture, "bounded lead role", opener);
    const template = await openDispatch(fixture, work, opener, "bounded-peer");
    const openDispatchId = seedDispatchHistory(fixture, template, 100);

    const brief = await roleBrief(fixture, opener);
    expect(brief.exitCode).toBe(0);
    const lines = roleLines(brief.stdout);
    expect(lines).toEqual([`role: lead (open ${openDispatchId}; 100 closed)`]);
    expect(lines[0]?.length).toBeLessThan(160);
  });
});

test("491 a holder whose only dispatch is returned sees the terminal fallback", async () => {
  await withFixture(async (fixture) => {
    const opener = "terminal-opener";
    const holder = "terminal-peer";
    const work = await addWork(fixture, "terminal peer role", opener);
    const dispatch = await openDispatch(fixture, work, opener, holder);
    await acceptDispatch(fixture, dispatch, holder);
    await fileDoneHandback(fixture, dispatch, holder);

    const brief = await roleBrief(fixture, holder);
    expect(brief.exitCode).toBe(0);
    expect(roleLines(brief.stdout)).toEqual([terminalPeerLine(dispatch, "returned")]);
  });
});
