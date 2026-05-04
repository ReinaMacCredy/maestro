export interface CiEnv {
  readonly provider: "github-actions" | "unknown";
  readonly pr?: number;
  readonly baseRef?: string;
  readonly headSha?: string;
  readonly repository?: string;
  readonly eventPath?: string;
  readonly outputPath?: string;
  readonly token?: string;
  readonly testResultsFile?: string;
}

export function readCiEnv(
  env: NodeJS.ProcessEnv,
  opts?: {
    readonly readEvent?: (path: string) => string | undefined;
  },
): CiEnv {
  const isGha = env.GITHUB_ACTIONS === "true";
  const provider = isGha ? "github-actions" : "unknown";

  const repository = env.GITHUB_REPOSITORY;
  const headSha = env.GITHUB_SHA;
  const baseRef = env.GITHUB_BASE_REF;
  const eventPath = env.GITHUB_EVENT_PATH;
  const outputPath = env.GITHUB_OUTPUT;
  const token = env.GITHUB_TOKEN;
  const testResultsFile = env.CI_TEST_RESULTS_FILE;

  // Parse PR number from GITHUB_REF: refs/pull/<n>/merge or refs/pull/<n>/head
  let pr: number | undefined;
  const ref = env.GITHUB_REF;
  if (typeof ref === "string") {
    const match = /^refs\/pull\/(\d+)\/(merge|head)$/.exec(ref);
    if (match && match[1] !== undefined) {
      pr = parseInt(match[1], 10);
    }
  }

  // Fall back to event JSON if REF didn't match and eventPath is available
  if (pr === undefined && typeof eventPath === "string") {
    try {
      const readEvent = opts?.readEvent ?? defaultReadEvent;
      const text = readEvent(eventPath);
      if (text !== undefined) {
        const parsed = JSON.parse(text) as unknown;
        if (
          typeof parsed === "object" &&
          parsed !== null &&
          "pull_request" in parsed &&
          typeof (parsed as Record<string, unknown>).pull_request === "object" &&
          (parsed as Record<string, unknown>).pull_request !== null
        ) {
          const prObj = (parsed as Record<string, unknown>).pull_request as Record<string, unknown>;
          if (typeof prObj.number === "number") {
            pr = prObj.number;
          }
        }
      }
    } catch {
      // event parse failure is non-fatal; pr stays undefined
    }
  }

  return {
    provider,
    ...(pr !== undefined ? { pr } : {}),
    ...(typeof baseRef === "string" && baseRef.length > 0 ? { baseRef } : {}),
    ...(typeof headSha === "string" && headSha.length > 0 ? { headSha } : {}),
    ...(typeof repository === "string" && repository.length > 0 ? { repository } : {}),
    ...(typeof eventPath === "string" && eventPath.length > 0 ? { eventPath } : {}),
    ...(typeof outputPath === "string" && outputPath.length > 0 ? { outputPath } : {}),
    ...(typeof token === "string" && token.length > 0 ? { token } : {}),
    ...(typeof testResultsFile === "string" && testResultsFile.length > 0 ? { testResultsFile } : {}),
  };
}

function defaultReadEvent(path: string): string | undefined {
  // Use synchronous FS read — called only during command startup, not in hot paths.
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const fs = require("node:fs") as typeof import("node:fs");
    return fs.readFileSync(path, "utf8");
  } catch {
    return undefined;
  }
}
