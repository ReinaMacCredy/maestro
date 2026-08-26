import { expect, test } from "bun:test";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import {
  addLinkedWorktree,
  initializeGitRepository,
  initializeSeparateGitRepository,
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

test("B3.2 session and prompt records are shared across worktrees", async () => {
  await withFixture(async (fixture) => {
    const worktree = await linkedWorktree(fixture, "linked-session");
    expect(
      (
        await runCli(fixture, ["hook", "record", "--event", "SessionStart"], {
          ...sessionEnvironment("main-target"),
        })
      ).exitCode,
    ).toBe(0);
    const submitted = await runCliAt(
      fixture,
      worktree,
      ["hook", "record", "--event", "UserPromptSubmit"],
      sessionEnvironment("main-target"),
      JSON.stringify({ prompt: "cross-worktree prompt" }),
    );
    const listed = await runCli(fixture, ["prompt", "list", "--session", "main-target"]);
    expect(submitted.exitCode).toBe(0);
    expect(listed.stdout).toContain("cross-worktree prompt");
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

    const gitDirectories = join(fixture.root, "git-directories");
    const firstSeparateRepo = join(fixture.root, "first-separate-repo");
    const secondSeparateRepo = join(fixture.root, "second-separate-repo");
    const firstGitDirectory = join(gitDirectories, "first", ".git");
    const secondGitDirectory = join(gitDirectories, "second", ".git");
    await initializeSeparateGitRepository(firstSeparateRepo, firstGitDirectory);
    await initializeSeparateGitRepository(secondSeparateRepo, secondGitDirectory);

    expect(
      (await runCliAt(fixture, firstSeparateRepo, ["work", "add", "first separate item"]))
        .exitCode,
    ).toBe(0);
    expect(
      (await runCliAt(fixture, secondSeparateRepo, ["work", "add", "second separate item"]))
        .exitCode,
    ).toBe(0);
    const firstSeparateList = await runCliAt(fixture, firstSeparateRepo, ["work", "list"]);
    const secondSeparateList = await runCliAt(fixture, secondSeparateRepo, ["work", "list"]);
    expect(firstSeparateList.stdout).toContain("first separate item");
    expect(firstSeparateList.stdout).not.toContain("second separate item");
    expect(secondSeparateList.stdout).toContain("second separate item");
    expect(secondSeparateList.stdout).not.toContain("first separate item");
    expect(
      await Bun.file(join(dirname(firstGitDirectory), ".maestro", "maestro.db")).exists(),
    ).toBeTrue();
    expect(
      await Bun.file(join(dirname(secondGitDirectory), ".maestro", "maestro.db")).exists(),
    ).toBeTrue();

    const brokenRepo = join(fixture.root, "broken-git-indirection");
    await mkdir(brokenRepo, { recursive: true });
    await writeFile(join(brokenRepo, ".git"), "gitdir: /definitely/missing/maestro-git-dir\n");
    const broken = await runCliAt(fixture, brokenRepo, ["status"]);
    expect(broken.exitCode).not.toBe(0);
    expect(await Bun.file(join(brokenRepo, ".maestro", "maestro.db")).exists()).toBeFalse();

    const outsideRepo = join(fixture.root, "outside-git-at-filesystem-boundary");
    const fakeBin = join(fixture.root, "fake-git-bin");
    await mkdir(join(outsideRepo, ".maestro"), { recursive: true });
    await writeFile(join(outsideRepo, ".maestro", "config"), `${JSON.stringify({ plugins: [] })}\n`);
    await mkdir(fakeBin, { recursive: true });
    const fakeGit = join(fakeBin, "git");
    await writeFile(
      fakeGit,
      [
        "#!/bin/sh",
        "echo 'fatal: not a git repository (or any parent up to mount point /)' >&2",
        "echo 'Stopping at filesystem boundary (GIT_DISCOVERY_ACROSS_FILESYSTEM not set).' >&2",
        "exit 128",
        "",
      ].join("\n"),
    );
    await chmod(fakeGit, 0o755);

    const filesystemBoundary = await runCliAt(fixture, outsideRepo, ["status"], {
      PATH: `${fakeBin}:${process.env.PATH ?? ""}`,
    });
    expect(filesystemBoundary.exitCode).toBe(0);
    expect(await Bun.file(join(outsideRepo, ".maestro", "maestro.db")).exists()).toBeTrue();
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
