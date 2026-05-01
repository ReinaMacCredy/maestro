import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-posttooluse-"));
  await mkdir(join(tmpDir, ".maestro"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("posttooluse hook", () => {
  it("does not persist free-form status text in session events", async () => {
    const hookPath = join(import.meta.dir, "../../../hooks/posttooluse.mjs");
    const proc = spawnSync("bun", [hookPath], {
      cwd: tmpDir,
      env: {
        ...process.env,
        CLAUDE_PROJECT_DIR: tmpDir,
      },
      input: JSON.stringify({
        tool_name: "Bash",
        tool_input: {
          command: "maestro task status",
          feature: "f1",
          task: "task-1",
          status: "secret-token-123",
        },
      }),
      encoding: "utf8",
    });

    expect(proc.status).toBe(0);
    const raw = await readFile(join(tmpDir, ".maestro", "sessions", "events.jsonl"), "utf8");
    const event = JSON.parse(raw.trim()) as Record<string, unknown>;
    expect(event).toMatchObject({ tool: "Bash", feature: "f1", task: "task-1" });
    expect(event).not.toHaveProperty("status");
    expect(raw).not.toContain("secret-token-123");
  });
});
