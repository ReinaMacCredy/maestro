import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

export const installStampFile = ".maestro-install.json";

export interface InstallStamp {
  commit: string;
  installedAt: string;
  version: string;
}

export type InstallStampRead =
  | { status: "invalid" }
  | { status: "missing" }
  | { stamp: InstallStamp; status: "valid" };

function validStamp(value: unknown): value is InstallStamp {
  if (!value || typeof value !== "object") return false;
  const stamp = value as Partial<InstallStamp>;
  return (
    typeof stamp.commit === "string" &&
    typeof stamp.installedAt === "string" &&
    typeof stamp.version === "string"
  );
}

export async function readInstallStamp(root: string): Promise<InstallStampRead> {
  let text: string;
  try {
    text = await readFile(join(root, installStampFile), "utf8");
  } catch (error) {
    if ((error as { code?: unknown }).code === "ENOENT") return { status: "missing" };
    throw error;
  }
  try {
    const stamp = JSON.parse(text) as unknown;
    return validStamp(stamp) ? { status: "valid", stamp } : { status: "invalid" };
  } catch {
    return { status: "invalid" };
  }
}

export async function writeInstallStamp(root: string, stamp: InstallStamp): Promise<void> {
  await writeFile(join(root, installStampFile), `${JSON.stringify(stamp)}\n`);
}
