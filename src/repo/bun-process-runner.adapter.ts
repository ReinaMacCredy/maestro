import type {
  ProcessRunOptions,
  ProcessRunResult,
  ProcessRunnerPort,
} from "./process-runner.port.js";

declare const Bun: {
  spawn(cmd: readonly string[], opts: { cwd?: string; stdout: "pipe"; stderr: "pipe" }): {
    stdout: ReadableStream<Uint8Array>;
    stderr: ReadableStream<Uint8Array>;
    exited: Promise<number>;
  };
};

export class BunProcessRunner implements ProcessRunnerPort {
  async run(command: string, options: ProcessRunOptions = {}): Promise<ProcessRunResult> {
    const proc = Bun.spawn(["sh", "-c", command], {
      cwd: options.cwd,
      stdout: "pipe",
      stderr: "pipe",
    });
    const [stdout, stderr, exitCode] = await Promise.all([
      readStream(proc.stdout),
      readStream(proc.stderr),
      proc.exited,
    ]);
    return { stdout, stderr, exitCode };
  }
}

async function readStream(stream: ReadableStream<Uint8Array>): Promise<string> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let out = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    if (value) out += decoder.decode(value, { stream: true });
  }
  out += decoder.decode();
  return out;
}
