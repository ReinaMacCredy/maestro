import { expect, test } from "bun:test";
import { join } from "node:path";
import {
  idFrom,
  initializeGitRepository,
  runCli,
  withFixture,
  type Fixture,
} from "./helpers.ts";

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
    "--pane",
    "w1:pA",
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
      "--pane",
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

test("263 returned and cancelled dispatches clear their lane holder", async () => {
  await withFixture(async (fixture) => {
    const returned = await openDispatch(fixture);
    expect(
      (await runCli(fixture, ["dispatch", "accept", returned], session("returning-lane")))
        .exitCode,
    ).toBe(0);
    expect(
      (await runCli(fixture, handbackFileArgs(returned), session("returning-lane"))).exitCode,
    ).toBe(0);

    const cancelled = await openDispatch(fixture);
    expect(
      (await runCli(fixture, ["dispatch", "accept", cancelled], session("cancelled-lane")))
        .exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(fixture, [
          "dispatch",
          "cancel",
          cancelled,
          "--reason",
          "lane was abandoned",
        ])
      ).exitCode,
    ).toBe(0);

    const listed = await runCli(fixture, ["dispatch", "list", "--json"]);
    expect(listed.exitCode).toBe(0);
    const envelope = JSON.parse(listed.stdout) as {
      data: { dispatches: Array<{ heldBy: string | null; id: string }> };
    };
    expect(
      envelope.data.dispatches
        .filter((dispatch) => [returned, cancelled].includes(dispatch.id))
        .map((dispatch) => dispatch.heldBy),
    ).toEqual([null, null]);
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

test("182 ready and work show add dispatches and linked decisions without changing existing lines", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "read path item", "--atomic-reason", "fixture"]),
    );
    const readyBefore = await runCli(fixture, ["ready"], session("lane-ready"));
    const showBefore = await runCli(fixture, ["work", "show", work]);
    const decision = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "keep dispatches attached to work",
        "--work",
        work,
      ]),
    );

    const opened = await runCli(fixture, [
      ...dispatchOpenArgs(work),
      "--target-session",
      "lane-ready",
    ]);
    const dispatch = dispatchId(opened);
    const readyAfter = await runCli(fixture, ["ready"], session("lane-ready"));
    const showAfter = await runCli(fixture, ["work", "show", work]);

    expect(readyAfter.stdout.split("\n")[0]).toBe(readyBefore.stdout.split("\n")[0]);
    expect(readyAfter.stdout).toContain(`dispatch: ${dispatch} [takeable]`);
    expect(showAfter.stdout.startsWith(showBefore.stdout.trimEnd())).toBe(true);
    expect(showAfter.stdout).toContain(`dispatch: ${dispatch} [open] delivery`);
    expect(showAfter.stdout).toContain(`decision: ${decision} [draft] keep dispatches attached to work`);

    const readyJson = JSON.parse(
      (await runCli(fixture, ["ready", "--json"], session("lane-ready"))).stdout,
    ) as { data: { dispatches: Array<{ id: string }> } };
    expect(readyJson.data.dispatches.map((item) => item.id)).toContain(dispatch);
    const showJson = await runCli(fixture, ["work", "show", work, "--json"]);
    expect(showJson.exitCode).toBe(0);
    const showEnvelope = JSON.parse(showJson.stdout) as {
      data: { decisions: Array<{ id: string }>; dispatches: Array<{ id: string }> };
    };
    expect(showEnvelope.data.dispatches.map((item) => item.id)).toContain(dispatch);
    expect(showEnvelope.data.decisions.map((item) => item.id)).toContain(decision);
  });
});

test("183 handoff renders returned handbacks into the packet", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const work = idFrom(
      await runCli(fixture, ["work", "add", "handoff returns", "--atomic-reason", "fixture"]),
    );
    expect(
      (await runCli(fixture, ["bundle", "open", "dispatch-handoff", "--work", work])).exitCode,
    ).toBe(0);
    const dispatch = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    expect((await runCli(fixture, handbackFileArgs(dispatch))).exitCode).toBe(0);

    const handedOff = await runCli(fixture, ["handoff", "dispatch-handoff", "--json"]);
    expect(handedOff.exitCode).toBe(0);
    const envelope = JSON.parse(handedOff.stdout) as {
      data: { handbacks: Array<{ dispatchId: string; status: string }> };
    };
    expect(envelope.data.handbacks).toEqual([
      expect.objectContaining({ dispatchId: dispatch, status: "DONE" }),
    ]);
    const notes = await Bun.file(
      join(fixture.repo, ".maestro", "bundle", "dispatch-handoff", "NOTES.md"),
    ).text();
    expect(notes).toContain("Handbacks:");
    expect(notes).toContain(`[DONE] dispatch ${dispatch}`);
    expect(notes).toContain("claim: the contract is stored");
    expect(notes).toContain("proof: source: focused test passes");
    expect(notes).toContain("assumptions not verified: None");
    expect(notes).toContain("residual risks: None");
    expect(notes).toContain("incidental findings: None");
  });
});

