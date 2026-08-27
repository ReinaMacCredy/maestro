import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { chmod, cp, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import {
  addLinkedWorktree,
  idFrom,
  initializeGitRepository,
  type CliResult,
  type Fixture,
  prepareInstallFixture,
  runCli,
  runCliAt,
  runTool,
  withFixture,
} from "./helpers.ts";

const projectRoot = resolve(import.meta.dir, "..");

test("284 orphan worktree store is silent on ordinary commands and reported healthy by doctor", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const linked = join(fixture.root, "linked");
    await addLinkedWorktree(fixture.repo, linked);
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCliAt(fixture, linked, ["install"], { PATH: path })).exitCode).toBe(0);

    const orphan = join(linked, ".maestro", "maestro.db");
    await cp(join(fixture.repo, ".maestro", "maestro.db"), orphan);

    const version = await runCliAt(fixture, linked, ["version"], { PATH: path });
    expect(version.exitCode).toBe(0);
    expect(version.stderr).toBe("");

    const diagnosed = await runCliAt(fixture, linked, ["doctor"], { PATH: path });
    expect(diagnosed.exitCode).toBe(0);
    expect(diagnosed.stdout).toContain("doctor: healthy");
    expect(diagnosed.stdout).toContain(orphan);
    expect(await Bun.file(orphan).exists()).toBe(true);
  });
});

function sha256(bytes: Uint8Array | string): string {
  return createHash("sha256").update(bytes).digest("hex");
}

async function git(cwd: string, args: string[]): Promise<string> {
  const result = await runTool(["git", ...args], cwd);
  if (result.exitCode !== 0) {
    throw new Error(`git ${args.join(" ")} failed in ${cwd}: ${result.stderr}`);
  }
  return result.stdout.trim();
}

async function createSourceCheckout(fixture: Fixture): Promise<{
  bare: string;
  publisher: string;
  source: string;
}> {
  const bare = join(fixture.root, "remote.git");
  const publisher = join(fixture.root, "publisher");
  const source = join(fixture.root, "source");
  await mkdir(source, { recursive: true });
  for (const entry of [
    "package.json",
    "tsconfig.json",
    "bin",
    "src",
    ".gitignore",
    "AGENTS.md",
    "CLAUDE.md",
    ".claude",
    ".codex",
  ]) {
    // .claude/ and .codex/ are untracked installer wiring: present in a working
    // checkout, absent in a clean clone; install below materializes them.
    if (!existsSync(join(projectRoot, entry))) continue;
    await cp(join(projectRoot, entry), join(source, entry), { recursive: true });
  }
  await git(source, ["init", "-b", "main"]);
  await git(source, ["config", "user.name", "Maestro Tests"]);
  await git(source, ["config", "user.email", "maestro-tests@example.invalid"]);
  await git(source, ["add", "."]);
  await git(source, ["commit", "-m", "initial source"]);
  const installFixture = await prepareInstallFixture(fixture);
  const materialized = await runCliAt(fixture, source, ["install"], {
    PATH: installFixture.path,
  });
  expect(materialized).toMatchObject({ exitCode: 0 });
  await git(source, ["add", "AGENTS.md", "CLAUDE.md", ".claude", ".codex"]);
  await git(source, ["commit", "--allow-empty", "-m", "materialize current wiring"]);
  await git(fixture.root, ["init", "--bare", "--initial-branch=main", bare]);
  await git(source, ["remote", "add", "origin", bare]);
  await git(source, ["push", "-u", "origin", "main"]);
  await git(fixture.root, ["clone", bare, publisher]);
  await git(publisher, ["config", "user.name", "Maestro Publisher"]);
  await git(publisher, ["config", "user.email", "publisher@example.invalid"]);
  return { bare, publisher, source };
}

