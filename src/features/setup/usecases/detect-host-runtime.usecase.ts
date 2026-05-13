import { join } from "node:path";
import { dirExists } from "@/shared/lib/fs.js";

export type HostRuntimeId = "claude-code" | "codex" | "cursor";

export interface DetectedHostRuntime {
  readonly id: HostRuntimeId;
  readonly settingsDir: string;
  readonly hooksFile: string;
}

export interface DetectHostRuntimeDeps {
  readonly checkDir?: (path: string) => Promise<boolean>;
}

const RUNTIME_DIRS: ReadonlyArray<{ readonly id: HostRuntimeId; readonly dir: string }> = [
  { id: "claude-code", dir: ".claude" },
  { id: "codex", dir: ".codex" },
  { id: "cursor", dir: ".cursor" },
];

const HOOKS_FILENAME = "maestro-hooks.md";

export async function detectHostRuntimes(
  projectRoot: string,
  deps: DetectHostRuntimeDeps = {},
): Promise<readonly DetectedHostRuntime[]> {
  const check = deps.checkDir ?? dirExists;
  const detected: DetectedHostRuntime[] = [];
  for (const runtime of RUNTIME_DIRS) {
    const settingsDir = join(projectRoot, runtime.dir);
    if (await check(settingsDir)) {
      detected.push({
        id: runtime.id,
        settingsDir,
        hooksFile: join(settingsDir, HOOKS_FILENAME),
      });
    }
  }
  return detected;
}
