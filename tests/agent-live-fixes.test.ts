import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, prepareInstallFixture, runCli, withFixture } from "./helpers.ts";

test("91 empty ready points at work add", async () => {
  await withFixture(async (fixture) => {
    const empty = await runCli(fixture, ["ready"]);
    expect(empty.exitCode).toBe(0);
    expect(empty.stdout).toContain("no ready work");
    expect(empty.stdout).toContain('maestro work add "<title>"');
  });
});

test("92 gate envelope carries origin without a duplicated reason; lease keeps command", async () => {
  await withFixture(async (fixture) => {
    const gated = idFrom(await runCli(fixture, ["work", "add", "gated root", "--kind", "feature"]));
    const blocked = await runCli(fixture, ["work", "start", gated]);
    expect(blocked.exitCode).not.toBe(0);
    const gateError = (JSON.parse(blocked.stderr) as { error: Record<string, unknown> }).error;
    expect(gateError.code).toBe("GATE_BLOCKED");
    expect(gateError.origin).toBe("policy-breakdown");
    expect(typeof gateError.message).toBe("string");
    expect("reason" in gateError).toBe(false);

    const loose = idFrom(
      await runCli(fixture, ["work", "add", "atomic item", "--kind", "task", "--atomic-reason", "spike"]),
    );
    const unleased = await runCli(fixture, ["work", "done", loose, "--evidence", "done"]);
    expect(unleased.exitCode).not.toBe(0);
    const leaseError = (JSON.parse(unleased.stderr) as { error: Record<string, unknown> }).error;
    expect(leaseError.code).toBe("LEASE_REQUIRED");
    expect(leaseError.command).toBe(`maestro work start ${loose}`);
  });
});

test("93 breakdown structure is visible: show lists children, list indents them, ready annotates the parent", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]));
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child task", "--kind", "task", "--parent", parent]),
    );

    const show = await runCli(fixture, ["work", "show", parent]);
    expect(show.exitCode).toBe(0);
    expect(show.stdout).toContain(`child: ${child} [open] child task`);

    const list = await runCli(fixture, ["work", "list"]);
    expect(list.exitCode).toBe(0);
    expect(list.stdout).toContain(`${parent} [open] parent feature`);
    expect(list.stdout).toContain(`\n  ${child} [open] child task`);

    const ready = await runCli(fixture, ["ready", "--json"]);
    const data = JSON.parse(ready.stdout).data as {
      gated: Array<{ id: string; origin: string }>;
      works: Array<{ id: string }>;
    };
    expect(data.works.map((work) => work.id)).toEqual([child]);
    expect(data.gated.map((work) => work.id)).toEqual([parent]);
    expect(data.gated[0]?.origin).toBe("policy-breakdown");
  });
});

test("94 a parent with open children cannot start or complete", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]));
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child task", "--kind", "task", "--parent", parent]),
    );

    const start = await runCli(fixture, ["work", "start", parent]);
    expect(start.exitCode).not.toBe(0);
    const startError = (JSON.parse(start.stderr) as { error: Record<string, unknown> }).error;
    expect(startError.code).toBe("GATE_BLOCKED");
    expect(startError.origin).toBe("policy-breakdown");
    expect(String(startError.message)).toContain(child);
    expect(String(startError.message)).toContain(`maestro work start ${child}`);

    // Reach the done gate via a parent started before its child existed.
    const grown = idFrom(
      await runCli(fixture, ["work", "add", "grown scope", "--kind", "task", "--atomic-reason", "spike"]),
    );
    expect((await runCli(fixture, ["work", "start", grown])).exitCode).toBe(0);
    const late = idFrom(
      await runCli(fixture, ["work", "add", "late child", "--kind", "task", "--parent", grown]),
    );
    const done = await runCli(fixture, ["work", "done", grown, "--evidence", "done"]);
    expect(done.exitCode).not.toBe(0);
    const doneError = (JSON.parse(done.stderr) as { error: Record<string, unknown> }).error;
    expect(doneError.code).toBe("GATE_BLOCKED");
    expect(String(doneError.message)).toContain(late);

    // Completing the child reopens the parent path.
    expect((await runCli(fixture, ["work", "start", late])).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "done", late, "--evidence", "child done"])).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "done", grown, "--evidence", "done"])).exitCode).toBe(0);
  });
});

test("95 status reflects completion instead of pinning the last start", async () => {
  await withFixture(async (fixture) => {
    const item = idFrom(
      await runCli(fixture, ["work", "add", "atomic item", "--kind", "task", "--atomic-reason", "spike"]),
    );
    expect((await runCli(fixture, ["work", "start", item])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", item, "--evidence", "done"])).exitCode).toBe(0);

    const status = await runCli(fixture, ["status"]);
    expect(status.exitCode).toBe(0);
    expect(status.stdout).toContain("work.done");
    expect(status.stdout).not.toContain("work.start pid");
  });
});

test("96 installed harness docs name the stderr JSON failure envelope", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const block = await readFile(join(fixture.repo, "CLAUDE.md"), "utf8");
    expect(block).toContain("JSON error envelope");
    expect(block).toContain("stderr");
  });
});

test("97 work recipe disambiguates claim tagging and evidence forms", async () => {
  await withFixture(async (fixture) => {
    const recipe = await runCli(fixture, ["recipe", "show", "work"]);
    expect(recipe.exitCode).toBe(0);
    expect(recipe.stdout).toContain("the tag prefix goes on the claim");
    expect(recipe.stdout).toContain("opaque");
  });
});
