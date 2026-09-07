import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { randomUUID } from "node:crypto";
import { existsSync } from "node:fs";
import { chmod, mkdir, readdir, readFile, realpath, rename, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { seatConfigRoot, seatTokenPath } from "../src/plugins/profiles.ts";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { slpRuntimeDirectory } from "../src/plugins/slp-process.ts";
import { editFakeHerdrState, fakeHerdrCommands, installFakeHerdr, setFakeHerdrBehavior, tripwireInvocations } from "./helpers-herdr.ts";
import {
  runCliAt,
  withFixture,
  writeConfig,
  type CliResult,
  type Fixture,
} from "./helpers.ts";

const maestroCli = join(import.meta.dir, "..", "bin", "maestro.ts");
// w702: Herdr renders its workspace counter in base 36, so a live pane id reads
// w2D:p1 or w2P:p5, never the digit-only w1:p1 the old /^w\d+:p\d+$/ demanded.
// That assertion could not pass against a real daemon, and this is the test
// that runs against one.
const herdrPaneIdShape = /^w[0-9A-Z]+:p\d+$/;
const roleCommandRunner = join(import.meta.dir, "slp-role-command.ts");

const runtimePhaseLine =
  /^\S+: (?:starting (?:claude|codex) pane in \S+|waiting for acknowledgement \(up to \d+s\)|ready in \d+s|already acknowledged in \S+; left alone)$/;

// Runtime phase lines (d757) are progress, not failures.
function phaseFree(stderr: string): string {
  return stderr
    .split("\n")
    .filter((line) => line !== "" && !runtimePhaseLine.test(line))
    .join("\n");
}

function envelope<T>(result: CliResult): T {
  expect(phaseFree(result.stderr)).toBe("");
  expect(result.exitCode).toBe(0);
  return (JSON.parse(result.stdout) as { data: T }).data;
}

function shellQuote(value: string): string {
  return `'${value.replaceAll("'", `'"'"'`)}'`;
}

function shellCommand(args: readonly string[]): string {
  return args.map(shellQuote).join(" ");
}

interface RoleCommandReceipt {
  command: string[];
  exitCode: number;
  stderr: string;
  stdout: string;
}

async function promptRoleCommand(
  fixture: Fixture,
  name: string,
  exactCommand: readonly string[],
  wait = true,
  tolerate?: (receipt: RoleCommandReceipt) => Tolerance,
): Promise<RoleCommandReceipt | null> {
  const receiptPath = join(fixture.root, "role-receipts", `${randomUUID()}.json`);
  const executedCommand = wait
    ? [process.execPath, roleCommandRunner, receiptPath, "--", ...exactCommand]
    : [...exactCommand];
  const prompt =
    "Execute this exact command once in the current project pane. " +
    "Use the HERDR_PANE_ID already injected into your live pane; do not set or replace it. " +
    "Do not paraphrase or simulate it. Report its literal exit code and output after it runs:\n\n" +
    shellCommand(executedCommand);
  const command = ["herdr", "agent", "prompt", name, prompt];
  if (wait) command.push("--wait", "--timeout", "120000");
  const child = Bun.spawn(command, {
    cwd: fixture.repo,
    env: process.env,
    stderr: "pipe",
    stdout: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) {
    throw new Error(`prompting ${name} failed (${exitCode}): ${stderr.trim()}\n${stdout.trim()}`);
  }
  if (!wait) return null;
  await waitFor(() => existsSync(receiptPath), `${name} did not write its command receipt`);
  const receipt = JSON.parse(await readFile(receiptPath, "utf8")) as RoleCommandReceipt;
  expect(receipt.command).toEqual([...exactCommand]);
  return settleRoleCommand(name, receipt, tolerate);
}

// The pass/fail rule for one scripted command, split out so the tolerance can
// be exercised without a live daemon.
function settleRoleCommand(
  name: string,
  receipt: RoleCommandReceipt,
  tolerate?: (receipt: RoleCommandReceipt) => Tolerance,
): RoleCommandReceipt {
  if (receipt.exitCode !== 0) {
    const verdict = tolerate?.(receipt);
    if (verdict?.tolerated) return receipt;
    throw new Error(
      `${name} command failed (${receipt.exitCode}): ${receipt.command.join(" ")}\n` +
        (verdict ? `not tolerated: ${verdict.evidence}\n` : "") +
        `stderr:\n${receipt.stderr}\nstdout:\n${receipt.stdout}`,
    );
  }
  expect(receipt.exitCode).toBe(0);
  expect(phaseFree(receipt.stderr)).toBe("");
  return receipt;
}

async function promptAgent<T = unknown>(
  fixture: Fixture,
  name: string,
  args: readonly string[],
  wait = true,
): Promise<T | null> {
  const receipt = await promptRoleCommand(
    fixture,
    name,
    [process.execPath, maestroCli, ...args],
    wait,
  );
  if (!receipt) return null;
  return envelope<T>({ exitCode: receipt.exitCode, stderr: receipt.stderr, stdout: receipt.stdout });
}

type Tolerance = { tolerated: boolean; evidence: string };

// w703: the journey's subjects are live agents. An agent woken by its
// `[from <role>][<id> OPEN]` line acts on it, so by the time a scripted prompt
// lands the item may already be ACTIVE, RETURNED, or - as observed - DONE,
// moved there by the very subjects the script was about to drive. That is
// correct SLP behaviour on both sides, so the script follows the store instead
// of assuming it is the only actor: it runs a step only while the store still
// needs it, and tolerates a step raced mid-flight only when the store shows
// that item moved FORWARD along the one legal path, still held by the same
// assignee. Anything else - a backward or sideways state, another holder,
// another error code, an unreadable envelope - is a state no correct subject
// could have produced and still fails.
const workStates = ["OPEN", "ACTIVE", "RETURNED", "DONE"] as const;
type WorkState = (typeof workStates)[number];

function workRank(state: string): number {
  const rank = workStates.indexOf(state as WorkState);
  if (rank < 0) throw new Error(`unknown work state: ${state}`);
  return rank;
}

interface WorkRow {
  assigned_to: string;
  state: string;
}

function workRow(projectDatabasePath: string, workId: string): WorkRow | null {
  if (!existsSync(projectDatabasePath)) return null;
  const database = new Database(projectDatabasePath, { readonly: true });
  try {
    return (
      database
        .query<WorkRow, [string]>("SELECT assigned_to, state FROM slp_work WHERE id = ?")
        .get(workId) ?? null
    );
  } finally {
    database.close();
  }
}

// One scripted command in an item's four-state path.
interface ScriptedStep {
  actor: string;
  args: readonly string[];
  assignee: string;
  from: WorkState;
  workId: string;
}

// Read the store, then decide whether this step is still the script's to run.
function stepPlan(step: ScriptedStep, current: WorkRow | null): "run" | "skip" {
  if (!current) throw new Error(`${step.workId} has no row in the store`);
  if (current.assigned_to !== step.assignee) {
    throw new Error(
      `${step.workId} is held by ${current.assigned_to}, not its assignee ${step.assignee}`,
    );
  }
  const rank = workRank(current.state);
  const needed = workRank(step.from);
  if (rank === needed) return "run";
  if (rank > needed) return "skip";
  throw new Error(
    `${step.workId} is ${current.state}; ${step.args.join(" ")} needs it at ${step.from} or beyond`,
  );
}

// The same forward-only rule applied to a step that lost the race mid-flight.
function racedForward(
  projectDatabasePath: string,
  step: ScriptedStep,
): (receipt: RoleCommandReceipt) => Tolerance {
  return (receipt) => {
    let code: string | null = null;
    try {
      code = (JSON.parse(receipt.stderr) as { error?: { code?: string } }).error?.code ?? null;
    } catch {
      return { tolerated: false, evidence: "stderr is not a maestro --json envelope" };
    }
    if (code !== "INVALID_STATE") {
      return { tolerated: false, evidence: `error code is ${code ?? "absent"}, not INVALID_STATE` };
    }
    const current = workRow(projectDatabasePath, step.workId);
    if (!current) {
      return { tolerated: false, evidence: `no ${step.workId} row at ${projectDatabasePath}` };
    }
    const seen = `${step.workId} is ${current.state} held by ${current.assigned_to}`;
    const evidence =
      `${seen}; the race tolerates only a state past ${step.from} held by ${step.assignee}`;
    if (current.assigned_to !== step.assignee) return { tolerated: false, evidence };
    return { tolerated: workRank(current.state) > workRank(step.from), evidence };
  };
}

async function runScriptedStep(
  fixture: Fixture,
  projectDatabasePath: string,
  step: ScriptedStep,
): Promise<void> {
  if (stepPlan(step, workRow(projectDatabasePath, step.workId)) === "skip") return;
  await promptRoleCommand(
    fixture,
    step.actor,
    [process.execPath, maestroCli, ...step.args],
    true,
    racedForward(projectDatabasePath, step),
  );
}

// Drive one item along OPEN -> ACTIVE -> RETURNED -> DONE, skipping whatever
// its own subjects already did.
async function driveWorkToDone(
  fixture: Fixture,
  projectDatabasePath: string,
  work: { assignee: string; id: string },
  reviewer: string,
  body: { note?: string; result: string },
): Promise<void> {
  const steps: ScriptedStep[] = [
    { actor: work.assignee, args: ["work", "take", work.id, "--json"], assignee: work.assignee, from: "OPEN", workId: work.id },
  ];
  if (body.note !== undefined) {
    steps.push({
      actor: work.assignee,
      args: ["work", "note", work.id, body.note, "--json"],
      assignee: work.assignee,
      from: "ACTIVE",
      workId: work.id,
    });
  }
  steps.push(
    { actor: work.assignee, args: ["work", "return", work.id, body.result, "--json"], assignee: work.assignee, from: "ACTIVE", workId: work.id },
    { actor: reviewer, args: ["work", "accept", work.id, "--json"], assignee: work.assignee, from: "RETURNED", workId: work.id },
  );
  for (const step of steps) await runScriptedStep(fixture, projectDatabasePath, step);
}

async function waitFor(
  predicate: () => boolean | Promise<boolean>,
  failure: string,
  timeoutMs = 120_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await Bun.sleep(250);
  }
  throw new Error(failure);
}

async function withHerdrFixture<T>(run: (fixture: Fixture) => Promise<T>): Promise<T> {
  const configured = process.env.MAESTRO_HERDR_TRUSTED_PROJECT;
  if (!configured) return withFixture(run);
  const repo = resolve(configured);
  const allowedParents = new Set([resolve("/private/tmp"), resolve(tmpdir())]);
  if (!allowedParents.has(dirname(repo)) || !basename(repo).startsWith("maestro-")) {
    throw new Error(
      "MAESTRO_HERDR_TRUSTED_PROJECT must be an absent maestro-* directory directly under a temporary directory",
    );
  }
  if (existsSync(repo)) {
    throw new Error("MAESTRO_HERDR_TRUSTED_PROJECT must not exist before the journey");
  }
  const fixture = { home: join(repo, ".fixture-home"), repo, root: repo };
  await mkdir(join(fixture.repo, ".maestro", "plugins"), { recursive: true });
  await mkdir(fixture.home, { recursive: true });
  await writeConfig(fixture, []);
  try {
    return await run(fixture);
  } finally {
    await rm(repo, { recursive: true, force: true });
  }
}

test.skipIf(process.env.HERDR_ENV !== "1")(
  "SLP v2 completes the nine-operation journey through live Herdr agents",
  async () => {
    await withHerdrFixture(async (fixture) => {
      const herdrHome = process.env.HOME;
      if (!herdrHome) throw new Error("real Herdr journey requires HOME");
      const liveEnvironment = { HOME: herdrHome };
      const room = await scaffoldRoom(fixture.home);
      const marked = await runCliAt(fixture, room, ["room", "mark"], {
        ...liveEnvironment,
        MAESTRO_ROOM_SCAFFOLD: "1",
        MAESTRO_SESSION_NONE: "1",
      });
      expect(phaseFree(marked.stderr)).toBe("");
      expect(marked.exitCode).toBe(0);
      let teamId: string | null = null;
      let stopped = false;
      try {
        const started = envelope<{
          team: {
            generation: number;
            roles: Array<{ name: string; paneId: string; role: string }>;
            runtimePaneId: string;
            teamId: string;
            workspaceId: string;
          };
          work: { id: string };
        }>(
          await runCliAt(
            fixture,
            room,
            ["team", "start", fixture.repo, "Complete the real Herdr SLP journey", "--json"],
            liveEnvironment,
          ),
        );
        teamId = started.team.teamId;
        const lead = started.team.roles.find((role) => role.role === "lead")!;
        const supervisor = started.team.roles.find((role) => role.role === "team-supervisor")!;
        const projectDatabasePath = join(fixture.repo, ".maestro", "maestro.db");
        // Hub d96: team start opened the runtime pane beside the Supervisor.
        expect(started.team.runtimePaneId).toMatch(herdrPaneIdShape);
        for (const malformed of ["", "w2G", "p3", "w2G:p", "w2G:t2", "workspace:pane"]) {
          expect(malformed).not.toMatch(herdrPaneIdShape);
        }
        // w703: the Lead's first item is pushed to it by team start (w696), so
        // it races the same way a Peer's does.
        await driveWorkToDone(
          fixture,
          projectDatabasePath,
          { assignee: lead.name, id: started.work.id },
          supervisor.name,
          { result: "result: initial objective complete; proof: live Lead pane" },
        );

        for (const target of ["peer-real-one", "peer-real-two"]) {
          await promptAgent(fixture, lead.name, [
            "work",
            "add",
            `Independent result from ${target}`,
            "--to",
            target,
            "--json",
          ]);
        }
        const project = new Database(projectDatabasePath, { readonly: true });
        const peerWork = project
          .query<{
            assigned_to: string;
            id: string;
          }, [string]>(
            `SELECT id, assigned_to FROM slp_work
             WHERE created_by = ? ORDER BY id`,
          )
          .all(lead.name);
        const peerRoles = new Map(
          project
            .query<{ name: string; pane_id: string }, []>(
              "SELECT name, pane_id FROM slp_local_roles WHERE role = 'peer' ORDER BY name",
            )
            .all()
            .map((role) => [role.name, role.pane_id]),
        );
        project.close();
        expect(peerWork).toHaveLength(2);
        expect(peerRoles.size).toBe(2);

        for (const work of peerWork) {
          expect(peerRoles.has(work.assigned_to)).toBe(true);
          await promptAgent(fixture, work.assigned_to, ["status", work.id, "--json"]);
          await driveWorkToDone(
            fixture,
            projectDatabasePath,
            { assignee: work.assigned_to, id: work.id },
            lead.name,
            {
              note: "proof: direct note from this live Peer pane",
              result: "result: independent result complete; proof: live Peer pane",
            },
          );
        }
        // Hub d96: the runtime pane holds the subscription; the runtime lock in
        // the generation directory proves it is up.
        const runtimeDirectory = slpRuntimeDirectory(
          fixture.repo,
          started.team.teamId,
          started.team.generation,
        );
        await waitFor(
          () => existsSync(join(runtimeDirectory, "runtime.lock")),
          "the runtime pane did not take its lock",
        );

        await promptAgent(fixture, lead.name, [
          "decide",
          "Use both independent Peer results",
          "--why",
          "Both returned through distinct live Peer panes",
          "--work",
          peerWork[0]!.id,
          "--json",
        ]);
        await promptAgent(fixture, supervisor.name, [
          "work",
          "note",
          peerWork[0]!.id,
          "Team Supervisor reviewed the live Peer handoff",
          "--json",
        ]);
        await promptAgent(fixture, supervisor.name, [
          "decide",
          "The team journey is ready to close",
          "--why",
          "Every work item is accepted",
          "--json",
        ]);
        envelope(
          await runCliAt(
            fixture,
            room,
            [
              "decide",
              "Accept the real runtime journey",
              "--why",
              "Hub and team durable state agree",
              "--work",
              `${teamId}:${peerWork[0]!.id}`,
              "--json",
            ],
            liveEnvironment,
          ),
        );

        const durable = new Database(projectDatabasePath, { readonly: true });
        expect(
          durable
            .query<{ count: number }, []>(
              "SELECT COUNT(*) AS count FROM slp_work WHERE state <> 'DONE'",
            )
            .get()?.count,
        ).toBe(0);
        expect(
          durable
            .query<{ count: number }, []>(
              "SELECT COUNT(DISTINCT actor) AS count FROM slp_activity WHERE operation IN ('work.take', 'work.note', 'work.return', 'work.accept')",
            )
            .get()?.count,
        ).toBeGreaterThanOrEqual(4);
        expect(
          durable
            .query<{ actor: string }, []>(
              "SELECT DISTINCT actor FROM slp_decisions ORDER BY actor",
            )
            .all()
            .map((row) => row.actor),
        ).toEqual([lead.name, supervisor.name].sort());
        durable.close();
        const hubDurable = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
        expect(
          hubDurable
            .query<{ count: number }, []>(
              "SELECT COUNT(*) AS count FROM slp_decisions WHERE actor = 'hub-supervisor' AND work_id IS NOT NULL",
            )
            .get()?.count,
        ).toBe(1);
        hubDurable.close();

        const hubStatus = envelope<{
          teams: Array<{ missingPanes: string[]; runtimePane: string; teamId: string }>;
        }>(
          await runCliAt(fixture, room, ["status", "--json"], liveEnvironment),
        );
        expect(hubStatus.teams).toContainEqual(
          expect.objectContaining({ missingPanes: [], runtimePane: "on", teamId }),
        );

        await promptAgent(
          fixture,
          supervisor.name,
          ["team", "stop", teamId, "--json"],
          false,
        );
        await waitFor(() => {
          const current = new Database(projectDatabasePath, { readonly: true });
          try {
            return current
              .query<{ state: string }, []>("SELECT state FROM slp_local_teams ORDER BY generation DESC LIMIT 1")
              .get()?.state === "STOPPED";
          } finally {
            current.close();
          }
        }, "live Team Supervisor did not commit STOPPED");
        await waitFor(async () => {
          const status = envelope<{
            teams: Array<{ runtime: string; state: string; teamId: string }>;
          }>(await runCliAt(fixture, room, ["status", "--json"], liveEnvironment));
          const team = status.teams.find((candidate) => candidate.teamId === teamId);
          return team?.state === "STOPPED" && team.runtime === "not-running";
        }, "live workspace did not finish shutdown");
        stopped = true;
        expect(
          existsSync(runtimeDirectory),
        ).toBe(false);
      } finally {
        if (teamId && !stopped) {
          await runCliAt(
            fixture,
            room,
            ["team", "stop", teamId, "--emergency", "--json"],
            liveEnvironment,
          );
        }
      }
    });
  },
  600_000,
);

// seat-config-dirs R6/R7 (d850, d846, A3): the socket fake records every
// tab.create env; a Claude seat pane is created with CLAUDE_CONFIG_DIR, a
// Codex one without; a matching-label shell tab is closed and recreated; a
// missing token refuses before any pane opens.

function failure(stderr: string): { code: string; message: string } {
  return (JSON.parse(stderr) as { error: { code: string; message: string } }).error;
}

async function markedRoom(fixture: Fixture): Promise<string> {
  const room = await scaffoldRoom(fixture.home);
  const marked = await runCliAt(fixture, room, ["room", "mark"], {
    MAESTRO_ROOM_SCAFFOLD: "1",
    MAESTRO_SESSION_NONE: "1",
  });
  expect(marked.exitCode).toBe(0);
  return room;
}

function tabCreates(commands: string[][]): Map<string, string[]> {
  return new Map(
    commands
      .filter((command) => command[0] === "tab" && command[1] === "create")
      .map((command) => [command[command.indexOf("--label") + 1] ?? "", command]),
  );
}

test("journey-race: a scripted step is skipped when its own subject already carried the item forward (take on a DONE item, note on a RETURNED one) and tolerated when it loses the race mid-flight, and still fails on a backward state, another holder, another error code, an unreadable envelope or an absent row (w703)", async () => {
  await withFixture(async (fixture) => {
    const databasePath = join(fixture.repo, "race.db");
    const database = new Database(databasePath, { create: true });
    database.run("CREATE TABLE slp_work (id TEXT PRIMARY KEY, assigned_to TEXT, state TEXT)");
    // w2 is the live specimen: the Peer was woken by its OPEN push and carried
    // the item to DONE with the Lead before the scripted take ever landed.
    database.run("INSERT INTO slp_work VALUES ('w2', 'peer-real-one-f88a8b', 'DONE')");
    database.run("INSERT INTO slp_work VALUES ('w3', 'peer-real-two-a1b2c3', 'RETURNED')");
    database.run("INSERT INTO slp_work VALUES ('w4', 'lead-live-9c0d1e', 'OPEN')");
    database.close();

    const step = (workId: string, assignee: string, from: WorkState, verb: string): ScriptedStep => ({
      actor: assignee,
      args: ["work", verb, workId, "--json"],
      assignee,
      from,
      workId,
    });
    const takeW2 = step("w2", "peer-real-one-f88a8b", "OPEN", "take");
    const noteW3 = step("w3", "peer-real-two-a1b2c3", "ACTIVE", "note");
    const takeW4 = step("w4", "lead-live-9c0d1e", "OPEN", "take");
    const read = (workId: string) => workRow(databasePath, workId);

    // The store decides which steps are still the script's to run.
    expect(stepPlan(takeW4, read("w4"))).toBe("run");
    expect(stepPlan(takeW2, read("w2"))).toBe("skip");
    expect(stepPlan(noteW3, read("w3"))).toBe("skip");
    // Backward, another holder, no row: no correct subject produced these.
    expect(() => stepPlan(step("w4", "lead-live-9c0d1e", "ACTIVE", "return"), read("w4"))).toThrow(
      /w4 is OPEN; work return w4 --json needs it at ACTIVE or beyond/,
    );
    expect(() => stepPlan(step("w2", "peer-real-two-a1b2c3", "OPEN", "take"), read("w2"))).toThrow(
      /w2 is held by peer-real-one-f88a8b, not its assignee peer-real-two-a1b2c3/,
    );
    expect(() => stepPlan(step("w9", "lead-live-9c0d1e", "OPEN", "take"), read("w9"))).toThrow(
      /w9 has no row in the store/,
    );

    // A step that read "run" and then lost the race reports the same failure
    // the live journey produced.
    const failed = (workId: string, message: string, code = "INVALID_STATE"): RoleCommandReceipt => ({
      command: [process.execPath, maestroCli, "work", "take", workId, "--json"],
      exitCode: 1,
      stderr: JSON.stringify({ ok: false, error: { code, message } }),
      stdout: "",
    });
    const raced = failed("w2", "w2 must be OPEN or RETURNED before work take");
    const tolerated = racedForward(databasePath, takeW2);
    expect(tolerated(raced).tolerated).toBe(true);
    // Forward from ACTIVE counts too: a note lost to the subject's own return.
    expect(racedForward(databasePath, noteW3)(raced).tolerated).toBe(true);
    // Wired in: the command settles as a pass instead of throwing.
    expect(settleRoleCommand("peer-real-one-f88a8b", raced, tolerated).exitCode).toBe(1);

    // Everything a correct subject could not have produced still fails, and
    // says in the failure what the store actually held.
    const rejected: Array<[string, Tolerance]> = [
      // the store never moved: INVALID_STATE here contradicts the store
      ["a state still at OPEN", racedForward(databasePath, takeW4)(raced)],
      // held by somebody other than this item's assignee
      ["another holder", racedForward(databasePath, step("w2", "peer-real-two-a1b2c3", "OPEN", "take"))(raced)],
      // no such row, and no such store
      ["an absent row", racedForward(databasePath, step("w9", "peer-real-one-f88a8b", "OPEN", "take"))(raced)],
      ["an absent store", racedForward(join(fixture.repo, "none.db"), takeW2)(raced)],
      // right store state, wrong failure: not this race
      ["another error code", tolerated(failed("w2", "no such work: w2", "NOT_FOUND"))],
      ["a plain-text failure", tolerated({ ...raced, stderr: "bun: command not found" })],
    ];
    for (const [what, verdict] of rejected) {
      expect(`${what}: ${verdict.tolerated}`).toBe(`${what}: false`);
      expect(verdict.evidence).not.toBe("");
      expect(() => settleRoleCommand("peer-real-one-f88a8b", raced, () => verdict)).toThrow(
        new RegExp(`not tolerated: ${verdict.evidence.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&")}`),
      );
    }
  });
});

test("seat-dirs-launch: team start creates each Claude seat pane with env CLAUDE_CONFIG_DIR=<seat dir> and a Codex peer without env; a matching-label shell tab is closed then recreated with the env (A3); a missing token refuses SEAT_TOKEN_MISSING before any tab.create (R6, R7)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "lead.md"),
      "---\nharness: claude\nmodel: default\ndescription: claude lead\n---\nRole: Lead (claude shadow).\n",
    );
    const fake = await installFakeHerdr(fixture);
    const seatRoot = seatConfigRoot(fixture.home);

    // d846: no token, no pane.
    const parked = `${seatTokenPath(fixture.home)}.parked`;
    await rename(seatTokenPath(fixture.home), parked);
    const refused = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Seat dirs", "--json"], fake.env);
    expect(refused.exitCode).toBe(1);
    expect(failure(refused.stderr).code).toBe("SEAT_TOKEN_MISSING");
    expect(failure(refused.stderr).message).toContain("maestro install --seat-token");
    expect(failure(refused.stderr).message).toContain(seatTokenPath(fixture.home));
    expect((await fakeHerdrCommands(fake)).filter((command) => command[1] === "create")).toEqual([]);
    await rename(parked, seatTokenPath(fixture.home));

    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Seat dirs", "--json"], fake.env);
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    const data = envelope<{ team: { roles: Array<{ name: string; paneId: string; role: string }>; teamId: string } }>(started);
    const prefix = `slp:${data.team.teamId}:g1`;
    let creates = tabCreates(await fakeHerdrCommands(fake));
    expect(creates.get(`${prefix}:team-supervisor`)).toContain(`CLAUDE_CONFIG_DIR=${join(seatRoot, "team-supervisor")}`);
    expect(creates.get(`${prefix}:lead`)).toContain(`CLAUDE_CONFIG_DIR=${join(seatRoot, "lead")}`);
    expect(creates.get(`${prefix}:lead`)).toContain("--env");

    // R7: a Claude peer gets the peer dir; the shipped codex peer gets no env.
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const security = await runCliAt(fixture, fixture.repo, ["work", "add", "review it", "--to", "peer-reviewer-security", "--json"], leadEnvironment);
    expect(phaseFree(security.stderr)).toBe("");
    expect(security.exitCode).toBe(0);
    const securityRole = envelope<{ role: { name: string } }>(security).role;
    const codex = await runCliAt(fixture, fixture.repo, ["work", "add", "codex item", "--to", "x", "--json"], leadEnvironment);
    expect(phaseFree(codex.stderr)).toBe("");
    expect(codex.exitCode).toBe(0);
    const codexRole = envelope<{ role: { name: string } }>(codex).role;
    creates = tabCreates(await fakeHerdrCommands(fake));
    expect(creates.get(`${prefix}:peer:${securityRole.name}`)).toContain(`CLAUDE_CONFIG_DIR=${join(seatRoot, "peer")}`);
    expect(creates.get(`${prefix}:peer:${codexRole.name}`)).not.toContain("--env");
    expect(creates.get(`${prefix}:peer:${codexRole.name}`)).toBeDefined();

    // A3: the Lead's pane went back to a shell prompt (agent gone, tab kept);
    // the re-run closes that tab and creates the pane with the env.
    let leadTabId = "";
    await editFakeHerdrState(fake, (state) => {
      const tab = (state.tabs as Array<{ label?: string; tab_id: string }>).find((candidate) => candidate.label === `${prefix}:lead`)!;
      leadTabId = tab.tab_id;
      state.agents = (state.agents as Array<{ pane_id: string }>).filter((agent) => agent.pane_id !== lead.paneId);
      delete state.processes[lead.paneId];
    });
    const before = (await fakeHerdrCommands(fake)).length;
    const repaired = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Seat dirs", "--json"], fake.env);
    expect(phaseFree(repaired.stderr)).toBe("");
    expect(repaired.exitCode).toBe(0);
    const after = (await fakeHerdrCommands(fake)).slice(before);
    const closeIndex = after.findIndex((command) => command[0] === "tab" && command[1] === "close" && command[2] === leadTabId);
    const createIndex = after.findIndex((command) => command[0] === "tab" && command[1] === "create" && command.includes(`${prefix}:lead`));
    expect(closeIndex).toBeGreaterThanOrEqual(0);
    expect(createIndex).toBeGreaterThan(closeIndex);
    expect(after[createIndex]).toContain(`CLAUDE_CONFIG_DIR=${join(seatRoot, "lead")}`);
    expect(after.filter((command) => command[0] === "pane" && command[1] === "close")).toEqual([]);
    const repairedLead = envelope<{ team: { roles: Array<{ paneId: string; role: string }> } }>(repaired).team.roles.find((role) => role.role === "lead")!;
    expect(repairedLead.paneId).not.toBe(lead.paneId);

    // A Claude peer with the token gone is refused the same way, before tab.create.
    await rename(seatTokenPath(fixture.home), parked);
    const marker = (await fakeHerdrCommands(fake)).length;
    const peerRefused = await runCliAt(fixture, fixture.repo, ["work", "add", "late", "--to", "peer-refuter", "--json"], { ...fake.env, HERDR_PANE_ID: repairedLead.paneId });
    expect(peerRefused.exitCode).toBe(1);
    expect(failure(peerRefused.stderr).code).toBe("SEAT_TOKEN_MISSING");
    expect((await fakeHerdrCommands(fake)).slice(marker).filter((command) => command[1] === "create")).toEqual([]);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

// seat-config-dirs R10 (d853): a fresh seat dir carries no per-project trust,
// so the owner's team start is the trust act: the project's
// hasTrustDialogAccepted goes into each launched Claude seat's .claude.json
// before its pane opens, every other key kept, nothing else seeded.
test("seat-dirs-trust: team start writes projects[<project>].hasTrustDialogAccepted into each launched Claude seat .claude.json before its tab.create (it survives a TRUST_DIALOG rollback), keeps every other key, a second launch leaves the file byte-identical; work add --to seeds the peer dir; a Codex seat writes nothing (R10, d853)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "lead.md"),
      "---\nharness: claude\nmodel: default\ndescription: claude lead\n---\nRole: Lead (claude shadow).\n",
    );
    const fake = await installFakeHerdr(fixture, { trustDialog: "claude" });
    const seatRoot = seatConfigRoot(fixture.home);
    const seatJson = (seat: string) => join(seatRoot, seat, ".claude.json");
    const readSeat = async (seat: string) => JSON.parse(await readFile(seatJson(seat), "utf8")) as Record<string, unknown>;
    // Claude Code keys projects by the cwd it starts in: the pane's cwd.
    const projectKey = await realpath(fixture.repo);
    const leadBefore = {
      hasCompletedOnboarding: true,
      mcpServers: {},
      numStartups: 3,
      projects: { "/elsewhere": { allowedTools: ["Bash(ls:*)"], hasTrustDialogAccepted: true }, [projectKey]: { allowedTools: ["Bash(git:*)"] } },
    };
    const leadText = `${JSON.stringify(leadBefore, null, 2)}\n`;
    await writeFile(seatJson("lead"), leadText);
    const peerBefore = { hasCompletedOnboarding: true, mcpServers: {}, userID: "peer-fixture" };
    const peerText = `${JSON.stringify(peerBefore, null, 2)}\n`;
    await writeFile(seatJson("peer"), peerText);
    const creates = async () => (await fakeHerdrCommands(fake)).filter((command) => command[0] === "tab" && command[1] === "create");

    // The team-supervisor launches first and its agent.start blocks on the
    // trust dialog: the start rolls back, its pane was the only tab.create,
    // and its file already carries the key; the lead never reached tab.create.
    const blocked = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Seat trust", "--json"], fake.env);
    expect(blocked.exitCode).toBe(1);
    expect(failure(phaseFree(blocked.stderr)).code).toBe("TRUST_DIALOG");
    const first = await creates();
    expect(first.length).toBe(1);
    expect(first[0]![first[0]!.indexOf("--cwd") + 1]).toBe(projectKey);
    expect(await readSeat("team-supervisor")).toEqual({
      hasCompletedOnboarding: true,
      mcpServers: {},
      projects: { [projectKey]: { hasTrustDialogAccepted: true } },
    });
    const supervisorText = await readFile(seatJson("team-supervisor"), "utf8");
    expect(await readFile(seatJson("lead"), "utf8")).toBe(leadText);

    // A clean start launches the team-supervisor pane again (byte-identical
    // file) and the lead (its project entry gains the key beside allowedTools).
    await setFakeHerdrBehavior(fake, { trustDialog: undefined });
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Seat trust", "--json"], fake.env);
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    expect((await creates()).length).toBe(3);
    expect(await readFile(seatJson("team-supervisor"), "utf8")).toBe(supervisorText);
    expect(await readSeat("lead")).toEqual({
      ...leadBefore,
      projects: { ...leadBefore.projects, [projectKey]: { allowedTools: ["Bash(git:*)"], hasTrustDialogAccepted: true } },
    });
    expect(await readFile(seatJson("peer"), "utf8")).toBe(peerText);

    // work add --to a Claude peer seeds the peer dir the same way.
    const lead = envelope<{ team: { roles: Array<{ paneId: string; role: string }> } }>(started).team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const security = await runCliAt(fixture, fixture.repo, ["work", "add", "review it", "--to", "peer-reviewer-security", "--json"], leadEnvironment);
    expect(phaseFree(security.stderr)).toBe("");
    expect(security.exitCode).toBe(0);
    expect(await readSeat("peer")).toEqual({ ...peerBefore, projects: { [projectKey]: { hasTrustDialogAccepted: true } } });

    // A Codex peer has no Claude seat dir: no seat file changes.
    const snapshot = async () => Promise.all(["lead", "peer", "team-supervisor"].map((seat) => readFile(seatJson(seat), "utf8")));
    const before = await snapshot();
    const codex = await runCliAt(fixture, fixture.repo, ["work", "add", "codex item", "--to", "x", "--json"], leadEnvironment);
    expect(phaseFree(codex.stderr)).toBe("");
    expect(codex.exitCode).toBe(0);
    expect((await creates()).length).toBe(5);
    expect(await snapshot()).toEqual(before);
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);

