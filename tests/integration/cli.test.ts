import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { resolveInstalledBinaryName } from "@/infra/usecases/install-release-binary.usecase.js";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];
const INSTALLED_BINARY_NAME = resolveInstalledBinaryName();
const DIST_CLI = join(import.meta.dir, "..", "..", "dist", INSTALLED_BINARY_NAME);

let tmpDir: string;
const SLOW_CLI_TIMEOUT_MS = 15_000;

async function run(
  args: string[],
  cwd = process.cwd(),
  env: Record<string, string | undefined> = process.env,
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    cwd,
    env,
  });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), stderr: stderr.trim(), exitCode };
}

async function runCompiled(
  args: string[],
  cwd = process.cwd(),
  env: Record<string, string | undefined> = process.env,
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([DIST_CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    cwd,
    env,
  });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), stderr: stderr.trim(), exitCode };
}

async function initGitRepo(cwd: string): Promise<void> {
  const init = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await init.exited;
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-cli-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("CLI integration", () => {
  it("prints version", async () => {
    const { stdout, exitCode } = await run(["--version"]);
    expect(exitCode).toBe(0);
    expect(stdout).toMatch(
      /^\d+\.\d+\.\d+\.\d+-g[a-z0-9]+ \(released \d{4}-\d{2}-\d{2}T.+, \S+ ago\)$/,
    );
  });

  it("prints the current repo HEAD in source-run version output", async () => {
    const shaProc = Bun.spawn(["git", "rev-parse", "--short=7", "HEAD"], {
      cwd: join(import.meta.dir, "..", ".."),
      stdout: "pipe",
      stderr: "pipe",
    });
    const sha = (await new Response(shaProc.stdout).text()).trim();
    expect(await shaProc.exited).toBe(0);

    const { stdout, exitCode } = await run(["--version"]);
    expect(exitCode).toBe(0);
    expect(stdout).toContain(`-g${sha} `);
  });

  it("compiled binary still honors runtime install dir overrides", async () => {
    const homeDir = join(tmpDir, "home");
    const installDir = join(tmpDir, "custom-bin");
    await mkdir(join(homeDir, ".maestro"), { recursive: true });
    await mkdir(installDir, { recursive: true });
      await writeFile(join(installDir, INSTALLED_BINARY_NAME), "test-binary");

    const { stdout, exitCode } = await runCompiled(
      ["uninstall", "--json"],
      tmpDir,
      {
        ...process.env,
        HOME: homeDir,
        MAESTRO_INSTALL_DIR: installDir,
      },
    );

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(result.binaryRemoved).toBe(true);
      expect(await Bun.file(join(installDir, INSTALLED_BINARY_NAME)).exists()).toBe(false);
  });

  it("install only injects home-scoped agent files", async () => {
    const homeDir = join(tmpDir, "home");
    await mkdir(homeDir, { recursive: true });

    const { stdout, exitCode } = await run(
      ["install", "--json"],
      tmpDir,
      {
        ...process.env,
        HOME: homeDir,
      },
    );

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(Array.isArray(result.agents)).toBe(true);
    expect(result.agents.some((agent: { agent: string }) => agent.agent === "Droid CLI")).toBe(false);
    expect(await Bun.file(join(tmpDir, ".maestro", "AGENTS.md")).exists()).toBe(false);
    expect(await Bun.file(join(homeDir, ".maestro", "config.yaml")).exists()).toBe(true);
  });

  it("update --agents-only leaves project-scoped agent files untouched", async () => {
    const homeDir = join(tmpDir, "home");
    await mkdir(join(homeDir, ".claude"), { recursive: true });
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await writeFile(join(tmpDir, ".maestro", "AGENTS.md"), "# Project config\n");

    const { stdout, exitCode } = await run(
      ["update", "--agents-only", "--json"],
      tmpDir,
      {
        ...process.env,
        HOME: homeDir,
      },
    );

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(Array.isArray(result.agents)).toBe(true);
    expect(result.agents.some((agent: { agent: string }) => agent.agent === "Droid CLI")).toBe(false);
    expect(await Bun.file(join(tmpDir, ".maestro", "AGENTS.md")).text()).toBe("# Project config\n");
  });

  it("compiled uninstall leaves project-scoped agent files untouched", async () => {
    const homeDir = join(tmpDir, "home");
    const installDir = join(tmpDir, "custom-bin");
    await mkdir(join(homeDir, ".maestro"), { recursive: true });
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await mkdir(installDir, { recursive: true });
      await writeFile(join(installDir, INSTALLED_BINARY_NAME), "test-binary");
    await writeFile(join(tmpDir, ".maestro", "AGENTS.md"), "# Project config\n");

    const { stdout, exitCode } = await runCompiled(
      ["uninstall", "--json"],
      tmpDir,
      {
        ...process.env,
        HOME: homeDir,
        MAESTRO_INSTALL_DIR: installDir,
      },
    );

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(result.binaryRemoved).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "AGENTS.md")).text()).toBe("# Project config\n");
  });

  it("prints help with all commands", async () => {
    const { stdout, exitCode } = await run(["--help"]);
    expect(exitCode).toBe(0);
    expect(stdout).toContain("init");
    expect(stdout).toContain("note");
    expect(stdout).toContain("status");
    expect(stdout).toContain("doctor");
  });

  it("doctor --json returns structured output", async () => {
    const { stdout, exitCode } = await run(["doctor", "--json"]);
    expect(exitCode).toBe(0);
    const checks = JSON.parse(stdout);
    expect(Array.isArray(checks)).toBe(true);
    expect(checks.length).toBeGreaterThan(0);
    expect(checks[0]).toHaveProperty("name");
    expect(checks[0]).toHaveProperty("status");
  }, SLOW_CLI_TIMEOUT_MS);

  it("status --json returns structured output", async () => {
    const { stdout, exitCode } = await run(["status", "--json"]);
    expect(exitCode).toBe(0);
    const status = JSON.parse(stdout);
    expect(status).toHaveProperty("gitAvailable");
  }, SLOW_CLI_TIMEOUT_MS);

    it("note writes a note and note --list returns it", async () => {
      await initGitRepo(tmpDir);

      const create = await run(
        ["note", "--content", "Remember the branch state", "--json"],
        tmpDir,
      );
    expect(create.exitCode).toBe(0);
    const createdNote = JSON.parse(create.stdout);
    expect(createdNote.content).toBe("Remember the branch state");
    expect(createdNote.git_branch).toBe("main");

    const list = await run(["note", "--list", "--json"], tmpDir);
    expect(list.exitCode).toBe(0);
    const notes = JSON.parse(list.stdout);
    expect(notes).toHaveLength(1);
    expect(notes[0]!.content).toBe("Remember the branch state");
  });
});
