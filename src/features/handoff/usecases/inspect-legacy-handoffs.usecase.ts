import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";

export async function countLegacyHandoffFiles(projectDir: string): Promise<number> {
  const legacyDir = join(projectDir, MAESTRO_DIR, "handoffs");
  try {
    const entries = await readdir(legacyDir, { withFileTypes: true });
    return entries.filter((entry) => entry.isFile() || entry.isDirectory()).length;
  } catch {
    return 0;
  }
}
