import { expect, test } from "bun:test";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function dispatchOpenArgs(work: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Settle the storage boundary",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push, tag, publish",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "the contract is stored",
    "--lane",
    "delivery",
    "--evidence-required",
    "source and live",
  ];
}

function dispatchId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(\S+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return match[1];
}

async function openDispatch(fixture: Fixture): Promise<string> {
  const work = idFrom(
    await runCli(fixture, ["work", "add", "handback contract", "--atomic-reason", "fixture"]),
  );
  const opened = await runCli(fixture, dispatchOpenArgs(work));
  expect(opened.exitCode).toBe(0);
  return dispatchId(opened);
}

function handbackFileArgs(dispatch: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "the contract is stored",
    "--proof",
    "source: focused test passes",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
  ];
}

function handbackId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(\S+) \[[A-Z_]+\]/);
  if (!match?.[1]) throw new Error(`missing handback id in stdout: ${result.stdout}`);
  return match[1];
}

async function openCouncil(
  fixture: Fixture,
): Promise<{ dispatches: [string, string]; work: string }> {
  const work = idFrom(
    await runCli(fixture, ["work", "add", "sealed council", "--atomic-reason", "fixture"]),
  );
  const first = await runCli(fixture, dispatchOpenArgs(work));
  const second = await runCli(fixture, dispatchOpenArgs(work));
  expect(first.exitCode).toBe(0);
  expect(second.exitCode).toBe(0);
  return { dispatches: [dispatchId(first), dispatchId(second)], work };
}

test("173 dispatch open refuses every missing or blank envelope field", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "dispatch contract", "--atomic-reason", "fixture"]),
    );
    const fields = [
      "--objective",
      "--owned-scope",
      "--excluded-scope",
      "--mutation",
      "--stop-condition",
      "--lane",
      "--evidence-required",
    ];

    for (const field of fields) {
      const args = dispatchOpenArgs(work);
      const index = args.indexOf(field);
      args.splice(index, 2);
      const missing = await runCli(fixture, args);
      expect(missing.exitCode).not.toBe(0);
      expect(missing.stderr).toContain(field);

      const blankArgs = dispatchOpenArgs(work);
      blankArgs[blankArgs.indexOf(field) + 1] = "   ";
      const blank = await runCli(fixture, blankArgs);
      expect(blank.exitCode).not.toBe(0);
      expect(blank.stderr).toContain(field);
    }
  });
});

test("174 dispatch show and list render the complete stored contract and identities", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "render dispatch", "--atomic-reason", "fixture"]),
    );
    const opened = await runCli(
      fixture,
      [...dispatchOpenArgs(work), "--target-session", "lane-one"],
    );
    expect(opened.exitCode).toBe(0);
    const id = dispatchId(opened);

    for (const command of [["dispatch", "show", id], ["dispatch", "list"]]) {
      const rendered = await runCli(fixture, command);
      expect(rendered.exitCode).toBe(0);
      for (const line of [
        `work: ${work}`,
        "objective: Settle the storage boundary",
        "owned scope: src/plugins/dispatch.ts",
        "excluded scope: push, tag, publish",
        "mutation: write-bounded: src/plugins/dispatch.ts",
        "stop condition: the contract is stored",
        "lane: delivery",
        "evidence required: source and live",
        "target session: lane-one",
        "held by: none",
      ]) {
        expect(rendered.stdout).toContain(line);
      }
    }
  });
});

test("175 accepting dispatches never changes the work write lease", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "three lanes", "--atomic-reason", "fixture"]),
    );
    const holders = ["lane-one", "lane-two", "lane-three"];
    const dispatches: string[] = [];
    for (const holder of holders) {
      const opened = await runCli(
        fixture,
        [...dispatchOpenArgs(work), "--target-session", holder],
      );
      expect(opened.exitCode).toBe(0);
      const id = dispatchId(opened);
      dispatches.push(id);
      expect((await runCli(fixture, ["dispatch", "accept", id], session(holder))).exitCode).toBe(0);
    }

    const listed = await runCli(fixture, ["dispatch", "list", "--json"]);
    expect(listed.exitCode).toBe(0);
    const listEnvelope = JSON.parse(listed.stdout) as {
      data: { dispatches: Array<{ heldBy: string | null; id: string }> };
    };
    expect(
      listEnvelope.data.dispatches
        .filter((dispatch) => dispatches.includes(dispatch.id))
        .map((dispatch) => dispatch.heldBy),
    ).toEqual(holders);

    const shown = await runCli(fixture, ["work", "show", work]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).not.toContain("held by:");
  });
});