async function installSource(
  fixture: Fixture,
  source: string,
): Promise<{ path: string; runtimeRoot: string; shim: string }> {
  const installFixture = await prepareInstallFixture(fixture);
  const direct = Bun.spawn([process.execPath, join(projectRoot, "bin", "maestro.ts"), "install"], {
    cwd: source,
    env: {
      ...process.env,
      HOME: fixture.home,
      MAESTRO_SESSION_ID: "test-session",
      MAESTRO_SESSION_PID: String(process.pid),
      PATH: installFixture.path,
    },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(direct.stdout).text(),
    new Response(direct.stderr).text(),
    direct.exited,
  ]);
  expect({ exitCode, stderr, stdout }).toMatchObject({ exitCode: 0 });
  return {
    path: installFixture.path,
    runtimeRoot: join(fixture.home, ".maestro", "runtime"),
    shim: installFixture.shim,
  };
}

async function runInstalled(
  fixture: Fixture,
  runtime: { path: string; shim: string },
  cwd: string,
  args: string[],
  env: Record<string, string> = {},
): Promise<CliResult> {
  const child = Bun.spawn([runtime.shim, ...args], {
    cwd,
    env: {
      ...process.env,
      HOME: fixture.home,
      MAESTRO_SESSION_ID: "lifecycle-session",
      MAESTRO_SESSION_PID: String(process.pid),
      PATH: runtime.path,
      ...env,
    },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { exitCode, stdout, stderr };
}

test("46 uninstall reverses managed wiring while preserving foreign content and stores", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const claudeSettings = join(fixture.repo, ".claude", "settings.json");
    const codexSettings = join(fixture.repo, ".codex", "hooks.json");
    await mkdir(join(fixture.repo, ".claude"), { recursive: true });
    await mkdir(join(fixture.repo, ".codex"), { recursive: true });
    const foreignSettings = {
      foreign: { exact: "keep-me" },
      hooks: {
        SessionStart: [
          {
            matcher: "foreign",
            hooks: [{ type: "command", command: "foreign-hook" }],
          },
        ],
      },
    };
    await writeFile(claudeSettings, `${JSON.stringify(foreignSettings, null, 2)}\n`);
    await writeFile(codexSettings, `${JSON.stringify(foreignSettings, null, 2)}\n`);
    await writeFile(join(fixture.repo, "AGENTS.md"), "foreign agents\n");
    await writeFile(join(fixture.repo, "CLAUDE.md"), "foreign claude\n");
    await writeFile(
      join(fixture.repo, ".maestro", "config"),
      `${JSON.stringify({ plugins: [{ name: "foreign-plugin", disabled: false }] })}\n`,
    );
    await writeFile(join(fixture.repo, ".maestro", ".gitignore"), "foreign-cache/\n");
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const legacyPath = join(fixture.repo, ".maestro", "store.sqlite");
    await writeFile(legacyPath, "legacy-store-bytes\n");
    const databaseBefore = sha256(await readFile(databasePath));
    const legacyBefore = sha256(await readFile(legacyPath));

    const first = await runCli(fixture, ["uninstall"], { PATH: path });

    expect(first.exitCode).toBe(0);
    expect(first.stdout).toContain("removed");
    expect(existsSync(join(fixture.repo, ".claude", "hooks", "maestro-record.ts"))).toBe(false);
    expect(existsSync(join(fixture.repo, ".codex", "hooks", "maestro-record.ts"))).toBe(false);
    const claudeAfter = JSON.parse(await readFile(claudeSettings, "utf8"));
    const codexAfter = JSON.parse(await readFile(codexSettings, "utf8"));
    for (const settings of [claudeAfter, codexAfter]) {
      expect(settings.foreign).toEqual(foreignSettings.foreign);
      expect(JSON.stringify(settings)).toContain("foreign-hook");
      expect(JSON.stringify(settings)).not.toContain("maestro-record.ts");
    }
    const config = JSON.parse(
      await readFile(join(fixture.repo, ".maestro", "config"), "utf8"),
    ) as { plugins: Array<{ disabled?: boolean; name: string }> };
    expect(config.plugins).toEqual([{ name: "foreign-plugin", disabled: false }]);
    expect(await readFile(join(fixture.repo, "AGENTS.md"), "utf8")).toBe("foreign agents\n");
    expect(await readFile(join(fixture.repo, "CLAUDE.md"), "utf8")).toBe("foreign claude\n");
    expect(await readFile(join(fixture.repo, ".maestro", ".gitignore"), "utf8")).toBe(
      "foreign-cache/\n",
    );
    expect(sha256(await readFile(databasePath))).toBe(databaseBefore);
    expect(sha256(await readFile(legacyPath))).toBe(legacyBefore);

    const second = await runCli(fixture, ["uninstall"], { PATH: path });
    expect(second.exitCode).toBe(0);
    expect(second.stdout).toContain("no changes");
    expect(sha256(await readFile(databasePath))).toBe(databaseBefore);
    expect(sha256(await readFile(legacyPath))).toBe(legacyBefore);
  });
});

