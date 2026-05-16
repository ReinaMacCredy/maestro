import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { dirExists, fileExists } from "@/shared/lib/fs.js";

export interface SetupBootstrapDeps {
  readonly repoRoot: string;
}

export interface SetupBootstrapResult {
  readonly created: readonly string[];
  readonly skipped: readonly string[];
}

const V2_DIRECTORIES: readonly string[] = [
  ".maestro/tasks",
  ".maestro/plans",
  ".maestro/evidence",
  ".maestro/runs",
  "docs/principles",
];

export async function setupBootstrap(
  deps: SetupBootstrapDeps,
): Promise<SetupBootstrapResult> {
  const created: string[] = [];
  const skipped: string[] = [];

  for (const rel of V2_DIRECTORIES) {
    const abs = join(deps.repoRoot, rel);
    if (await dirExists(abs)) {
      skipped.push(rel);
      continue;
    }
    await mkdir(abs, { recursive: true });
    await ensureGitkeep(abs);
    created.push(rel);
  }

  return { created, skipped };
}

async function ensureGitkeep(dir: string): Promise<void> {
  const path = join(dir, ".gitkeep");
  if (await fileExists(path)) return;
  await writeFile(path, "", "utf8");
}
