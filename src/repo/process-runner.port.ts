// Process runner port lets services exec shell commands (e.g. principle
// scan commands) without the service layer importing Bun or node:child_process
// directly. Tests can pass an in-memory mock.

export interface ProcessRunOptions {
  readonly cwd?: string;
}

export interface ProcessRunResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

export interface ProcessRunnerPort {
  run(command: string, options?: ProcessRunOptions): Promise<ProcessRunResult>;
}