test("47 update fast-forwards and resyncs while divergence and fetch failure change nothing", async () => {
  await withFixture(async (fixture) => {
    const { publisher, source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);
    expect(await git(source, ["status", "--porcelain"])).toBe("");
    const oldCommit = await git(source, ["rev-parse", "HEAD"]);

    const packagePath = join(publisher, "package.json");
    const packageJson = JSON.parse(await readFile(packagePath, "utf8"));
    packageJson.version = "0.1.1";
    await writeFile(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);
    await git(publisher, ["add", "package.json"]);
    await git(publisher, ["commit", "-m", "remote update"]);
    await git(publisher, ["push", "origin", "main"]);
    const remoteCommit = await git(publisher, ["rev-parse", "HEAD"]);

    const updated = await runInstalled(fixture, runtime, source, ["update"]);

    expect(updated.exitCode).toBe(0);
    expect(updated.stdout).toContain(`${oldCommit} -> ${remoteCommit}`);
    expect(updated.stdout).toContain("0.1.1");
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(remoteCommit);
    const stampPath = join(runtime.runtimeRoot, ".maestro-install.json");
    expect(JSON.parse(await readFile(stampPath, "utf8")).commit).toBe(remoteCommit);

    await writeFile(join(source, "local-only.txt"), "local\n");
    await git(source, ["add", "local-only.txt"]);
    await git(source, ["commit", "-m", "local divergence"]);
    await writeFile(join(publisher, "remote-only.txt"), "remote\n");
    await git(publisher, ["add", "remote-only.txt"]);
    await git(publisher, ["commit", "-m", "remote divergence"]);
    await git(publisher, ["push", "origin", "main"]);
    const divergedHead = await git(source, ["rev-parse", "HEAD"]);
    const divergedStamp = await readFile(stampPath);

    const diverged = await runInstalled(fixture, runtime, source, ["update"]);

    expect(diverged.exitCode).not.toBe(0);
    expect(diverged.stderr).toContain("UPDATE_DIVERGED");
    expect(diverged.stderr).toContain("maestro update");
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(divergedHead);
    expect(await readFile(stampPath)).toEqual(divergedStamp);

    await git(source, ["remote", "set-url", "origin", join(fixture.root, "missing.git")]);
    const unreachableHead = await git(source, ["rev-parse", "HEAD"]);
    const unreachableStamp = await readFile(stampPath);
    const unreachable = await runInstalled(fixture, runtime, source, ["update"]);

    expect(unreachable.exitCode).not.toBe(0);
    expect(unreachable.stderr).toContain("UPDATE_FETCH_FAILED");
    expect(unreachable.stderr).toContain("fix");
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(unreachableHead);
    expect(await readFile(stampPath)).toEqual(unreachableStamp);
  });
});

