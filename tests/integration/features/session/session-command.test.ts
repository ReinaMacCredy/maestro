import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runCommand } from "../../../helpers/command-runner.js";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "..", "..", "src", "index.ts"),
];

const NO_AGENT_ENV = {
  CLAUDECODE: "",
  CODEX_THREAD_ID: "",
};

let tmpDir: string;

async function runSessionCommand(
  args: readonly string[],
  env?: Record<string, string>,
): Promise<{
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}> {
  return runCommand(CLI.concat(args), tmpDir, { env });
}

describe("session command integration", () => {
  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-session-source-"));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("reports a structured json error from the source CLI when no session exists", async () => {
    const result = await runSessionCommand(["session", "--json"], {
      ...process.env,
      ...NO_AGENT_ENV,
    });

    expect(result.exitCode).toBe(1);
    expect(JSON.parse(result.stdout)).toMatchObject({
      error: "No session detected",
    });
    expect(result.stderr).toBe("");
  });

  it("returns the detected codex session in json mode", async () => {
    const homeDir = join(tmpDir, "home");
    const sessionsDir = join(homeDir, ".codex", "sessions", "2026", "04", "15");
    const threadId = "thread-abc";
    const filename = "rollout-2026-04-15T09-08-07-thread-abc.jsonl";
    const sourcePath = join(sessionsDir, filename);
    await mkdir(sessionsDir, { recursive: true });
    await writeFile(sourcePath, '{"event":"started"}\n');

    const result = await runSessionCommand(["session", "--json"], {
      ...process.env,
      HOME: homeDir,
      USERPROFILE: homeDir,
      CODEX_THREAD_ID: threadId,
      CLAUDECODE: "",
    });

    expect(result.exitCode).toBe(0);
    const parsed = JSON.parse(result.stdout);
    expect(parsed.session.agent).toBe("codex");
    expect(parsed.session.sessionId).toBe(threadId);
    expect(parsed.session.sourcePath).toBe(sourcePath);
    expect(typeof parsed.session.startedAt).toBe("number");
  });

  it("prints only the session id in quiet mode", async () => {
    const homeDir = join(tmpDir, "home");
    const sessionsDir = join(homeDir, ".codex", "sessions");
    const threadId = "thread-quiet";
    await mkdir(sessionsDir, { recursive: true });
    await writeFile(
      join(sessionsDir, "rollout-2026-04-15T10-11-12-thread-quiet-extra.jsonl"),
      '{"event":"started"}\n',
    );

    const result = await runSessionCommand(["session", "--quiet"], {
      ...process.env,
      HOME: homeDir,
      USERPROFILE: homeDir,
      CODEX_THREAD_ID: threadId,
      CLAUDECODE: "",
    });

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toBe("thread-quiet-extra");
    expect(result.stderr).toBe("");
  });
});
