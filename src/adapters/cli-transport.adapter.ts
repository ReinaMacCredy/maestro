import type { WorkerConfig, WorkerProgressEvent, WorkerResult } from "../domain/worker-types.js";
import { execArgv } from "../lib/shell.js";
import { parseRawOutput, parseStreamJsonOutput } from "../lib/stream-json-parser.js";
import type { TransportPort, TransportSpawnOptions } from "../ports/transport.port.js";

export class CliTransportAdapter implements TransportPort {
  async spawn(
    workerConfig: WorkerConfig,
    prompt: string,
    opts: TransportSpawnOptions,
  ): Promise<WorkerResult> {
    const startedAt = Date.now();
    const changedFilesBefore = await captureChangedFileSet(opts.cwd);
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
        filesChanged: [],
        durationMs: Date.now() - startedAt,
        failureClass: "infrastructure",
      };
    }

    await emitEvent(opts, {
      timestamp: new Date().toISOString(),
      kind: "status",
      worker: opts.workerSlug,
      runtimeState: "starting",
      text: `Started ${opts.workerSlug}`,
    });

    proc.stdin.write(`${prompt}\n`);
    proc.stdin.end();

    let heartbeat: ReturnType<typeof setInterval> | undefined;
    try {
      heartbeat = setInterval(() => {
        void emitEvent(opts, {
          timestamp: new Date().toISOString(),
          kind: "heartbeat",
          worker: opts.workerSlug,
          runtimeState: "live",
        });
      }, 15_000);

      const [stdoutRaw, stderrRaw] = await Promise.all([
        collectStream(proc.stdout, "stdout", opts),
        collectStream(proc.stderr, "stderr", opts),
      ]);
      const exitCode = await proc.exited;
      const durationMs = Date.now() - startedAt;
      const parsedOutput = workerConfig.outputMode === "stream-json"
        ? parseStreamJsonOutput(stdoutRaw, opts.workerSlug)
        : parseRawOutput(stdoutRaw);
      const summary = summarizeWorkerOutput(parsedOutput, stderrRaw, exitCode, opts.workerSlug);

      await emitEvent(opts, {
        timestamp: new Date().toISOString(),
        kind: "status",
        worker: opts.workerSlug,
        runtimeState: exitCode === 0 ? "completed" : "failed",
        text: exitCode === 0 ? `${opts.workerSlug} completed` : `${opts.workerSlug} exited with code ${exitCode}`,
      });

      return {
        success: exitCode === 0,
        exitCode,
        summary,
        stdoutRaw: stdoutRaw.trim(),
        stderrRaw: stderrRaw.trim(),
        filesChanged: await captureChangedFiles(opts.cwd, changedFilesBefore),
        durationMs,
        parsedOutput,
        failureClass: exitCode === 0
          ? undefined
          : exitCode === 127
            ? "infrastructure"
            : "worker-crash",
      };
    } finally {
      if (heartbeat) {
        clearInterval(heartbeat);
      }
    }
  }
}

async function collectStream(
  stream: ReadableStream<Uint8Array>,
  kind: "stdout" | "stderr",
  opts: TransportSpawnOptions,
): Promise<string> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let pendingLine = "";

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const chunk = decoder.decode(value, { stream: true });
      buffer += chunk;
      pendingLine += chunk;
      const lines = pendingLine.split(/\r?\n/);
      pendingLine = lines.pop() ?? "";
      for (const line of lines) {
        const text = line.trim();
        if (text.length === 0) continue;
        await emitEvent(opts, {
          timestamp: new Date().toISOString(),
          kind,
          worker: opts.workerSlug,
          runtimeState: "live",
          text,
        });
      }
    }
  } finally {
    reader.releaseLock();
  }

  const tail = decoder.decode();
  if (tail.length > 0) {
    buffer += tail;
    pendingLine += tail;
  }
  const finalLine = pendingLine.trim();
  if (finalLine.length > 0) {
    await emitEvent(opts, {
      timestamp: new Date().toISOString(),
      kind,
      worker: opts.workerSlug,
      runtimeState: "live",
      text: finalLine,
    });
  }

  return buffer;
}

async function emitEvent(
  opts: {
    onEvent?: (event: WorkerProgressEvent) => void | Promise<void>;
  },
  event: WorkerProgressEvent,
): Promise<void> {
  try {
    await opts.onEvent?.(event);
  } catch {
    // Progress telemetry should not fail worker execution.
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

async function captureChangedFiles(
  cwd: string,
  before: ReadonlySet<string>,
): Promise<readonly string[]> {
  const after = await captureChangedFileSet(cwd);
  return [...after].filter((path) => !before.has(path)).sort();
}

async function captureChangedFileSet(cwd: string): Promise<ReadonlySet<string>> {
  const result = await execArgv(["git", "status", "--short"], { cwd });
  if (result.exitCode !== 0 || result.stdout.length === 0) {
    return new Set();
  }

  return new Set(
    result.stdout
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
      .filter((value) => value.length > 0),
  );
}