test("438 update warns about live holders and still completes", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);
    const holder = {
      MAESTRO_SESSION_ID: "activation-peer",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const caller = {
      MAESTRO_SESSION_ID: "lifecycle-session",
      MAESTRO_SESSION_PID: String(process.pid),
    };
    const work = idFrom(
      await runCli(
        fixture,
        ["work", "add", "live activation work", "--atomic-reason", "fixture"],
        holder,
      ),
    );
    expect((await runCli(fixture, ["work", "start", work], holder)).exitCode).toBe(0);
    const callerWork = idFrom(
      await runCli(
        fixture,
        ["work", "add", "caller activation work", "--atomic-reason", "fixture"],
        caller,
      ),
    );
    expect((await runCli(fixture, ["work", "start", callerWork], caller)).exitCode).toBe(0);
    await writeFile(join(fixture.home, "maestro", "registry"), `${source}\n${fixture.repo}\n`);

    const updated = await runInstalled(fixture, runtime, source, ["update"]);

    expect(updated.exitCode).toBe(0);
    expect(updated.stderr).toContain(
      `[update] 1 live session holds work or an open dispatch (repos: ${fixture.repo}); they load the new runtime on their next maestro call`,
    );
  });
});

test("440 update treats an unreadable registered repository as unsafe and continues", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);
    const unreadableRepo = join(fixture.root, "unreadable-repo");
    await mkdir(unreadableRepo, { recursive: true });
    await writeFile(join(unreadableRepo, ".maestro"), "not a directory\n");
    await writeFile(
      join(fixture.home, "maestro", "registry"),
      `${source}\n${unreadableRepo}\n`,
    );

    const updated = await runInstalled(fixture, runtime, source, ["update"]);

    expect(updated.exitCode).toBe(0);
    expect(updated.stderr).toContain(
      `[update] 1 registered repository unreadable (repos: ${unreadableRepo}); treating as unsafe`,
    );
  });
});

test("302 update reports a failed rollback with commits and an exact recovery command", async () => {
  await withFixture(async (fixture) => {
    const { publisher, source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);
    const oldCommit = await git(source, ["rev-parse", "HEAD"]);
    await rm(join(publisher, "tsconfig.json"));
    await git(publisher, ["add", "-A"]);
    await git(publisher, ["commit", "-m", "remove runtime entry"]);
    await git(publisher, ["push", "origin", "main"]);
    const currentCommit = await git(publisher, ["rev-parse", "HEAD"]);

    const realGit = Bun.which("git");
    if (!realGit) throw new Error("git is required for lifecycle tests");
    const fakeBin = join(fixture.root, "reset-refusal-bin");
    await mkdir(fakeBin, { recursive: true });
    await writeFile(
      join(fakeBin, "git"),
      `#!/bin/sh
if [ "$1" = "reset" ] && [ "$2" = "--hard" ]; then
  printf '%s\n' 'reset refused by fixture' >&2
  exit 42
fi
exec ${JSON.stringify(realGit)} "$@"
`,
    );
    await chmod(join(fakeBin, "git"), 0o755);

    const updated = await runInstalled(fixture, runtime, source, ["update"], {
      PATH: `${fakeBin}:${runtime.path}`,
    });
    const recoveryCommand = `git -C ${JSON.stringify(await realpath(source))} reset --hard ${oldCommit}`;
    const envelope = JSON.parse(updated.stderr) as {
      error: { recoveryCommand?: string };
    };
    expect(updated.exitCode).not.toBe(0);
    expect(updated.stderr).toContain("UPDATE_ROLLBACK_FAILED");
    expect(updated.stderr).toContain(oldCommit);
    expect(updated.stderr).toContain(currentCommit);
    expect(envelope.error.recoveryCommand).toBe(recoveryCommand);
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(currentCommit);
  });
});

