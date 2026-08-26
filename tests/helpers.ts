import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

export interface Fixture {
  home: string;
  repo: string;
  root: string;
}

export interface CliResult {
  exitCode: number;
  stderr: string;
  stdout: string;
}

export interface InstallFixture {
  localBin: string;
  path: string;
  shim: string;
}

const cli = join(import.meta.dir, "..", "bin", "maestro.ts");

export async function withFixture<T>(run: (fixture: Fixture) => Promise<T>): Promise<T> {
  const root = await mkdtemp(join(tmpdir(), "maestro-stage1-"));
  const fixture = {
    root,
    home: join(root, "home"),
    repo: join(root, "repo"),
  };
  await mkdir(join(fixture.repo, ".maestro", "plugins"), { recursive: true });
  await mkdir(fixture.home, { recursive: true });
  await writeConfig(fixture, []);

  try {
    return await run(fixture);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

export async function runCli(
  fixture: Fixture,
  args: string[],
  env: Record<string, string | undefined> = {},
  stdin?: string,
): Promise<CliResult> {
  return runCliAt(fixture, fixture.repo, args, env, stdin);
}

export async function runCliAt(
  fixture: Fixture,
  cwd: string,
  args: string[],
  env: Record<string, string | undefined> = {},
  stdin?: string,
): Promise<CliResult> {
  return runCliBinary(fixture, cli, cwd, args, env, stdin);
}

export async function runInstalledCliAt(
  fixture: Fixture,
  cwd: string,
  args: string[],
  env: Record<string, string | undefined> = {},
  stdin?: string,
): Promise<CliResult> {
  return runCliBinary(
    fixture,
    join(fixture.home, ".local", "bin", "maestro"),
    cwd,
    args,
    env,
    stdin,
  );
}

async function runCliBinary(
  fixture: Fixture,
  binary: string,
  cwd: string,
  args: string[],
  env: Record<string, string | undefined>,
  stdin?: string,
): Promise<CliResult> {
  const childEnvironment: Record<string, string | undefined> = {
    ...process.env,
    HOME: fixture.home,
    MAESTRO_SESSION_ID: "test-session",
    MAESTRO_SESSION_PID: String(process.pid),
  };
  for (const [name, value] of Object.entries(env)) {
    if (value === undefined) {
      delete childEnvironment[name];
    } else {
      childEnvironment[name] = value;
    }
  }
  const command = binary === cli ? [process.execPath, binary, ...args] : [binary, ...args];
  const child = Bun.spawn(command, {
    cwd,
    env: childEnvironment,
    stdin: stdin === undefined ? undefined : new TextEncoder().encode(stdin),
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

export async function prepareInstallFixture(
  fixture: Fixture,
  shimSource = "#!/bin/sh\necho legacy-maestro\n",
): Promise<InstallFixture> {
  const localBin = join(fixture.home, ".local", "bin");
  const shim = join(localBin, "maestro");
  await mkdir(localBin, { recursive: true });
  await writeFile(shim, shimSource);
  await chmod(shim, 0o755);
  return {
    localBin,
    path: [localBin, dirname(process.execPath), "/usr/bin", "/bin"].join(":"),
    shim,
  };
}

export async function initializeGitRepository(repo: string): Promise<void> {
  await mkdir(join(repo, ".maestro"), { recursive: true });
  const config = join(repo, ".maestro", "config");
  if (!(await Bun.file(config).exists())) {
    await writeFile(config, `${JSON.stringify({ plugins: [] })}\n`);
  }
  const commands = [
    ["git", "init", "-b", "main"],
    ["git", "add", ".maestro/config"],
    [
      "git",
      "-c",
      "user.name=Maestro Tests",
      "-c",
      "user.email=maestro-tests@example.invalid",
      "commit",
      "-m",
      "fixture",
    ],
  ];
  for (const command of commands) {
    const result = await runTool(command, repo);
    if (result.exitCode !== 0) {
      throw new Error(`fixture git command failed: ${command.join(" ")}\n${result.stderr}`);
    }
  }
}

export async function initializeSeparateGitRepository(
  repo: string,
  gitDirectory: string,
): Promise<void> {
  await mkdir(repo, { recursive: true });
  await mkdir(dirname(gitDirectory), { recursive: true });
  const initialized = await runTool(
    ["git", "init", "-b", "main", "--separate-git-dir", gitDirectory],
    repo,
  );
  if (initialized.exitCode !== 0) {
    throw new Error(`fixture separate git init failed: ${initialized.stderr}`);
  }
  await mkdir(join(repo, ".maestro"), { recursive: true });
  await writeFile(join(repo, ".maestro", "config"), `${JSON.stringify({ plugins: [] })}\n`);
  for (const command of [
    ["git", "add", ".maestro/config"],
    [
      "git",
      "-c",
      "user.name=Maestro Tests",
      "-c",
      "user.email=maestro-tests@example.invalid",
      "commit",
      "-m",
      "fixture",
    ],
  ]) {
    const result = await runTool(command, repo);
    if (result.exitCode !== 0) {
      throw new Error(`fixture git command failed: ${command.join(" ")}\n${result.stderr}`);
    }
  }
}

export async function addLinkedWorktree(repo: string, path: string): Promise<void> {
  const result = await runTool(["git", "worktree", "add", "--detach", path], repo);
  if (result.exitCode !== 0) {
    throw new Error(`fixture git worktree add failed: ${result.stderr}`);
  }
}

export async function runTool(
  command: string[],
  cwd: string,
): Promise<CliResult> {
  const child = Bun.spawn(command, { cwd, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { exitCode, stdout, stderr };
}

export async function writeConfig(
  fixture: Fixture,
  plugins: Array<{ disabled?: boolean; name: string }>,
): Promise<void> {
  await mkdir(join(fixture.repo, ".maestro"), { recursive: true });
  await writeFile(
    join(fixture.repo, ".maestro", "config"),
    `${JSON.stringify({ plugins })}\n`,
  );
}

export async function setPlugin(
  fixture: Fixture,
  name: string,
  disabled: boolean,
): Promise<void> {
  const path = join(fixture.repo, ".maestro", "config");
  const config = JSON.parse(await readFile(path, "utf8")) as {
    plugins: Array<{ disabled?: boolean; name: string }>;
  };
  const entry = config.plugins.find((candidate) => candidate.name === name);
  if (entry) {
    entry.disabled = disabled;
  } else {
    config.plugins.push({ name, disabled });
  }
  await writeFile(path, `${JSON.stringify(config)}\n`);
}

export async function writePlugin(
  fixture: Fixture,
  tier: "global" | "repo",
  name: string,
  source: string,
): Promise<string> {
  const directory =
    tier === "repo"
      ? join(fixture.repo, ".maestro", "plugins")
      : join(fixture.home, ".maestro", "plugins");
  await mkdir(directory, { recursive: true });
  const path = join(directory, `${name}.ts`);
  await writeFile(path, source);
  return path;
}

export function idFrom(result: CliResult): string {
  const match = result.stdout.match(/\b[wd]\d+\b/);
  if (!match) {
    throw new Error(`missing entity id in stdout: ${JSON.stringify(result.stdout)}`);
  }
  return match[0];
}

export const helloPlugin = `
export default {
  name: "hello",
  apply(ctx) {
    ctx.effect(() => ctx.cli.register("hello", async () => "hello from effect"));
  },
};
`;
