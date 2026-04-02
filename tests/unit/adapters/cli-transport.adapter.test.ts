import { describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { CliTransportAdapter } from "../../../src/adapters/cli-transport.adapter.js";

async function initGitRepo(cwd: string): Promise<void> {
  const proc = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await proc.exited;
}

describe("CliTransportAdapter", () => {
  it("pipes prompts to a raw-output worker and captures changed files", async () => {
    const cwd = await mkdtemp(join(tmpdir(), "cli-transport-"));
    await initGitRepo(cwd);
    await mkdir(join(cwd, ".maestro"), { recursive: true });
    const script = join(cwd, "worker.ts");
    await writeFile(
      script,
      [
        "const input = await new Response(Bun.stdin).text();",
        "await Bun.write('created.txt', input.trim());",
        "console.log(JSON.stringify({ salientSummary: 'worker ok', whatWasImplemented: 'did it', whatWasLeftUndone: '', verification: { commandsRun: [], interactiveChecks: [] }, tests: { added: [] }, discoveredIssues: [] }));",
      ].join("\n"),
    );

    const adapter = new CliTransportAdapter();
    const result = await adapter.spawn(
      {
        enabled: true,
        transport: "cli",
        command: "bun",
        args: [script],
        outputMode: "raw",
      },
      "hello world",
      {
        cwd,
        featureId: "f1",
        missionId: "m1",
        workerSlug: "codex",
      },
    );

    expect(result.success).toBe(true);
    expect(result.summary).toContain("salientSummary");
    expect(result.filesChanged).toContain("created.txt");
  });

  it("parses stream-json worker output", async () => {
    const cwd = await mkdtemp(join(tmpdir(), "cli-transport-"));
    await initGitRepo(cwd);
    const script = join(cwd, "stream-worker.ts");
    await writeFile(
      script,
      "console.log(JSON.stringify({ type: 'result', result: 'stream ok' }));",
    );

    const adapter = new CliTransportAdapter();
    const result = await adapter.spawn(
      {
        enabled: true,
        transport: "cli",
        command: "bun",
        args: [script],
        outputMode: "stream-json",
      },
      "ignored",
      {
        cwd,
        featureId: "f1",
        missionId: "m1",
        workerSlug: "claude-code",
      },
    );

    expect(result.parsedOutput).toBe("stream ok");
    expect(result.summary).toBe("stream ok");
  });
});
