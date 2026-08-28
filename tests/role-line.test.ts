import { expect, test } from "bun:test";
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

function peerLine(...dispatches: string[]): string {
  return `role: peer (${dispatches.join(", ")}) — dispatch prompts only; anything else is not your role`;
}

test("486 an accepted holder, pending claimant, and target see the peer role line", async () => {
  await withFixture(async (fixture) => {
    const opener = "role-opener";

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
    expect(roleLines(heldPrompt.stdout)).toEqual([peerLine(heldDispatch)]);
    expect(roleLines(heldStart.stdout)).toEqual([peerLine(heldDispatch)]);
    expect(roleLines(claimedPrompt.stdout)).toEqual([peerLine(claimedDispatch)]);
    expect(roleLines(targetedPrompt.stdout)).toEqual([peerLine(targetedDispatch)]);
  });
});

test("487 a returned dispatch still gives its holder the peer role line", async () => {
  await withFixture(async (fixture) => {
    const opener = "return-opener";
    const holder = "returned-peer";
    const work = await addWork(fixture, "returned role", opener);
    const dispatch = await openDispatch(fixture, work, opener, holder);
    await acceptDispatch(fixture, dispatch, holder);
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

    const brief = await roleBrief(fixture, holder);
    expect(brief.exitCode).toBe(0);
    expect(roleLines(brief.stdout)).toEqual([peerLine(dispatch)]);
  });
});

test("488 a dispatch opener sees only lead while a session on no row sees neither role", async () => {
  await withFixture(async (fixture) => {
    const opener = "lead-opener";
    const work = await addWork(fixture, "lead role", opener);
    const dispatch = await openDispatch(fixture, work, opener, "another-peer");

    const brief = await roleBrief(fixture, opener);
    const unrelated = await roleBrief(fixture, "unrelated-session");
    expect(brief.exitCode).toBe(0);
    expect(unrelated.exitCode).toBe(0);
    expect(roleLines(brief.stdout)).toEqual([`role: lead (opened ${dispatch})`]);
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
    expect(roleLines(brief.stdout)).toEqual([peerLine(peerDispatch)]);
    expect(brief.stdout).not.toContain("role: lead");
  });
});