test("64 update ignores untracked source files but still refuses tracked changes", async () => {
  await withFixture(async (fixture) => {
    const { publisher, source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);

    // Real installs leave untracked wiring in the source checkout (.claude/,
    // .codex/); update must not treat those as dirt or it refuses forever.
    await writeFile(join(source, "untracked-note.txt"), "scratch\n");
    // An untracked file under src/ would be copied into a runtime stamped as
    // HEAD, so update refuses it until it is committed or removed.
    const stray = join(source, "src", "untracked-runtime-marker.ts");
    await writeFile(stray, "export const marker = true;\n");
    const strayRefused = await runInstalled(fixture, runtime, source, ["update"]);
    expect(strayRefused.exitCode).not.toBe(0);
    expect(strayRefused.stderr).toContain("UPDATE_SOURCE_DIRTY");
    await rm(stray);
    const packagePath = join(publisher, "package.json");
    const packageJson = JSON.parse(await readFile(packagePath, "utf8"));
    packageJson.version = "0.1.2";
    await writeFile(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);
    await git(publisher, ["add", "package.json"]);
    await git(publisher, ["commit", "-m", "remote update"]);
    await git(publisher, ["push", "origin", "main"]);
    const remoteCommit = await git(publisher, ["rev-parse", "HEAD"]);

    const updated = await runInstalled(fixture, runtime, source, ["update"]);

    expect(updated.exitCode).toBe(0);
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(remoteCommit);
    expect(existsSync(join(source, "untracked-note.txt"))).toBe(true);

    const agentsPath = join(source, "AGENTS.md");
    await writeFile(agentsPath, `${await readFile(agentsPath, "utf8")}\ntracked dirt\n`);
    const dirtyHead = await git(source, ["rev-parse", "HEAD"]);
    const refused = await runInstalled(fixture, runtime, source, ["update"]);

    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("UPDATE_SOURCE_DIRTY");
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(dirtyHead);
  });
});

test("65 install writes wiring from the freshly synced runtime, not the stale running one", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);

    // Change the wiring content in the source and commit it; the installed
    // runtime still carries the old constant in its loaded code.
    const installPath = join(source, "src", "plugins", "install.ts");
    const installCode = await readFile(installPath, "utf8");
    await writeFile(
      installPath,
      installCode.replace("method depth:", "method depth (v2-marker):"),
    );
    await git(source, ["add", "src/plugins/install.ts"]);
    await git(source, ["commit", "-m", "wiring marker"]);
    const newCommit = await git(source, ["rev-parse", "HEAD"]);

    const installed = await runInstalled(fixture, runtime, source, ["install"]);

    expect(installed.exitCode).toBe(0);
    expect(await readFile(join(source, "AGENTS.md"), "utf8")).toContain("v2-marker");
    expect(await readFile(join(source, "CLAUDE.md"), "utf8")).toContain("v2-marker");
    const stampPath = join(runtime.runtimeRoot, ".maestro-install.json");
    expect(JSON.parse(await readFile(stampPath, "utf8")).commit).toBe(newCommit);
  });
});

test("66 update treats an ahead-only source as nothing to pull yet still refuses true divergence", async () => {
  await withFixture(async (fixture) => {
    const { publisher, source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);

    await writeFile(join(source, "ahead-note.txt"), "local ahead\n");
    await git(source, ["add", "ahead-note.txt"]);
    await git(source, ["commit", "-m", "local ahead"]);
    const aheadHead = await git(source, ["rev-parse", "HEAD"]);

    const ahead = await runInstalled(fixture, runtime, source, ["update"]);

    expect(ahead.exitCode).toBe(0);
    expect(ahead.stdout).toContain("nothing to pull");
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(aheadHead);
    const stampPath = join(runtime.runtimeRoot, ".maestro-install.json");
    expect(JSON.parse(await readFile(stampPath, "utf8")).commit).toBe(aheadHead);

    await writeFile(join(publisher, "remote-note.txt"), "remote\n");
    await git(publisher, ["add", "remote-note.txt"]);
    await git(publisher, ["commit", "-m", "remote divergence"]);
    await git(publisher, ["push", "origin", "main"]);

    const diverged = await runInstalled(fixture, runtime, source, ["update"]);

    expect(diverged.exitCode).not.toBe(0);
    expect(diverged.stderr).toContain("UPDATE_DIVERGED");
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(aheadHead);
  });
});

