import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function packetHeads(result: { stdout: string }): Map<string, string> {
  const detections = (JSON.parse(result.stdout) as {
    data: { detections: Array<{ kind: string; packet: string }> };
  }).data.detections;
  return new Map(
    detections.map(({ kind, packet }) => [kind, packet.split("\n")[0] ?? ""]),
  );
}

test("403 uninstall removes only the repository's absolute registry line", async () => {
  await withFixture(async (fixture) => {
    const installed = await runCli(fixture, ["install"]);
    expect(installed.exitCode).toBe(0);
    const registry = join(fixture.home, "maestro", "registry");
    const otherRepo = join(fixture.root, "other-repo");
    await writeFile(registry, `${fixture.repo}\n${otherRepo}\n`);

    const uninstalled = await runCli(fixture, ["uninstall"]);
    expect(uninstalled.exitCode).toBe(0);
    expect(await readFile(registry, "utf8")).toBe(`${otherRepo}\n`);
  });
});

test("404 brief skips registered paths that are absent or have no .maestro", async () => {
  await withFixture(async (fixture) => {
    const missingRepo = join(fixture.root, "missing-repo");
    const bareRepo = join(fixture.root, "bare-repo");
    await mkdir(bareRepo, { recursive: true });
    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(
      join(fixture.home, "maestro", "registry"),
      `${missingRepo}\n${bareRepo}\n`,
    );

    const brief = await runCli(fixture, ["brief"]);
    expect(brief).toEqual({
      exitCode: 0,
      stderr: "",
      stdout:
        `Needs attention:\nskipped: ${missingRepo} (missing)\n` +
        `skipped: ${bareRepo} (missing)\n`,
    });
  });
});

test("405 room forget removes a registry line without uninstalling repository wiring", async () => {
  await withFixture(async (fixture) => {
    const installed = await runCli(fixture, ["install"]);
    expect(installed.exitCode).toBe(0);
    const hook = join(fixture.repo, ".codex", "hooks", "maestro-record.ts");
    expect(existsSync(hook)).toBe(true);

    const forgotten = await runCli(fixture, ["room", "forget", fixture.repo]);
    expect(forgotten).toEqual({
      exitCode: 0,
      stderr: "",
      stdout: `forgot: ${fixture.repo}\n`,
    });
    expect(await readFile(join(fixture.home, "maestro", "registry"), "utf8")).toBe("");
    expect(existsSync(hook)).toBe(true);

    const help = await runCli(fixture, ["help", "room"]);
    expect(help.exitCode).toBe(0);
    expect(help.stdout).toContain("room forget <path>");
  });
});

test("407 attention and briefs prefix work subjects with their kind", async () => {
  await withFixture(async (fixture) => {
    const stalled = idFrom(
      await runCli(fixture, ["work", "add", "stalled work", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", stalled], session("stalled-holder"))).exitCode)
      .toBe(0);

    const repeated = idFrom(
      await runCli(fixture, ["work", "add", "repeated work", "--atomic-reason", "fixture"]),
    );
    expect(
      (await runCli(fixture, ["work", "start", repeated], session("repeated-holder"))).exitCode,
    ).toBe(0);
    for (const note of ["failed: first", "failed: second", "failed: third"]) {
      expect(
        (await runCli(fixture, ["work", "note", repeated, note], session("repeated-holder")))
          .exitCode,
      ).toBe(0);
    }

    const parent = idFrom(
      await runCli(fixture, ["work", "add", "collision parent", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", parent], session("parent-holder"))).exitCode)
      .toBe(0);
    const first = idFrom(
      await runCli(fixture, ["work", "add", "first collision", "--parent", parent]),
    );
    const second = idFrom(
      await runCli(fixture, ["work", "add", "second collision", "--parent", parent]),
    );
    expect((await runCli(fixture, ["work", "start", first], session("first-holder"))).exitCode)
      .toBe(0);
    expect((await runCli(fixture, ["work", "start", second], session("second-holder"))).exitCode)
      .toBe(0);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - 31 * 60_000).toISOString(), "stalled-holder");
    database.close();

    const attention = await runCli(fixture, ["attention", "--json"]);
    expect(attention.exitCode).toBe(0);
    const heads = packetHeads(attention);
    expect(heads.get("STALLED_LEASE")).toBe(`attention STALLED_LEASE work ${stalled}`);
    expect(heads.get("REPEATED_FAILURE")).toBe(`attention REPEATED_FAILURE work ${repeated}`);
    expect(heads.get("SCOPE_COLLISION")).toBe(
      `attention SCOPE_COLLISION work ${first},${second}`,
    );

    const hook = await runCli(fixture, ["hook", "record", "--event", "UserPromptSubmit"]);
    expect(hook.stdout).toContain(`attention STALLED_LEASE work ${stalled}`);
    expect(hook.stdout).toContain(`attention SCOPE_COLLISION work ${first},${second}`);

    await mkdir(join(fixture.home, "maestro"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "registry"), `${fixture.repo}\n`);
    const brief = await runCli(fixture, ["brief"], { MAESTRO_READ_ONLY: "1" });
    expect(brief.stdout).toContain(`attention REPEATED_FAILURE work ${repeated}`);
  });
});

test("409 attention and hook briefs prefix decision subjects with their kind", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "stale decision", "--atomic-reason", "fixture"]),
    );
    const decision = idFrom(
      await runCli(fixture, ["decision", "draft", "choose the boundary", "--work", work]),
    );
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE decisions SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
    database.close();

    const attention = await runCli(fixture, ["attention", "--json"]);
    expect(attention.exitCode).toBe(0);
    expect(packetHeads(attention).get("DECISION_STALE")).toBe(
      `attention DECISION_STALE decision ${decision}`,
    );

    const hook = await runCli(fixture, ["hook", "record", "--event", "UserPromptSubmit"]);
    expect(hook.stdout).toContain(`attention DECISION_STALE decision ${decision}`);
  });
});
