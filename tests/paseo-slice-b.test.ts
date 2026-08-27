import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  idFrom,
  prepareInstallFixture,
  runCli,
  runInstalledCliAt,
  type Fixture,
  withFixture,
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
    "Route repeated failure attention",
    "--owned-scope",
    "fixture",
    "--excluded-scope",
    "product source",
    "--mutation",
    "no-write",
    "--stop-condition",
    "routing is observable",
    "--lane",
    "delivery",
    "--evidence-required",
    "source: CLI output",
    "--pane",
    "w1:p390",
  ];
}

function dispatchId(stdout: string): string {
  const match = stdout.match(/^(x\d+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${stdout}`);
  return match[1];
}

async function addFailedNotes(
  fixture: Fixture,
  work: string,
  environment: Record<string, string>,
): Promise<void> {
  for (const note of ["failed: first", "failed: second", "failed: third"]) {
    expect((await runCli(fixture, ["work", "note", work, note], environment)).exitCode).toBe(0);
  }
}

function sha256(content: string): string {
  return createHash("sha256").update(content).digest("hex");
}

test("390 REPEATED_FAILURE routes by the current holder role", async () => {
  await withFixture(async (fixture) => {
    const peerWork = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "peer failure episode",
        "--atomic-reason",
        "routing fixture",
      ]),
    );
    const leadWork = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "lead failure episode",
        "--atomic-reason",
        "routing fixture",
      ]),
    );
    const peer = session("peer-holder");
    const lead = session("lead-holder");
    const opened = await runCli(fixture, dispatchOpenArgs(peerWork));
    expect(opened.exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatchId(opened.stdout)], peer)).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "start", peerWork], peer)).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "start", leadWork], lead)).exitCode).toBe(0);
    await addFailedNotes(fixture, peerWork, peer);
    await addFailedNotes(fixture, leadWork, lead);

    const hook = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      session("lead-viewer"),
    );
    expect(hook.exitCode).toBe(0);
    expect(hook.stdout).toContain(`attention REPEATED_FAILURE ${peerWork}`);
    expect(hook.stdout).not.toContain(`attention REPEATED_FAILURE ${leadWork}`);

    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const room = await runCli(fixture, ["brief"], { MAESTRO_READ_ONLY: "1" });
    expect(room.exitCode).toBe(0);
    expect(room.stdout).not.toContain(`attention REPEATED_FAILURE ${peerWork}`);
    expect(room.stdout).toContain(`attention REPEATED_FAILURE ${leadWork}`);

    const attention = await runCli(fixture, ["attention", "--json"], session("scanner"));
    expect(attention.exitCode).toBe(0);
    const findings = (JSON.parse(attention.stdout) as {
      data: {
        detections: Array<{
          holderRole?: string;
          kind: string;
          packet: string;
          route?: string;
          subjectWork: string | null;
        }>;
      };
    }).data.detections.filter((finding) => finding.kind === "REPEATED_FAILURE");
    expect(findings).toContainEqual(
      expect.objectContaining({
        holderRole: "peer",
        route: "lead",
        subjectWork: peerWork,
        packet: expect.stringContaining("holder role: peer"),
      }),
    );
    expect(findings).toContainEqual(
      expect.objectContaining({
        holderRole: "lead",
        route: "supervisor",
        subjectWork: leadWork,
        packet: expect.stringContaining("holder role: lead"),
      }),
    );
  });
});

test("391 install creates the private room deny list and doctor reports it", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const settingsPath = join(fixture.home, "maestro", ".claude", "settings.json");
    const settings = JSON.parse(await readFile(settingsPath, "utf8")) as {
      permissions?: { deny?: string[] };
    };
    expect(settings.permissions?.deny).toEqual(["Agent", "Task"]);
    expect((await stat(settingsPath)).mode & 0o777).toBe(0o600);

    const doctor = await runInstalledCliAt(fixture, fixture.repo, ["doctor"], { PATH: path });
    expect(doctor.exitCode).toBe(0);
    expect(doctor.stdout).toContain("room deny list: ok");
  });
});

test("392 install merges room deny entries without losing foreign settings", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const settingsPath = join(fixture.home, "maestro", ".claude", "settings.json");
    await mkdir(join(fixture.home, "maestro", ".claude"), { recursive: true });
    await writeFile(
      settingsPath,
      `${JSON.stringify({
        foreign: { retained: true },
        permissions: { allow: ["Read"], deny: ["CustomDeny"] },
      }, null, 2)}\n`,
    );

    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const settings = JSON.parse(await readFile(settingsPath, "utf8")) as {
      foreign?: { retained?: boolean };
      permissions?: { allow?: string[]; deny?: string[] };
    };
    expect(settings.foreign).toEqual({ retained: true });
    expect(settings.permissions?.allow).toEqual(["Read"]);
    expect(settings.permissions?.deny).toEqual(["CustomDeny", "Agent", "Task"]);
  });
});

test("393 a second install leaves the room settings hash unchanged", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const settingsPath = join(fixture.home, "maestro", ".claude", "settings.json");
    const firstHash = sha256(await readFile(settingsPath, "utf8"));

    expect(
      (await runInstalledCliAt(fixture, fixture.repo, ["install"], { PATH: path })).exitCode,
    ).toBe(0);
    expect(sha256(await readFile(settingsPath, "utf8"))).toBe(firstHash);
  });
});

test("394 doctor reports a missing room deny list", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const settingsPath = join(fixture.home, "maestro", ".claude", "settings.json");
    const settings = JSON.parse(await readFile(settingsPath, "utf8")) as {
      permissions?: { deny?: string[] };
    };
    settings.permissions = { deny: ["Task"] };
    await writeFile(settingsPath, `${JSON.stringify(settings, null, 2)}\n`);

    const doctor = await runInstalledCliAt(fixture, fixture.repo, ["doctor"], { PATH: path });
    expect(doctor.exitCode).toBe(0);
    expect(doctor.stdout).toContain("room deny list: missing");
  });
});
