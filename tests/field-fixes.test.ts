import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { renderedProfilePath } from "../src/plugins/profiles.ts";
import { prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

const OWNER = { MAESTRO_SESSION_ID: "eperm-owner", MAESTRO_SESSION_PID: "1" };
const PEER = {
  MAESTRO_SESSION_ID: "reader-peer",
  MAESTRO_SESSION_PID: String(process.pid),
};

test("61 EPERM-only pids stay alive while dead pids still expire", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(
      fixture,
      [
        "work",
        "add",
        "eperm victim",
        "--acceptance",
        "lease survives foreign reads",
        "--atomic-reason",
        "single-step probe",
      ],
      OWNER,
    );
    expect(added.exitCode).toBe(0);

    const started = await runCli(fixture, ["work", "start", "w1"], OWNER);
    expect(started.exitCode).toBe(0);

    // pid 1 is alive but unsignalable from an unprivileged checker (EPERM);
    // a peer's read verbs must not expire the lease.
    const listed = await runCli(fixture, ["work", "list"], PEER);
    expect(listed.exitCode).toBe(0);
    const status = await runCli(fixture, ["status"], PEER);
    expect(status.stdout).toContain("eperm-owner");
    expect(status.stdout).not.toContain("eperm-owner [dead]");

    const shown = await runCli(fixture, ["work", "show", "w1"], PEER);
    expect(shown.stdout).toContain("active");

    const done = await runCli(
      fixture,
      [
        "work",
        "done",
        "w1",
        "--claim",
        "lease held across foreign reads",
        "--proof",
        "peer work list/status ran between start and done",
      ],
      OWNER,
    );
    expect(done.exitCode).toBe(0);
    expect(done.stdout).toContain("w1 done");

    // A genuinely dead pid must still expire immediately.
    const corpse = Bun.spawnSync(["sh", "-c", "exit 0"]);
    const deadPid = String(corpse.pid);
    const DEAD = { MAESTRO_SESSION_ID: "dead-owner", MAESTRO_SESSION_PID: deadPid };
    const added2 = await runCli(
      fixture,
      [
        "work",
        "add",
        "dead victim",
        "--acceptance",
        "lease expires",
        "--atomic-reason",
        "single-step probe",
      ],
      DEAD,
    );
    expect(added2.exitCode).toBe(0);
    const started2 = await runCli(fixture, ["work", "start", "w2"], DEAD);
    expect(started2.exitCode).toBe(0);

    await runCli(fixture, ["work", "list"], PEER);
    const shown2 = await runCli(fixture, ["work", "show", "w2"], PEER);
    expect(shown2.stdout).toContain("open");
    expect(shown2.stdout).not.toContain("active");
  });
});

