import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expectJson, initGitRepo } from "../../../helpers/run-compiled-cli.js";
import { runCli } from "../../../helpers/run-cli.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-receipt-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task completion receipt feeds hint keywords", () => {
  it("surfaces a hint from receipt summary/surprise on related future tasks", async () => {
    const firstCreate = await runCli(
      ["task", "create", "write authentication middleware", "--json"],
      tmpDir,
    );
    const first = expectJson<{ id: string }>(firstCreate);

    await runCli(
      ["task", "claim", first.id, "--session", "test-alpha"],
      tmpDir,
    );
    await runCli(
      ["task", "update", first.id, "--status", "in_progress", "--session", "test-alpha"],
      tmpDir,
    );
    await runCli(
      [
        "task",
        "update",
        first.id,
        "--status",
        "completed",
        "--session",
        "test-alpha",
        "--summary",
        "added argon2 hashing to the session pipeline",
        "--surprise",
        "bcrypt legacy records broke on rotation",
      ],
      tmpDir,
    );

    const showCompleted = await runCli(["task", "show", first.id, "--json"], tmpDir);
    const showed = expectJson<{ receipt?: { summary: string; surprise?: string } }>(showCompleted);
    expect(showed.receipt?.summary).toBe("added argon2 hashing to the session pipeline");
    expect(showed.receipt?.surprise).toBe("bcrypt legacy records broke on rotation");

    const secondCreate = await runCli(
      ["task", "create", "rotate argon2 session keys", "--json"],
      tmpDir,
    );
    const second = expectJson<{ id: string }>(secondCreate);

    const readyResult = await runCli(["task", "ready", "--json"], tmpDir);
    const ready = expectJson<Array<{
      id: string;
      hints: Array<{ sourceTaskId: string; reason: string; matchedKeywords: readonly string[] }>;
    }>>(readyResult);

    const forSecond = ready.find((task) => task.id === second.id);
    expect(forSecond?.hints.length ?? 0).toBeGreaterThan(0);
    const matchedHint = forSecond?.hints.find((hint) => hint.sourceTaskId === first.id);
    expect(matchedHint).toBeDefined();
    expect(matchedHint?.matchedKeywords).toContain("argon2");
  });
});
