import { expect, test } from "bun:test";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  addLinkedWorktree,
  initializeGitRepository,
  runCli,
  runCliAt,
  withFixture,
  type Fixture,
} from "./helpers.ts";

function sessionEnvironment(id: string, pid = process.pid): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(pid),
  };
}

async function linkedWorktree(fixture: Fixture, name: string): Promise<string> {
  await initializeGitRepository(fixture.repo);
  const path = join(fixture.root, name);
  await addLinkedWorktree(fixture.repo, path);
  return path;
}

test("B3.1 work is shared between the main checkout and a linked worktree", async () => {
  await withFixture(async (fixture) => {
    const worktree = await linkedWorktree(fixture, "linked-one");

    expect((await runCliAt(fixture, worktree, ["work", "add", "from linked"])).exitCode).toBe(0);
    const listedInMain = await runCli(fixture, ["work", "list"]);
    expect(listedInMain.stdout).toContain("from linked");

    expect((await runCli(fixture, ["work", "add", "from main"])).exitCode).toBe(0);
    const listedInWorktree = await runCliAt(fixture, worktree, ["work", "list"]);
    expect(listedInWorktree.stdout).toContain("from linked");
    expect(listedInWorktree.stdout).toContain("from main");
  });
});

test("B3.2 mailbox delivery and cursor state are shared across worktrees", async () => {
  await withFixture(async (fixture) => {
    const worktree = await linkedWorktree(fixture, "linked-mailbox");
    expect(
      (
        await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
          ...sessionEnvironment("main-target"),
        })
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(fixture, worktree, ["msg", "send", "main-target", "cross-worktree"])
      ).exitCode,
    ).toBe(0);

    const first = await runCli(fixture, ["msg", "read"], sessionEnvironment("main-target"));
    const second = await runCli(fixture, ["msg", "read"], sessionEnvironment("main-target"));
    expect(first.stdout).toContain("cross-worktree");
    expect(second.stdout).not.toContain("cross-worktree");
  });
});

test("B3.3 shared stores stay isolated between repositories", async () => {
  await withFixture(async (fixture) => {
    const firstWorktree = await linkedWorktree(fixture, "first-repo-worktree");
    const secondRepo = join(fixture.root, "second-repo");
    await mkdir(secondRepo, { recursive: true });
    await initializeGitRepository(secondRepo);
    const secondWorktree = join(fixture.root, "second-repo-worktree");
    await addLinkedWorktree(secondRepo, secondWorktree);

    expect(
      (await runCliAt(fixture, firstWorktree, ["work", "add", "first repository item"]))
        .exitCode,
    ).toBe(0);
    expect(
      (await runCliAt(fixture, secondWorktree, ["work", "add", "second repository item"]))
        .exitCode,
    ).toBe(0);

    const firstList = await runCliAt(fixture, fixture.repo, ["work", "list"]);
    const secondList = await runCliAt(fixture, secondRepo, ["work", "list"]);
    expect(firstList.stdout).toContain("first repository item");
    expect(firstList.stdout).not.toContain("second repository item");
    expect(secondList.stdout).toContain("second repository item");
    expect(secondList.stdout).not.toContain("first repository item");
  });
});

test("B3.4 a linked-worktree private store is advised once per invocation and left untouched", async () => {
  await withFixture(async (fixture) => {
    const worktree = await linkedWorktree(fixture, "linked-orphan");
    const privateStore = join(worktree, ".maestro", "maestro.db");
    const original = "stage-one-private-store";
    await writeFile(privateStore, original);

    const added = await runCliAt(fixture, worktree, ["work", "add", "shared despite orphan"]);
    const advisoryLines = added.stderr.trim().split("\n").filter(Boolean);
    expect(added.exitCode).toBe(0);
    expect(advisoryLines).toHaveLength(1);
    expect(advisoryLines[0]).toContain("[orphan]");
    expect(advisoryLines[0]).toContain(privateStore);
    expect(await readFile(privateStore, "utf8")).toBe(original);
    expect((await runCli(fixture, ["work", "list"])).stdout).toContain("shared despite orphan");

    const silent = await runCli(fixture, ["status"]);
    expect(silent.exitCode).toBe(0);
    expect(silent.stderr).not.toContain("[orphan]");
  });
});