// seat-config-dirs R16 (d855 rule 5): the trust seed was a plain
// read-modify-write shared by every peer's Claude (SEC-4) and left the file at
// whatever mode it had (SEC-5); it now lands through a 0600 temp file in the
// seat dir renamed over .claude.json, so a reader never sees a partial file.
test("seat-dirs-trust-atomic: after team start no <seat>/.claude.json.tmp-* remains in any seat dir and every seeded .claude.json is 0600 even when it was 0644 before, with its content intact (R16, d855)", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "profiles", "lead.md"),
      "---\nharness: claude\nmodel: default\ndescription: claude lead\n---\nRole: Lead (claude shadow).\n",
    );
    const fake = await installFakeHerdr(fixture);
    const seatRoot = seatConfigRoot(fixture.home);
    const seatJson = (seat: string) => join(seatRoot, seat, ".claude.json");
    const projectKey = await realpath(fixture.repo);
    const leadBefore = { hasCompletedOnboarding: true, mcpServers: {}, numStartups: 3 };
    // The fake install already created the file 0600; stage the pre-fix mode.
    await writeFile(seatJson("lead"), `${JSON.stringify(leadBefore, null, 2)}\n`);
    await chmod(seatJson("lead"), 0o644);

    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Seat trust atomic", "--json"], fake.env);
    expect(phaseFree(started.stderr)).toBe("");
    expect(started.exitCode).toBe(0);
    for (const seat of ["lead", "team-supervisor"]) {
      const entries = (await readdir(join(seatRoot, seat))).filter((entry) => entry.startsWith(".claude.json."));
      expect({ seat, entries }).toEqual({ seat, entries: [] });
      expect({ seat, mode: (await stat(seatJson(seat))).mode & 0o777 }).toEqual({ seat, mode: 0o600 });
    }
    expect(JSON.parse(await readFile(seatJson("lead"), "utf8"))).toEqual({
      ...leadBefore,
      projects: { [projectKey]: { hasTrustDialogAccepted: true } },
    });
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);