test("48 status and hook brief report local drift unless auto-update checks are disabled", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);

    const current = await runInstalled(fixture, runtime, source, ["status"]);
    expect(current.stdout).not.toContain("maestro update");

    await writeFile(join(source, "drift.txt"), "drift\n");
    await git(source, ["add", "drift.txt"]);
    await git(source, ["commit", "-m", "local drift"]);
    const drifted = await runInstalled(fixture, runtime, source, ["status"]);
    const brief = await runInstalled(fixture, runtime, source, [
      "hook",
      "record",
      "--event",
      "SessionStart",
    ]);

    expect(drifted.stdout).toContain("maestro update");
    expect(brief.stdout).toContain("maestro update");
    const disabled = await runInstalled(
      fixture,
      runtime,
      source,
      ["status"],
      { MAESTRO_AUTO_UPDATE: "0" },
    );
    expect(disabled.stdout).not.toContain("maestro update");
  });
});

test("49 doctor reports healthy components and structured fixable issues without repair", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);
    const databasePath = join(source, ".maestro", "maestro.db");
    const database = new Database(databasePath, { strict: true });
    const tableCountBeforeSeed = database
      .query<{ count: number }, []>(
        "SELECT count(*) AS count FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
      )
      .get()?.count ?? 0;
    database.exec("CREATE TABLE doctor_independent_probe (id INTEGER PRIMARY KEY)");
    database.close();
    const databaseBefore = sha256(await readFile(databasePath));
    await writeFile(join(source, "doctor-untracked.txt"), "scratch\n");
    const sourcePath = await realpath(source);
    const sourceHead = await git(source, ["rev-parse", "HEAD"]);

    const healthy = await runInstalled(fixture, runtime, source, ["doctor"]);

    expect(healthy.exitCode).toBe(0);
    for (const component of ["shim", "runtime", "source", "wiring", "store"]) {
      expect(healthy.stdout).toContain(component);
    }
    expect(healthy.stdout).toContain(`source: ok ${sourcePath} ${sourceHead} clean`);
    expect(healthy.stdout).toContain("room deny list: ok");
    expect(healthy.stdout).toContain(`store: ok (${tableCountBeforeSeed + 1} tables)`);
    expect(healthy.stdout).not.toContain("store: ok schema");
    expect(sha256(await readFile(databasePath))).toBe(databaseBefore);

    const missingHook = join(source, ".claude", "hooks", "maestro-record.ts");
    await rm(missingHook);
    const broken = await runInstalled(fixture, runtime, source, ["doctor"]);

    expect(broken.exitCode).not.toBe(0);
    expect(broken.stderr).toContain("DOCTOR_ISSUES");
    expect(broken.stderr).toContain("maestro install");
    expect(existsSync(missingHook)).toBe(false);

    await writeFile(
      join(fixture.home, ".maestro", "source.json"),
      `${JSON.stringify({ path: join(fixture.root, "missing-source") })}\n`,
    );
    const stale = await runInstalled(fixture, runtime, source, ["doctor"]);
    expect(stale.exitCode).not.toBe(0);
    expect(stale.stderr).toContain("source");
    expect(stale.stderr).toContain("maestro install");
  });
});

