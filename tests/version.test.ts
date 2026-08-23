import { expect, test } from "bun:test";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import {
  type CliResult,
  type Fixture,
  runCli,
  runTool,
  withFixture,
} from "./helpers.ts";

const installStampFile = ".maestro-install.json";
const sourceRoot = resolve(import.meta.dir, "..");

function isolatedPath(localBin: string): string {
  return [localBin, dirname(process.execPath), "/usr/bin", "/bin"].join(":");
}

async function installRuntime(fixture: Fixture): Promise<{
  localBin: string;
  path: string;
  runtimeRoot: string;
}> {
  const localBin = join(fixture.home, ".local", "bin");
  const shim = join(localBin, "maestro");
  await mkdir(localBin, { recursive: true });
  await writeFile(shim, "#!/bin/sh\necho legacy-maestro\n");
  await chmod(shim, 0o755);
  const path = isolatedPath(localBin);
  const installed = await runCli(fixture, ["install"], { PATH: path });
  expect(installed.exitCode).toBe(0);
  return {
    localBin,
    path,
    runtimeRoot: join(fixture.home, ".maestro", "runtime"),
  };
}

async function runInstalled(
  fixture: Fixture,
  localBin: string,
  path: string,
  args: string[],
): Promise<CliResult> {
  const child = Bun.spawn([join(localBin, "maestro"), ...args], {
    cwd: fixture.repo,
    env: {
      ...process.env,
      HOME: fixture.home,
      MAESTRO_SESSION_ID: "test-session",
      MAESTRO_SESSION_PID: String(process.pid),
      PATH: path,
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

async function sourceCommit(): Promise<string> {
  const result = await runTool(["git", "rev-parse", "HEAD"], sourceRoot);
  expect(result.exitCode).toBe(0);
  return result.stdout.trim();
}

test("31 installed version and top-level aliases print package and install identity", async () => {
  await withFixture(async (fixture) => {
    const { localBin, path } = await installRuntime(fixture);
    const verb = await runInstalled(fixture, localBin, path, ["version"]);
    const longAlias = await runInstalled(fixture, localBin, path, ["--version"]);
    const shortAlias = await runInstalled(fixture, localBin, path, ["-v"]);

    for (const result of [verb, longAlias, shortAlias]) {
      expect(result.exitCode).toBe(0);
      expect(result.stderr).toBe("");
      expect(result.stdout).toContain("maestro 0.1.0");
      expect(result.stdout).toContain(await sourceCommit());
      expect(result.stdout).toMatch(/installed \d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z/);
    }
    expect(longAlias.stdout).toBe(verb.stdout);
    expect(shortAlias.stdout).toBe(verb.stdout);
  });
});

test("32 source version without an install stamp reports source dev and exits zero", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["version"]);

    expect(result.exitCode).toBe(0);
    expect(result.stderr).toBe("");
    expect(result.stdout).toBe("maestro 0.1.0 (source/dev)\n");
  });
});

test("33 install writes a portable version commit and ISO date stamp inside the runtime", async () => {
  await withFixture(async (fixture) => {
    const { runtimeRoot } = await installRuntime(fixture);
    const stampPath = join(runtimeRoot, installStampFile);
    const stampText = await readFile(stampPath, "utf8");
    const stamp = JSON.parse(stampText) as {
      commit: string;
      installedAt: string;
      version: string;
    };

    expect(Object.keys(stamp).sort()).toEqual(["commit", "installedAt", "version"]);
    expect(stamp.version).toBe("0.1.0");
    expect(stamp.commit).toBe(await sourceCommit());
    expect(new Date(stamp.installedAt).toISOString()).toBe(stamp.installedAt);
    expect(Object.values(stamp).some((value) => value.startsWith("/"))).toBe(false);
    expect(stampText).not.toContain(sourceRoot);
    expect(stampText).not.toContain(fixture.repo);
  });
});
