import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
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
    const ready = await runCli(fixture, ["ready"], PEER);
    expect(ready.exitCode).toBe(0);
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
        "peer ready/status ran between start and done",
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

    await runCli(fixture, ["ready"], PEER);
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

    const list = await runCli(fixture, ["work", "list"], PEER);
    expect(list.stdout).toContain("[cancelled] orphan stub");

    const ready = await runCli(fixture, ["ready", "--json"], PEER);
    expect(ready.exitCode).toBe(0);
    const readyData = JSON.parse(ready.stdout).data as {
      gated: Array<{ id: string; origin: string }>;
      works: Array<{ title: string }>;
    };
    expect(readyData.works.map((work) => work.title)).not.toContain("orphan stub");
    expect(readyData.gated).toContainEqual(expect.objectContaining({
      id: "w3",
      origin: "work-blockers",
    }));

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
    const readyBefore = await runCli(fixture, ["ready", "--json"], PEER);
    expect(readyBefore.exitCode).toBe(0);
    const beforeData = JSON.parse(readyBefore.stdout).data as {
      gated: Array<{ id: string; origin: string }>;
      works: Array<{ title: string }>;
    };
    expect(beforeData.works.map((work) => work.title)).not.toContain("dependent item");
    expect(beforeData.gated).toContainEqual(expect.objectContaining({
      id: "w3",
      origin: "work-blockers",
    }));
    const cancelBlocker = await runCli(
      fixture,
      ["work", "cancel", "w2", "--reason", "blocker abandoned"],
      PEER,
    );
    expect(cancelBlocker.exitCode).toBe(0);
    const readyAfter = await runCli(fixture, ["ready", "--json"], PEER);
    expect(readyAfter.exitCode).toBe(0);
    const afterData = JSON.parse(readyAfter.stdout).data as {
      gated: unknown[];
      works: Array<{ title: string }>;
    };
    expect(afterData.works.map((work) => work.title)).toContain("dependent item");
    expect(afterData.gated).toEqual([]);

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

test("63 [lint] install mirrors teach the work lifecycle", async () => {
  await withFixture(async (fixture) => {
    // Proves mirror documentation content, not that an agent loads or follows the lifecycle.
    const { path } = await prepareInstallFixture(fixture);

    const installed = await runCli(fixture, ["install"], { PATH: path });
    const agents = await readFile(join(fixture.repo, "AGENTS.md"), "utf8");
    const claude = await readFile(join(fixture.repo, "CLAUDE.md"), "utf8");

    expect(installed.exitCode).toBe(0);
    for (const mirror of [agents, claude]) {
      expect(mirror).toContain("maestro work add");
      expect(mirror).toContain("maestro recipe show work");
    }
  });
});

test("289 [lint] recipe slp presents the SLP roles and install mirrors state the role bindings", async () => {
  await withFixture(async (fixture) => {
    // Proves doctrine text, not runtime enforcement of dispatch or session role ownership.
    const { path } = await prepareInstallFixture(fixture);
    const recipe = await runCli(fixture, ["recipe", "show", "slp"]);
    expect(recipe.exitCode).toBe(0);
    for (const heading of ["### Human", "### Supervisor", "### Lead", "### Peer"]) {
      expect(recipe.stdout).toContain(heading);
    }
    for (const status of [
      "DONE",
      "BLOCKED",
      "UNTESTABLE",
      "UNKNOWN",
      "FAILED",
      "CHALLENGE",
      "REOPEN_REQUEST",
      "DEPENDENCY_REQUEST",
      "COUNCIL_REQUEST",
    ]) {
      expect(recipe.stdout).toContain(status);
    }
    expect(recipe.stdout).toContain("## How maestro binds a session to a role");
    expect(recipe.stdout).toContain("HANDBACK_UNREVIEWED");
    expect(recipe.stdout).toContain("HUMAN_DECISION_REQUIRED");
    expect(recipe.stdout).toContain("LEAD_COLLISION");

    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    for (const name of ["AGENTS.md", "CLAUDE.md"]) {
      const mirror = await readFile(join(fixture.repo, name), "utf8");
      expect(mirror).toContain(
        "A session in this repository is its Lead; panes it opens with a dispatch are Peers; the room at ~/maestro is the Supervisor. Roles: `maestro recipe show slp`.",
      );
    }
    const room = await readFile(join(fixture.home, "maestro", "AGENTS.md"), "utf8");
    expect(room).toContain("This room is the Supervisor; roles: `maestro recipe show slp`.");
  });
});