test("176 handback file refuses a status outside the eight-value vocabulary", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    const args = handbackFileArgs(dispatch);
    args[args.indexOf("--status") + 1] = "PASS";
    const filed = await runCli(fixture, args);
    expect(filed.exitCode).not.toBe(0);
    for (const status of [
      "DONE",
      "BLOCKED",
      "UNTESTABLE",
      "UNKNOWN",
      "FAILED",
      "CHALLENGE",
      "REOPEN_REQUEST",
      "DEPENDENCY_REQUEST",
    ]) {
      expect(filed.stderr).toContain(status);
    }
  });
});

test("177 handback assumptions and residual risks must be explicit while None is valid", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    for (const field of ["--assumptions", "--residual-risks"]) {
      const args = handbackFileArgs(dispatch);
      args[args.indexOf(field) + 1] = "  ";
      const blank = await runCli(fixture, args);
      expect(blank.exitCode).not.toBe(0);
      expect(blank.stderr).toContain(field);
    }

    const filed = await runCli(fixture, handbackFileArgs(dispatch));
    expect(filed.exitCode).toBe(0);
    const match = filed.stdout.match(/^(\S+) \[DONE\]/);
    expect(match?.[1]).toBeTruthy();
    const shown = await runCli(fixture, ["handback", "show", match?.[1] as string]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("assumptions not verified: None");
    expect(shown.stdout).toContain("residual risks: None");
    expect(shown.stdout).toContain("incidental findings: None");
  });
});

test("178 handback proof must name an evidence layer", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    const args = handbackFileArgs(dispatch);
    args[args.indexOf("--proof") + 1] = "the check passed";
    const filed = await runCli(fixture, args);
    expect(filed.exitCode).not.toBe(0);
    for (const layer of ["source", "artifact", "installed", "live", "journey"]) {
      expect(filed.stderr).toContain(layer);
    }
  });
});

test("179 a council stays sealed until every dispatch has returned", async () => {
  await withFixture(async (fixture) => {
    const council = await openCouncil(fixture);
    const first = await runCli(fixture, handbackFileArgs(council.dispatches[0]));
    expect(first.exitCode).toBe(0);
    const firstHandback = handbackId(first);

    const sealed = await runCli(fixture, ["handback", "show", firstHandback]);
    expect(sealed.exitCode).not.toBe(0);
    expect(sealed.stderr).toContain("SEALED");

    const listed = await runCli(fixture, ["dispatch", "list", council.work]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("council: sealed (1/2 returned)");
    expect(listed.stdout).not.toContain("claim:");

    expect((await runCli(fixture, handbackFileArgs(council.dispatches[1]))).exitCode).toBe(0);
    const opened = await runCli(fixture, ["handback", "show", firstHandback]);
    expect(opened.exitCode).toBe(0);
    expect(opened.stdout).toContain("claim: the contract is stored");
  });
});

test("180 a single dispatch handback reads immediately", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    const filed = await runCli(fixture, handbackFileArgs(dispatch));
    expect(filed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["handback", "show", handbackId(filed)]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("claim: the contract is stored");
    expect(shown.stdout).not.toContain("SEALED");
  });
});

test("181 dispatch unseal records its reason and marks later reads unsealed", async () => {
  await withFixture(async (fixture) => {
    const council = await openCouncil(fixture);
    const filed = await runCli(fixture, handbackFileArgs(council.dispatches[0]));
    const id = handbackId(filed);
    const reason = "owner ended the council early";

    const unsealed = await runCli(fixture, [
      "dispatch",
      "unseal",
      council.work,
      "--reason",
      reason,
    ]);
    expect(unsealed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["handback", "show", id]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("council: unsealed (1/2 returned)");
    expect(shown.stdout).toContain(`reason: ${reason}`);

    const trace = await runCli(fixture, ["trace", council.work]);
    expect(trace.exitCode).toBe(0);
    expect(trace.stdout).toContain("dispatch.unseal");
    expect(trace.stdout).toContain(reason);
  });
});
