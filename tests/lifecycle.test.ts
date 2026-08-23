import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { cp, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import {
  type CliResult,
  type Fixture,
  prepareInstallFixture,
  runCli,
  runTool,
  withFixture,
} from "./helpers.ts";

const projectRoot = resolve(import.meta.dir, "..");

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
    await cp(join(projectRoot, entry), join(source, entry), { recursive: true });
  }
  await git(source, ["init", "-b", "main"]);
  await git(source, ["config", "user.name", "Maestro Tests"]);
  await git(source, ["config", "user.email", "maestro-tests@example.invalid"]);
  await git(source, ["add", "."]);
  await git(source, ["commit", "-m", "initial source"]);
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
    ) as { plugins: Array<{ name: string }> };
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
    const databaseBefore = sha256(await readFile(databasePath));

    const healthy = await runInstalled(fixture, runtime, source, ["doctor"]);

    expect(healthy.exitCode).toBe(0);
    for (const component of ["shim", "runtime", "source", "wiring", "store"]) {
      expect(healthy.stdout).toContain(component);
    }
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
  const workflowPath = join(projectRoot, ".github", "workflows", "ci.yml");
  expect(existsSync(workflowPath)).toBe(true);
  const workflow = await readFile(workflowPath, "utf8");

  expect(workflow).toMatch(/push:/);
  expect(workflow).toMatch(/pull_request:/);
  expect(workflow).toContain("bun test");
  expect(workflow).toContain("bunx tsc --noEmit");
  expect(workflow).toContain("setInterval");
  expect(workflow).toContain("test-first");
  expect(workflow).toContain("--lane");
  expect(workflow).not.toMatch(/schedule:|cron:/);
});