test("441 doctor reports one skills issue for each past-due review date", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);
    const skill = join(
      fixture.home,
      "maestro",
      "skills",
      "maestro-work",
      "SKILL.md",
    );
    const content = await readFile(skill, "utf8");
    await writeFile(
      skill,
      content.replace(/^review-date: .*$/m, "review-date: 2000-01-01"),
    );

    const diagnosed = await runInstalled(fixture, runtime, source, ["doctor"]);

    expect(diagnosed.exitCode).not.toBe(0);
    expect(diagnosed.stderr).toContain("DOCTOR_ISSUES");
    expect(diagnosed.stderr).toContain(`skills: review date 2000-01-01 is past due: ${skill}`);
    const error = JSON.parse(diagnosed.stderr) as {
      error: {
        issues: Array<{ component: string; fix: string; message: string }>;
      };
    };
    expect(error.error.issues.filter((issue) => issue.component === "skills")).toEqual([
      {
        component: "skills",
        fix: "review the rule or move the date",
        message: `review date 2000-01-01 is past due: ${skill}`,
      },
    ]);
  });
});

test("50 source installs record a machine-scoped checkout without leaking paths into the runtime stamp", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);
    const sourceRecordPath = join(fixture.home, ".maestro", "source.json");
    const stampPath = join(runtime.runtimeRoot, ".maestro-install.json");
    const sourceRecord = JSON.parse(await readFile(sourceRecordPath, "utf8"));
    const stampText = await readFile(stampPath, "utf8");
    const stamp = JSON.parse(stampText);

    expect(sourceRecord).toEqual({ path: await realpath(source) });
    expect(sourceRecordPath.startsWith(runtime.runtimeRoot)).toBe(false);
    expect(Object.keys(stamp).sort()).toEqual(["commit", "installedAt", "version"]);
    expect(stampText).not.toContain(source);
    expect(stampText).not.toContain(fixture.root);
  });
});

test("51 CI runs tests, type-check, and anti-goal greps on push and pull requests only", async () => {
  await withFixture(async (fixture) => {
    const workflowPath = join(projectRoot, ".github", "workflows", "ci.yml");
    expect(existsSync(workflowPath)).toBe(true);
    const workflowText = await readFile(workflowPath, "utf8");
    const workflow = Bun.YAML.parse(workflowText) as {
      jobs?: Record<
        string,
        {
          if?: boolean | string;
          steps?: Array<{ if?: boolean | string; name?: string; run?: string; uses?: string }>;
        }
      >;
      on?: Record<string, unknown>;
    };

    expect(Object.keys(workflow.on ?? {}).sort()).toEqual(["pull_request", "push"]);
    expect(Object.keys(workflow.jobs ?? {})).toEqual(["verify"]);
    const verify = workflow.jobs?.verify;
    expect(verify?.if).toBeUndefined();
    const steps = verify?.steps ?? [];
    expect(steps.map((step) => step.name)).toEqual([
      "Check out source",
      "Set up Bun",
      "Install",
      "Install zsh and ripgrep for the shellrc tests and the anti-goal gates",
      "Test",
      "Type-check",
      "A1 no daemon or scheduler",
      "A2 mechanism-only kernel",
      "A3 no escape-hatch flags",
    ]);
    expect(steps.every((step) => step.if === undefined)).toBe(true);
    const stepByName = new Map(steps.map((step) => [step.name, step]));
    expect(stepByName.get("Test")?.run).toBe("bun test");
    expect(stepByName.get("Type-check")?.run).toBe("bunx tsc --noEmit");

    const sourceText = await readFile(join(projectRoot, "src", "kernel", "index.ts"), "utf8");
    const probeRoot = join(fixture.root, "grep-probe");
    const probePath = join(probeRoot, "src", "kernel", "index.ts");
    await mkdir(join(probeRoot, "src", "kernel"), { recursive: true });
    for (const [name, violation] of [
      ["A1 no daemon or scheduler", "setInterval(() => {}, 1000);"],
      ["A2 mechanism-only kernel", 'const proof = "test-first";'],
      ["A3 no escape-hatch flags", 'const lane = "--lane light";'],
    ] as const) {
      const run = stepByName.get(name)?.run ?? "";
      const command = run.match(/^if (rg .+); then$/m)?.[1];
      expect(command, name).toBeString();
      const clean = await runTool(["/bin/sh", "-c", command ?? "exit 2"], projectRoot);
      expect(clean.exitCode, `${name}\n${clean.stdout}${clean.stderr}`).toBe(1);

      await writeFile(probePath, `${sourceText}\n${violation}\n`);
      const seeded = await runTool(["/bin/sh", "-c", command ?? "exit 2"], probeRoot);
      expect(seeded.exitCode, `${name}\n${seeded.stdout}${seeded.stderr}`).toBe(0);
    }
  });
});

