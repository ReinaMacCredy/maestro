import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { randomUUID } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { slpWatchRuntimeDirectory } from "../src/plugins/slp-watch.ts";
import {
  runCliAt,
  withFixture,
  writeConfig,
  type CliResult,
  type Fixture,
} from "./helpers.ts";

const maestroCli = join(import.meta.dir, "..", "bin", "maestro.ts");
const roleCommandRunner = join(import.meta.dir, "slp-role-command.ts");
const watchOpener = join(import.meta.dir, "slp-open-watch.ts");

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
  if (receipt.exitCode !== 0) {
    throw new Error(
      `${name} command failed (${receipt.exitCode}): ${receipt.command.join(" ")}\n` +
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
        const watchReceipt = await promptRoleCommand(fixture, supervisor.name, [
          process.execPath,
          watchOpener,
          supervisor.paneId,
          fixture.repo,
          started.team.teamId,
          String(started.team.generation),
          started.team.workspaceId,
          join(import.meta.dir, "..", "bin", "maestro-slp-watch.ts"),
        ]);
        expect(JSON.parse(watchReceipt!.stdout)).toEqual({ paneId: expect.any(String) });
        const runtimeDirectory = slpWatchRuntimeDirectory(
          fixture.repo,
          started.team.teamId,
          started.team.generation,
        );
        const transcriptPath = join(runtimeDirectory, "transcript.txt");
        await waitFor(() => existsSync(transcriptPath), "Watch did not create its live transcript");

        await promptAgent(fixture, lead.name, ["work", "take", started.work.id, "--json"]);
        await promptAgent(fixture, lead.name, [
          "work",
          "return",
          started.work.id,
          "result: initial objective complete; proof: live Lead pane",
          "--json",
        ]);
        await promptAgent(fixture, supervisor.name, ["work", "accept", started.work.id, "--json"]);

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
          await promptAgent(fixture, work.assigned_to, ["work", "take", work.id, "--json"]);
          await promptAgent(fixture, work.assigned_to, [
            "work",
            "note",
            work.id,
            "proof: direct note from this live Peer pane",
            "--json",
          ]);
          await promptAgent(fixture, work.assigned_to, [
            "work",
            "return",
            work.id,
            "result: independent result complete; proof: live Peer pane",
            "--json",
          ]);
          await promptAgent(fixture, lead.name, ["work", "accept", work.id, "--json"]);
        }
        await waitFor(async () => {
          if (!existsSync(transcriptPath)) return false;
          const transcript = await readFile(transcriptPath, "utf8");
          return [supervisor.name, lead.name, ...peerWork.map((work) => work.assigned_to)]
            .every((name) => transcript.includes(name));
        }, "Watch did not render every live SLP role");

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
          teams: Array<{ missingPanes: string[]; teamId: string; watch: string }>;
        }>(
          await runCliAt(fixture, room, ["status", "--json"], liveEnvironment),
        );
        expect(hubStatus.teams).toContainEqual(
          expect.objectContaining({ missingPanes: [], teamId, watch: "on" }),
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
