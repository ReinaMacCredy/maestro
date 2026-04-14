import { readdir, rm } from "node:fs/promises";
import { join } from "node:path";

export function isRootBunBuildArtifact(name: string): boolean {
  return name.endsWith(".bun-build");
}

export async function removeRootBunBuildArtifacts(root: string): Promise<string[]> {
  const entries = await readdir(root, { withFileTypes: true });
  const removed: string[] = [];

  for (const entry of entries) {
    if (!entry.isFile() || !isRootBunBuildArtifact(entry.name)) {
      continue;
    }

    const path = join(root, entry.name);
    await rm(path, { force: true });
    removed.push(path);
  }

  return removed.sort((left, right) => left.localeCompare(right));
}
