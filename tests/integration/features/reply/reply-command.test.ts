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

let tmpDir: string;

async function runReplyCommand(
  args: readonly string[],
): Promise<{
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}> {
  return runCommand(CLI.concat(args), tmpDir);
}

describe("reply command integration", () => {
  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-reply-command-"));
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("writes a reply in json mode and lists it in text mode", async () => {
    const reportFile = join(tmpDir, "report.json");
    await writeFile(reportFile, JSON.stringify({
      content: "Completed the feature",
    }));

    const writeResult = await runReplyCommand([
      "reply",
      "write",
      "f-42",
      "--mission",
      "2026-04-15-001",
      "--outcome",
      "completed",
      "--note",
      "line\u001b[31m alert\u001b[0m",
      "--source",
      "cli:test",
      "--agent",
      "--report-file",
      reportFile,
      "--json",
    ]);

    expect(writeResult.exitCode).toBe(0);
    const reply = JSON.parse(writeResult.stdout);
    expect(reply).toMatchObject({
      missionId: "2026-04-15-001",
      featureId: "f-42",
      outcome: "completed",
      notes: "line\u001b[31m alert\u001b[0m",
      writtenBy: "agent",
      source: "cli:test",
    });
    expect(reply.report.salientSummary).toBe("Completed the feature");

    const listResult = await runReplyCommand(["reply", "list"]);
    expect(listResult.exitCode).toBe(0);
    expect(listResult.stdout).toContain("2026-04-15-001/f-42 [completed]");
    expect(listResult.stdout).toContain("note: line alert");
  });

  it("lists an empty state cleanly", async () => {
    const result = await runReplyCommand(["reply", "list"]);

    expect(result.exitCode).toBe(0);
    expect(result.stdout).toBe("(no replies on disk)");
  });

  it("rejects missing or invalid outcomes", async () => {
    const result = await runReplyCommand([
      "reply",
      "write",
      "f-42",
      "--mission",
      "2026-04-15-001",
      "--outcome",
      "wrong",
    ]);

    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("--outcome is required");
    expect(result.stderr).toContain("maestro reply write f-42");
  });

  it("fails clearly when the report file cannot be read", async () => {
    const result = await runReplyCommand([
      "reply",
      "write",
      "f-42",
      "--mission",
      "2026-04-15-001",
      "--outcome",
      "completed",
      "--report-file",
      join(tmpDir, "missing.json"),
    ]);

    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("Cannot read --report-file");
    expect(result.stderr).toContain("Check the path exists");
  });

  it("fails clearly when the report file is not valid json", async () => {
    const reportFile = join(tmpDir, "bad-report.json");
    await writeFile(reportFile, "{bad json");

    const result = await runReplyCommand([
      "reply",
      "write",
      "f-42",
      "--mission",
      "2026-04-15-001",
      "--outcome",
      "completed",
      "--report-file",
      reportFile,
    ]);

    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("--report-file is not valid JSON");
    expect(result.stderr).toContain("Provide a JSON file matching the WorkerReport schema");
  });
});
