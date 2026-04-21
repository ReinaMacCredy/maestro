import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-contract-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function writeTemplate(name: string, body: string): Promise<string> {
  const path = join(tmpDir, name);
  await Bun.write(path, body);
  return path;
}

describe("task contract compiled E2E", () => {
  it("prints the plain ok marker for new and lock in silent mode", async () => {
    const taskId = (await runCompiled(["task", "create", "silent contract", "--silent"], tmpDir)).stdout;
    const templatePath = await writeTemplate(
      "contract-template.yaml",
      [
        "intent: silent contract flow",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: silent mode works",
        "",
      ].join("\n"),
    );

    const drafted = await runCompiled(
      ["task", "contract", "new", taskId, "--from", templatePath, "--silent"],
      tmpDir,
    );
    expect(drafted.stdout).toMatch(/^c-[0-9a-f]{6} \[ok\]$/);

    const contractId = drafted.stdout.split(" ")[0]!;
    const locked = await runCompiled(
      ["task", "contract", "lock", contractId, "--silent"],
      tmpDir,
    );
    expect(locked.stdout).toBe(`${contractId} [ok]`);
  }, SLOW_CLI_TIMEOUT_MS);

  it("prints the plain ok marker for discard in silent mode", async () => {
    const taskId = (await runCompiled(["task", "create", "discard silent contract", "--silent"], tmpDir)).stdout;
    const templatePath = await writeTemplate(
      "discard-template.yaml",
      [
        "intent: discard this draft",
        "scope:",
        "  filesExpected:",
        "    - src/features/task/**",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: discard works",
        "",
      ].join("\n"),
    );

    const drafted = await runCompiled(
      ["task", "contract", "new", taskId, "--from", templatePath, "--silent"],
      tmpDir,
    );
    const contractId = drafted.stdout.split(" ")[0]!;

    const discarded = await runCompiled(
      ["task", "contract", "discard", contractId, "--silent"],
      tmpDir,
    );
    expect(discarded.stdout).toBe(`${contractId} [ok]`);
  }, SLOW_CLI_TIMEOUT_MS);
});
