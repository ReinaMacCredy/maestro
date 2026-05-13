import { writeFile } from "node:fs/promises";
import { ensureDir, readText } from "@/shared/lib/fs.js";
import { detectHostRuntimes, type DetectedHostRuntime } from "./detect-host-runtime.usecase.js";

export interface RuntimeHookInstallResult {
  readonly runtime: DetectedHostRuntime["id"];
  readonly file: string;
  readonly status: "installed" | "already-present";
}

export interface InstallRuntimeHooksDeps {
  readonly read?: (path: string) => Promise<string | undefined>;
  readonly write?: (path: string, content: string) => Promise<void>;
}

const SENTINEL = "<!-- maestro-managed: session hooks -->";

export async function installRuntimeHooks(
  projectRoot: string,
  deps: InstallRuntimeHooksDeps = {},
): Promise<readonly RuntimeHookInstallResult[]> {
  const runtimes = await detectHostRuntimes(projectRoot);
  const results: RuntimeHookInstallResult[] = [];
  for (const runtime of runtimes) {
    const result = await installFor(runtime, deps);
    results.push(result);
  }
  return results;
}

async function installFor(
  runtime: DetectedHostRuntime,
  deps: InstallRuntimeHooksDeps,
): Promise<RuntimeHookInstallResult> {
  const read = deps.read ?? readText;
  const write = deps.write ?? writeFileSafe;

  const existing = await read(runtime.hooksFile);
  const payload = renderHookPayload(runtime.id);

  if (existing && existing.includes(SENTINEL)) {
    return { runtime: runtime.id, file: runtime.hooksFile, status: "already-present" };
  }

  const next = existing && existing.trim().length > 0
    ? mergeHookIntoExisting(existing, payload)
    : `${SENTINEL}\n${payload}\n`;

  await write(runtime.hooksFile, next);
  return { runtime: runtime.id, file: runtime.hooksFile, status: "installed" };
}

function renderHookPayload(runtime: DetectedHostRuntime["id"]): string {
  return `# Maestro session hooks (${runtime})

This file is informational. To activate these hooks in the host runtime,
wire them into the runtime's native config (e.g. \`hooks.SessionStart\` /
\`hooks.SessionEnd\` in Claude Code's \`settings.json\`).

- SessionStart -> \`maestro session start "$TASK_ID"\`
- SessionEnd   -> \`maestro session exit "$TASK_ID"\``;
}

function mergeHookIntoExisting(existing: string, payload: string): string {
  const trimmed = existing.endsWith("\n") ? existing : `${existing}\n`;
  return `${trimmed}\n${SENTINEL}\n${payload}\n`;
}

async function writeFileSafe(path: string, content: string): Promise<void> {
  const dir = path.slice(0, path.lastIndexOf("/"));
  if (dir.length > 0) await ensureDir(dir);
  await writeFile(path, content, "utf8");
}
