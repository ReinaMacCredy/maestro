import { join } from "node:path";
import { dirExists } from "@/shared/lib/fs.js";

export type HostRuntimeId = "claude-code" | "codex" | "cursor";

export interface DetectedHostRuntime {
  readonly id: HostRuntimeId;
  readonly settingsDir: string;
  readonly settingsFile: string;
}

export interface DetectHostRuntimeDeps {
  readonly checkDir?: (path: string) => Promise<boolean>;
}

const RUNTIMES: ReadonlyArray<{
  readonly id: HostRuntimeId;
  readonly dir: string;
  readonly settings: string;
}> = [
  { id: "claude-code", dir: ".claude", settings: "settings.json" },
  { id: "codex", dir: ".codex", settings: "settings.json" },
  { id: "cursor", dir: ".cursor", settings: "settings.json" },
];

export async function detectHostRuntimes(
  projectRoot: string,
  deps: DetectHostRuntimeDeps = {},
): Promise<readonly DetectedHostRuntime[]> {
  const check = deps.checkDir ?? dirExists;
  const detected: DetectedHostRuntime[] = [];
  for (const runtime of RUNTIMES) {
    const settingsDir = join(projectRoot, runtime.dir);
    if (await check(settingsDir)) {
      detected.push({
        id: runtime.id,
        settingsDir,
        settingsFile: join(settingsDir, runtime.settings),
      });
    }
  }
  return detected;
}