test("184 work done refuses an unreturned dispatch and cancel with a reason unblocks it", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "done gate", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session("holder"))).exitCode).toBe(0);
    const dispatch = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));

    const blocked = await runCli(
      fixture,
      ["work", "done", work, "--evidence", "source: fixture"],
      session("holder"),
    );
    expect(blocked.exitCode).not.toBe(0);
    const error = JSON.parse(blocked.stderr) as {
      error: { code: string; message: string; origin: string };
    };
    expect(error.error.code).toBe("GATE_BLOCKED");
    expect(error.error.origin).toBe("policy-dispatch");
    expect(error.error.message).toContain(`maestro dispatch cancel ${dispatch} --reason`);

    expect(
      (
        await runCli(fixture, [
          "dispatch",
          "cancel",
          dispatch,
          "--reason",
          "lane was abandoned",
        ])
      ).exitCode,
    ).toBe(0);
    const cancelled = await runCli(fixture, ["dispatch", "show", dispatch]);
    expect(cancelled.stdout).toContain(`${dispatch} [cancelled]`);
    expect(cancelled.stdout).toContain("cancel reason: lane was abandoned");
    expect(
      (
        await runCli(
          fixture,
          ["work", "done", work, "--evidence", "source: fixture"],
          session("holder"),
        )
      ).exitCode,
    ).toBe(0);
  });
});

test("185 work start refuses a sealed council until its dispatches are resolved", async () => {
  await withFixture(async (fixture) => {
    const council = await openCouncil(fixture);
    const blocked = await runCli(fixture, ["work", "start", council.work], session("implementer"));
    expect(blocked.exitCode).not.toBe(0);
    const error = JSON.parse(blocked.stderr) as {
      error: { code: string; message: string; origin: string };
    };
    expect(error.error.code).toBe("GATE_BLOCKED");
    expect(error.error.origin).toBe("policy-dispatch");
    expect(error.error.message).toContain("sealed council");

    for (const dispatch of council.dispatches) {
      expect(
        (
          await runCli(fixture, [
            "dispatch",
            "cancel",
            dispatch,
            "--reason",
            "council lane abandoned",
          ])
        ).exitCode,
      ).toBe(0);
    }
    expect(
      (await runCli(fixture, ["work", "start", council.work], session("implementer"))).exitCode,
    ).toBe(0);
  });
});

test("186 attention records one DISPATCH_UNRETURNED packet per fingerprint without routing", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "lead scope", "--kind", "idea"]),
    );
    expect((await runCli(fixture, ["work", "start", parent], session("lead-session"))).exitCode)
      .toBe(0);
    const child = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "lane question",
        "--parent",
        parent,
        "--atomic-reason",
        "fixture",
      ]),
    );
    const opened = await runCli(fixture, [
      ...dispatchOpenArgs(child),
      "--target-session",
      "worker-session",
    ]);
    const dispatch = dispatchId(opened);
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatch], session("worker-session"))).exitCode,
    ).toBe(0);

    const scan = () =>
      runCli(
        fixture,
        ["attention", "--json", "--dispatch-stale", "0.000001"],
        session("scanner-session"),
      );
    const first = await scan();
    expect(first.exitCode).toBe(0);
    const firstEnvelope = JSON.parse(first.stdout) as {
      data: {
        detections: Array<{
          fingerprint: string;
          kind: string;
          packet: string;
          raised: boolean;
          targets?: string[];
        }>;
      };
    };
    const firstDispatch = firstEnvelope.data.detections.filter(
      (finding) => finding.kind === "DISPATCH_UNRETURNED",
    );
    expect(firstDispatch).toHaveLength(1);
    expect(firstDispatch[0]?.raised).toBe(true);
    expect(firstDispatch[0]?.targets).toBeUndefined();
    expect(firstDispatch[0]?.fingerprint).toContain(dispatch);
    expect(firstDispatch[0]?.packet).toContain(`smallest action: maestro dispatch show ${dispatch}`);

    const secondEnvelope = JSON.parse((await scan()).stdout) as typeof firstEnvelope;
    const secondDispatch = secondEnvelope.data.detections.filter(
      (finding) => finding.kind === "DISPATCH_UNRETURNED",
    );
    expect(secondDispatch).toHaveLength(1);
    expect(secondDispatch[0]?.raised).toBe(false);
    expect(secondDispatch[0]?.targets).toBeUndefined();
  });
});

test("245 dispatch open refuses a missing pane and names the flag", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "pane required", "--atomic-reason", "fixture"]),
    );
    const args = dispatchOpenArgs(work);
    const pane = args.indexOf("--pane");
    args.splice(pane, 2);

    const opened = await runCli(fixture, args);

    expect(opened.exitCode).not.toBe(0);
    expect(opened.stderr).toContain("--pane");
  });
});

test("246 dispatch open stores the pane verbatim and dispatch show reports it", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "pane identity", "--atomic-reason", "fixture"]),
    );
    const pane = "  not/a-herdr-pane::verbatim  ";
    const args = dispatchOpenArgs(work);
    args[args.indexOf("--pane") + 1] = pane;
    const opened = await runCli(fixture, args);
    expect(opened.exitCode).toBe(0);
    const id = dispatchId(opened);

    const shown = await runCli(fixture, ["dispatch", "show", id]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`pane: ${pane}\n`);

    const json = await runCli(fixture, ["dispatch", "list", work, "--json"]);
    expect(json).toEqual(expect.objectContaining({ exitCode: 0, stderr: "" }));
    const envelope = JSON.parse(json.stdout) as {
      data: { dispatches: Array<{ id: string; pane: string }> };
    };
    expect(envelope.data.dispatches).toContainEqual(expect.objectContaining({ id, pane }));
  });
});
