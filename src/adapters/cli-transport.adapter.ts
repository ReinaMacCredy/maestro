import type { WorkerConfig, WorkerResult } from "../domain/worker-types.js";
import { execArgv } from "../lib/shell.js";
import { parseRawOutput, parseStreamJsonOutput } from "../lib/stream-json-parser.js";
import type { TransportPort } from "../ports/transport.port.js";

export class CliTransportAdapter implements TransportPort {
  async spawn(
    workerConfig: WorkerConfig,
    prompt: string,
    opts: {
      cwd: string;
      featureId: string;
      missionId: string;
      workerSlug: string;
    },
  ): Promise<WorkerResult> {
    const startedAt = Date.now();
    let proc;
    try {
      proc = Bun.spawn([workerConfig.command, ...(workerConfig.args ?? [])], {
        cwd: opts.cwd,
        stdin: "pipe",
        stdout: "pipe",
        stderr: "pipe",
        env: workerConfig.env,
      });
    } catch {
      return {
        success: false,
        exitCode: 127,
        summary: `Failed to start worker '${opts.workerSlug}'`,
        stdoutRaw: "",
        stderrRaw: `Command not found: ${workerConfig.command}`,
        filesChanged: await captureChangedFiles(opts.cwd),
        durationMs: Date.now() - startedAt,
        failureClass: "infrastructure",
      };
    }

    proc.stdin.write(`${prompt}\n`);
    proc.stdin.end();

    const [stdoutRaw, stderrRaw] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ]);
    const exitCode = await proc.exited;
    const durationMs = Date.now() - startedAt;
    const parsedOutput = workerConfig.outputMode === "stream-json"
      ? parseStreamJsonOutput(stdoutRaw, opts.workerSlug)
      : parseRawOutput(stdoutRaw);
    const summary = summarizeWorkerOutput(parsedOutput, stderrRaw, exitCode, opts.workerSlug);

    return {
      success: exitCode === 0,
      exitCode,
      summary,
      stdoutRaw: stdoutRaw.trim(),
      stderrRaw: stderrRaw.trim(),
      filesChanged: await captureChangedFiles(opts.cwd),
      durationMs,
      parsedOutput,
      failureClass: exitCode === 0
        ? undefined
        : exitCode === 127
          ? "infrastructure"
          : "worker-crash",
    };
  }
}

function summarizeWorkerOutput(
  parsedOutput: string,
  stderrRaw: string,
  exitCode: number,
  workerSlug: string,
): string {
  const firstParsedLine = parsedOutput
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0);
  if (firstParsedLine) {
    return firstParsedLine;
  }

  const firstErrorLine = stderrRaw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0);
  if (firstErrorLine) {
    return firstErrorLine;
  }

  return exitCode === 0
    ? `${workerSlug} completed successfully`
    : `${workerSlug} exited with code ${exitCode}`;
}

async function captureChangedFiles(cwd: string): Promise<readonly string[]> {
  const result = await execArgv(["git", "status", "--short"], { cwd });
  if (result.exitCode !== 0 || result.stdout.length === 0) {
    return [];
  }

  return result.stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line) => {
      const renamed = line.match(/^.. (.+?) -> (.+)$/);
      if (renamed) {
        return renamed[2];
      }
      return line.slice(3).trim();
    })
    .filter((value) => value.length > 0);
}