test("53 a clean install type-checks without runtime dependencies", async () => {
  await withFixture(async (fixture) => {
    const cleanCheckout = join(fixture.root, "clean-checkout");
    await mkdir(cleanCheckout, { recursive: true });
    for (const entry of ["package.json", "tsconfig.json", "bin", "src", "tests"]) {
      await cp(join(projectRoot, entry), join(cleanCheckout, entry), { recursive: true });
    }
    const lockfile = join(projectRoot, "bun.lock");
    if (existsSync(lockfile)) await cp(lockfile, join(cleanCheckout, "bun.lock"));

    const installed = await runTool([process.execPath, "install"], cleanCheckout);
    const typechecked = await runTool(
      [process.execPath, "x", "tsc", "--noEmit"],
      cleanCheckout,
    );
    const packageJson = JSON.parse(await readFile(join(cleanCheckout, "package.json"), "utf8")) as {
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
    };

    expect(installed.exitCode).toBe(0);
    expect(typechecked).toEqual({ exitCode: 0, stdout: "", stderr: "" });
    expect(packageJson.dependencies ?? {}).toEqual({});
    expect(Object.keys(packageJson.devDependencies ?? {}).sort()).toEqual([
      "@types/bun",
      "typescript",
    ]);
    for (const version of Object.values(packageJson.devDependencies ?? {})) {
      expect(version).toMatch(/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/);
    }
  });
}, 10_000);

test("231 update on a source branch with no upstream resyncs instead of erroring", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);

    // The local-development shape this repo has run in for hundreds of commits:
    // a working branch nobody has pushed, so there is no upstream to pull from.
    await git(source, ["checkout", "-b", "local-only"]);
    await writeFile(join(source, "local-only-note.txt"), "local only\n");
    await git(source, ["add", "local-only-note.txt"]);
    await git(source, ["commit", "-m", "work with no upstream"]);
    const head = await git(source, ["rev-parse", "HEAD"]);

    const updated = await runInstalled(fixture, runtime, source, ["update"]);

    expect(updated.exitCode).toBe(0);
    expect(updated.stdout).toContain("nothing to pull");
    expect(updated.stdout).toContain("no upstream");
    expect(await git(source, ["rev-parse", "HEAD"])).toBe(head);
    const stampPath = join(runtime.runtimeRoot, ".maestro-install.json");
    expect(JSON.parse(await readFile(stampPath, "utf8")).commit).toBe(head);
  });
});

test("411 update regenerates the room's generated files and keeps OWNER.md", async () => {
  await withFixture(async (fixture) => {
    const { source } = await createSourceCheckout(fixture);
    const runtime = await installSource(fixture, source);
    const room = join(fixture.home, "maestro");
    const owner = join(room, "OWNER.md");
    await writeFile(owner, "# owner notes\nkeep me\n");
    await writeFile(join(room, "lane.md"), "stale lane text\n");

    const updated = await runInstalled(fixture, runtime, source, ["update"]);

    expect(updated.exitCode).toBe(0);
    const lane = await readFile(join(room, "lane.md"), "utf8");
    expect(lane).not.toContain("stale lane text");
    expect(lane).toContain("a lane with a second stop point needs a second dispatch");
    expect(await readFile(owner, "utf8")).toBe("# owner notes\nkeep me\n");
  });
});
