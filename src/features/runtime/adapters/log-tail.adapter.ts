import { readFile } from "node:fs/promises";
import type {
  DevLogLine,
  DevLogTail,
  DevMetricSample,
  DevObservabilityPort,
} from "../ports/dev-observability.port.js";

/**
 * File-tail adapter for `maestro task observe logs`. Reads the file at
 * construction time (no streaming, no follow): the agent invokes the verb
 * deliberately and gets the last N lines optionally filtered by substring.
 *
 * The path resolves in this order: constructor arg → `MAESTRO_DEV_LOG_FILE`
 * env → throw. Keeping the env lookup inside the adapter (not the command)
 * lets tests inject a path and skips a tedious `??` chain in the caller.
 */
export class LogTailAdapter implements DevObservabilityPort {
  private readonly resolvedPath: string;
  private readonly DEFAULT_LINES = 100;

  constructor(filePath?: string, envSource: NodeJS.ProcessEnv = process.env) {
    const path = filePath ?? envSource.MAESTRO_DEV_LOG_FILE;
    if (!path || path.length === 0) {
      throw new Error("log-tail: no path; pass --log-file or set MAESTRO_DEV_LOG_FILE");
    }
    this.resolvedPath = path;
  }

  async queryMetric(): Promise<DevMetricSample> {
    throw new Error("LogTailAdapter does not support queryMetric; use a metrics adapter.");
  }

  async tailLogs(filter: string | undefined, lines?: number): Promise<DevLogTail> {
    const maxLines = lines ?? this.DEFAULT_LINES;
    const text = await readFile(this.resolvedPath, "utf8");
    const allLines = text.split("\n");
    // Drop a trailing empty line from a file ending in \n so the tail count
    // matches what the user expects.
    if (allLines.length > 0 && allLines[allLines.length - 1] === "") {
      allLines.pop();
    }
    const filtered = filter !== undefined && filter.length > 0
      ? allLines.filter((line) => line.includes(filter))
      : allLines;
    const tail = filtered.slice(-maxLines);
    const out: DevLogLine[] = tail.map((text) => ({ text }));
    return { lines: out, source: `file:${this.resolvedPath}` };
  }
}
