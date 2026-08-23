import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
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
  env: Record<string, string> = {},
): Promise<CliResult> {
  const child = Bun.spawn([process.execPath, cli, ...args], {
    cwd: fixture.repo,
    env: {
      ...process.env,
      HOME: fixture.home,
      MAESTRO_SESSION_ID: "test-session",
      MAESTRO_SESSION_PID: String(process.pid),
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
