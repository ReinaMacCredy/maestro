import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

let tmpDir: string;

async function run(
  args: string[],
  cwd = process.cwd(),
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    cwd,
  });

  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);

  return {
    stdout: stdout.trim(),
    stderr: stderr.trim(),
    exitCode: await proc.exited,
  };
}

async function initGitRepo(cwd: string): Promise<void> {
  const proc = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });

  await proc.exited;
}

describe("init CLI", () => {
  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-init-cli-"));
    await initGitRepo(tmpDir);
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("creates the full .maestro bootstrap skeleton", async () => {
    const { stdout, exitCode } = await run(["init", "--json"], tmpDir);

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(result.scope).toBe("project");
    expect(result.bootstrapGenerated).toBe(true);

    expect(await Bun.file(join(tmpDir, ".maestro", "AGENTS.md")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "bootstrap", "init.sh")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "bootstrap", "services.yaml")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "bootstrap", "library", "architecture.md")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "bootstrap", "validation", "README.md")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".factory")).exists()).toBe(false);
  });

  it("skips existing files in non-interactive mode", async () => {
    const agentsPath = join(tmpDir, ".maestro", "AGENTS.md");
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await writeFile(agentsPath, "custom bootstrap\n");

    const { stdout, exitCode } = await run(["init", "--json"], tmpDir);

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(result.skipped.some((path: string) => path.endsWith("/.maestro/AGENTS.md"))).toBe(true);
    expect(await readFile(agentsPath, "utf8")).toBe("custom bootstrap\n");
  });
});
