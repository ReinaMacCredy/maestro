import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { access, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];
const PTY_TIMEOUT_MS = 15_000;
let pythonAvailable = false;

const PYTHON_PTY_RUNNER = `
import base64, json, os, select, subprocess, sys, time, pty, fcntl, termios, struct

payload = json.loads(sys.argv[1])
master, slave = pty.openpty()
rows = payload.get("rows", 24)
cols = payload.get("cols", 80)
winsize = struct.pack("HHHH", rows, cols, 0, 0)
fcntl.ioctl(slave, termios.TIOCSWINSZ, winsize)
proc = subprocess.Popen(
    payload["cmd"],
    cwd=payload["cwd"],
    stdin=slave,
    stdout=slave,
    stderr=slave,
    close_fds=True,
)
os.close(slave)

deadline = time.time() + (payload.get("timeoutMs", 15000) / 1000.0)
chunks = []
timed_out = False

while True:
    timeout = max(0.0, min(0.1, deadline - time.time()))
    readable, _, _ = select.select([master], [], [], timeout)
    if readable:
        try:
            data = os.read(master, 4096)
            if data:
                chunks.append(data)
        except OSError:
            pass

    if proc.poll() is not None:
        end = time.time() + 0.2
        while time.time() < end:
            try:
                data = os.read(master, 4096)
                if not data:
                    break
                chunks.append(data)
            except OSError:
                break
        break

    if time.time() >= deadline:
        timed_out = True
        proc.kill()
        proc.wait()
        break

os.close(master)
print(json.dumps({
    "exitCode": proc.returncode,
    "timedOut": timed_out,
    "rawOutput": base64.b64encode(b"".join(chunks)).decode("ascii"),
}))
`;

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

async function runInteractivePty(
  args: string[],
  cwd = process.cwd(),
): Promise<{ exitCode: number | null; timedOut: boolean; rawOutput: string }> {
  const payload = JSON.stringify({
    cmd: [...CLI, ...args],
    cwd,
    timeoutMs: PTY_TIMEOUT_MS,
    rows: 24,
    cols: 80,
  });

  const proc = Bun.spawn(["python3", "-c", PYTHON_PTY_RUNNER, payload], {
    stdout: "pipe",
    stderr: "pipe",
  });

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  expect(exitCode).toBe(0);
  expect(stderr.trim()).toBe("");

  const result = JSON.parse(stdout) as {
    exitCode: number | null;
    timedOut: boolean;
    rawOutput: string;
  };

  return {
    exitCode: result.exitCode,
    timedOut: result.timedOut,
    rawOutput: Buffer.from(result.rawOutput, "base64").toString("utf8"),
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

async function commandExists(command: string): Promise<boolean> {
  if (process.platform === "win32") return false;
  const proc = Bun.spawn(["bash", "-lc", `command -v ${command}`], {
    stdout: "pipe",
    stderr: "pipe",
  });
  return (await proc.exited) === 0;
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

describe("init CLI", () => {
  beforeAll(async () => {
    pythonAvailable = await commandExists("python3");
  });

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
    expect(result.ok).toBe(true);

    expect(await Bun.file(join(tmpDir, ".maestro", "AGENTS.md")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "bootstrap", "init.sh")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "bootstrap", "services.yaml")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "bootstrap", "library", "architecture.md")).exists()).toBe(true);
    expect(await Bun.file(join(tmpDir, ".maestro", "bootstrap", "validation", "README.md")).exists()).toBe(true);
    expect(await readFile(join(tmpDir, ".maestro", "bootstrap", "services.yaml"), "utf8")).toContain(
      "Customize commands.missionControlPreview",
    );
    expect(await readFile(join(tmpDir, ".maestro", "bootstrap", "services.yaml"), "utf8")).not.toContain(
      "maestro mission-control",
    );
    expect(await readFile(join(tmpDir, ".gitignore"), "utf8")).toContain(".maestro/sessions/");
    expect(await readFile(join(tmpDir, ".gitignore"), "utf8")).toContain(".maestro/tasks/local-history/");
    expect(await readFile(join(tmpDir, ".gitignore"), "utf8")).toContain(".maestro/evidence/");
    expect(await readFile(join(tmpDir, ".gitignore"), "utf8")).toContain(".maestro/runs/");
  });

  it("skips existing files in non-interactive mode", async () => {
    const agentsPath = join(tmpDir, ".maestro", "AGENTS.md");
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await writeFile(agentsPath, "custom bootstrap\n");

    const { stdout, exitCode } = await run(["init", "--json"], tmpDir);

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(result.skipped.some((path: string) => path.endsWith(join(".maestro", "AGENTS.md")))).toBe(true);
    expect(await readFile(agentsPath, "utf8")).toBe("custom bootstrap\n");
  });

  it("reports no new directories on an idempotent second run", async () => {
    await run(["init", "--json"], tmpDir);

    const { stdout, exitCode } = await run(["init", "--json"], tmpDir);

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(result.created).toEqual([]);
    expect(result.skipped.some((path: string) => path.endsWith(join(".maestro", "config.yaml")))).toBe(true);
  });

  it("hard-deletes legacy .factory directory on setup", async () => {
    await mkdir(join(tmpDir, ".factory", "library"), { recursive: true });
    await writeFile(
      join(tmpDir, ".factory", "services.yaml"),
      "commands:\n  test: echo legacy-test\nservices: {}\n",
    );

    const { exitCode } = await run(["init", "--json"], tmpDir);

    expect(exitCode).toBe(0);
    expect(await pathExists(join(tmpDir, ".factory"))).toBe(false);
  });

  it("keeps runtime session logs ignored after init", async () => {
    await run(["init", "--json"], tmpDir);
    await mkdir(join(tmpDir, ".maestro", "sessions"), { recursive: true });
    const sessionPath = join(tmpDir, ".maestro", "sessions", "events.jsonl");
    await writeFile(sessionPath, "{}\n");

    const proc = Bun.spawn(
      ["git", "-c", "core.quotePath=false", "check-ignore", sessionPath],
      {
        cwd: tmpDir,
        stdout: "pipe",
        stderr: "pipe",
      },
    );
    const stdout = await new Response(proc.stdout).text();
    const exitCode = await proc.exited;

    expect(exitCode).toBe(0);
    // Windows git still renders backslash paths as C-escaped double-quoted
    // strings in check-ignore output (e.g. "C:\\Users\\..."); unwrap them
    // before comparing so the test is deterministic across runners.
    const reported = stdout.trim().replace(/^"(.*)"$/, "$1").replace(/\\\\/g, "\\");
    expect(reported).toBe(sessionPath);
  });

  it("does not duplicate .maestro/evidence/ or .maestro/runs/ on a second init", async () => {
    await run(["init", "--json"], tmpDir);
    await run(["init", "--json"], tmpDir);

    const content = await readFile(join(tmpDir, ".gitignore"), "utf8");
    const evidenceMatches = content.split("\n").filter((line) => line === ".maestro/evidence/");
    const runsMatches = content.split("\n").filter((line) => line === ".maestro/runs/");
    expect(evidenceMatches.length).toBe(1);
    expect(runsMatches.length).toBe(1);
  });

  it("exits cleanly in tty mode when no replacement prompt is needed", async () => {
    if (!pythonAvailable) return;

    const result = await runInteractivePty(["init"], tmpDir);

    expect(result.timedOut).toBe(false);
    expect(result.exitCode).toBe(0);
    expect(result.rawOutput).toContain("setup: OK");
  }, PTY_TIMEOUT_MS);
});
