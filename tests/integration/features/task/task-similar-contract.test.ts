import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expectJson, initGitRepo } from "../../../helpers/run-compiled-cli.js";
import { runCli } from "../../../helpers/run-cli.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-similar-contract-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task similar contract corpus", () => {
  it("surfaces a match driven by contract intent and criteria text", async () => {
    const firstCreate = await runCli(
      ["task", "create", "unrelated archived task", "--json"],
      tmpDir,
    );
    const first = expectJson<{ id: string }>(firstCreate);

    const templatePath = join(tmpDir, "contract-template.yaml");
    await Bun.write(
      templatePath,
      [
        "intent: argon2 session rotation cleanup",
        "scope:",
        "  filesExpected:",
        "    - README.md",
        "  filesForbidden: []",
        "doneWhen:",
        "  - text: session rotation verified",
        "    kind: manual",
        "",
      ].join("\n"),
    );

    await runCli(
      ["task", "contract", "new", first.id, "--from", templatePath, "--json"],
      tmpDir,
    );

    const secondCreate = await runCli(
      ["task", "create", "plan argon2 rotation", "--json"],
      tmpDir,
    );
    const second = expectJson<{ id: string }>(secondCreate);

    const result = await runCli(["task", "similar", second.id, "--json"], tmpDir);
    const matches = expectJson<Array<{ task: { id: string }; matchedKeywords: string[] }>>(result);

    expect(matches.map((match) => match.task.id)).toContain(first.id);
    const firstMatch = matches.find((match) => match.task.id === first.id);
    expect(firstMatch?.matchedKeywords).toContain("argon2");
  });
});