test("62 work cancel is terminal, evidenced, and unblocks dependents", async () => {
  await withFixture(async (fixture) => {
    const add = (title: string, extra: string[] = []) =>
      runCli(
        fixture,
        [
          "work",
          "add",
          title,
          "--acceptance",
          "n/a",
          "--atomic-reason",
          "probe",
          ...extra,
        ],
        PEER,
      );

    expect((await add("orphan stub")).exitCode).toBe(0); // w1
    expect((await add("blocker item")).exitCode).toBe(0); // w2
    expect((await add("dependent item", ["--blocked-by", "w2"])).exitCode).toBe(0); // w3
    expect((await add("done item")).exitCode).toBe(0); // w4

    // --reason is required.
    const bare = await runCli(fixture, ["work", "cancel", "w1"], PEER);
    expect(bare.exitCode).not.toBe(0);

    const cancelled = await runCli(
      fixture,
      ["work", "cancel", "w1", "--reason", "duplicate of w6 created while fixing flags"],
      PEER,
    );
    expect(cancelled.exitCode).toBe(0);

    const list = await runCli(fixture, ["work", "list", "--all"], PEER);
    expect(list.stdout).toContain("[cancelled] orphan stub");

    type Listed = { blockedBy: Array<{ id: string; state: string }>; gate: unknown; id: string; title: string };
    const listed = async (): Promise<Listed[]> => {
      const result = await runCli(fixture, ["work", "list", "--json"], PEER);
      expect(result.exitCode).toBe(0);
      return (JSON.parse(result.stdout).data as { works: Listed[] }).works;
    };
    const startable = (works: Listed[]) =>
      works.filter((work) => work.blockedBy.length === 0 && work.gate === null).map((work) => work.title);
    const openWork = await listed();
    expect(startable(openWork)).not.toContain("orphan stub");
    expect(openWork.find((work) => work.id === "w3")?.blockedBy).toEqual([{ id: "w2", state: "open" }]);

    const startCancelled = await runCli(fixture, ["work", "start", "w1"], PEER);
    expect(startCancelled.exitCode).not.toBe(0);
    expect(startCancelled.stderr + startCancelled.stdout).toContain("cancel");

    const doneCancelled = await runCli(
      fixture,
      ["work", "done", "w1", "--claim", "x", "--proof", "y"],
      PEER,
    );
    expect(doneCancelled.exitCode).not.toBe(0);

    const twice = await runCli(
      fixture,
      ["work", "cancel", "w1", "--reason", "again"],
      PEER,
    );
    expect(twice.exitCode).not.toBe(0);

    const shown = await runCli(fixture, ["work", "show", "w1"], PEER);
    expect(shown.stdout).toContain("cancelled");
    expect(shown.stdout).toContain("duplicate of w6");

    // A cancelled blocker resolves its dependents.
    const before = await listed();
    expect(startable(before)).not.toContain("dependent item");
    expect(before.find((work) => work.id === "w3")?.blockedBy).toEqual([{ id: "w2", state: "open" }]);
    const cancelBlocker = await runCli(
      fixture,
      ["work", "cancel", "w2", "--reason", "blocker abandoned"],
      PEER,
    );
    expect(cancelBlocker.exitCode).toBe(0);
    const after = await listed();
    expect(startable(after)).toContain("dependent item");
    expect(after.every((work) => work.blockedBy.length === 0 && work.gate === null)).toBe(true);

    // A done item refuses cancel.
    expect((await runCli(fixture, ["work", "start", "w4"], PEER)).exitCode).toBe(0);
    const doneOk = await runCli(
      fixture,
      ["work", "done", "w4", "--claim", "c", "--proof", "p"],
      PEER,
    );
    expect(doneOk.exitCode).toBe(0);
    const cancelDone = await runCli(
      fixture,
      ["work", "cancel", "w4", "--reason", "too late"],
      PEER,
    );
    expect(cancelDone.exitCode).not.toBe(0);

    // The current holder can abandon active work without recording a false completion.
    expect((await add("held item")).exitCode).toBe(0); // w5
    expect((await runCli(fixture, ["work", "start", "w5"], PEER)).exitCode).toBe(0);
    const cancelHeld = await runCli(
      fixture,
      ["work", "cancel", "w5", "--reason", "mid-flight"],
      PEER,
    );
    expect(cancelHeld.exitCode).toBe(0);
    const shownHeld = await runCli(fixture, ["work", "show", "w5"], PEER);
    expect(shownHeld.stdout).toContain("[cancelled] held item");
    expect(shownHeld.stdout).not.toContain("held by:");
  });
});

test("63 [lint] install leaves repository instructions owner-managed", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    expect(await Bun.file(join(fixture.repo, "AGENTS.md")).exists()).toBe(false);
    expect(await Bun.file(join(fixture.repo, "CLAUDE.md")).exists()).toBe(false);
    expect(await Bun.file(join(fixture.home, "maestro", "SLP.md")).exists()).toBe(true);
  });
});

test("289 [lint] recipe and installed Hub expose the same simplified SLP roles", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const recipe = await runCli(fixture, ["recipe", "show", "slp"]);
    expect(recipe.exitCode).toBe(0);
    for (const heading of ["### Hub Supervisor", "### Lead", "### Peer"]) {
      expect(recipe.stdout).toContain(heading);
    }
    expect(recipe.stdout).not.toContain("### Team Supervisor");
    expect(recipe.stdout).toContain("OPEN -> ACTIVE -> RETURNED -> DONE");
    expect(recipe.stdout).toContain("~/maestro/SLP.md");
    expect(recipe.stdout).not.toContain("advisor-<team>");
    expect(recipe.stdout).not.toContain("observer-<team>");

    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const room = await readFile(join(fixture.home, "maestro", "AGENTS.md"), "utf8");
    const pack = await readFile(join(fixture.home, "maestro", "SLP.md"), "utf8");
    expect(room).toContain("This workspace is the Hub Supervisor.");
    expect(room).toContain("read `SLP.md`");
    // Hub d91/d98: pack version 3 names profiles; the seat mandate lives in
    // the rendered profile, not in the pack.
    expect(pack).toContain("<!-- slp:version=3 -->");
    expect(pack).toContain("<!-- slp:profile:lead=lead -->");
    expect(pack).not.toContain("slp:profile:team-supervisor");
    expect(pack).not.toContain("## Team Supervisor");
    expect(
      await readFile(renderedProfilePath(fixture.home, "claude", "lead"), "utf8"),
    ).toContain("Role: Lead.");
  });
});
