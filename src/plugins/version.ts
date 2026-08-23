import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { CliError, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";

export const installStampFile = ".maestro-install.json";

interface InstallStamp {
  commit: string;
  installedAt: string;
  version: string;
}

interface PackageJson {
  version: string;
}

function validStamp(value: unknown): value is InstallStamp {
  if (!value || typeof value !== "object") return false;
  const stamp = value as Partial<InstallStamp>;
  return (
    typeof stamp.commit === "string" &&
    typeof stamp.installedAt === "string" &&
    typeof stamp.version === "string"
  );
}

async function readStamp(root: string): Promise<InstallStamp | null> {
  const path = join(root, installStampFile);
  if (!existsSync(path)) return null;
  try {
    const stamp = JSON.parse(await readFile(path, "utf8")) as unknown;
    if (validStamp(stamp)) return stamp;
  } catch {
    // Report the same actionable error for malformed JSON and invalid fields.
  }
  throw new CliError(
    "INVALID_INSTALL_STAMP",
    "runtime install stamp is invalid; run maestro install from the Maestro source checkout",
  );
}

export const versionPlugin: BuiltInPlugin = {
  name: "version",
  apply(context) {
    context.effect(() =>
      context.cli.register("version", async (): Promise<CliResult> => {
        const root = resolve(import.meta.dir, "..", "..");
        const packageJson = JSON.parse(
          await readFile(join(root, "package.json"), "utf8"),
        ) as PackageJson;
        const stamp = await readStamp(root);
        return {
          data: stamp ?? { version: packageJson.version, source: "dev" },
          text: stamp
            ? `maestro ${packageJson.version}\ncommit ${stamp.commit}\ninstalled ${stamp.installedAt}`
            : `maestro ${packageJson.version} (source/dev)`,
        };
      }),
    );
  },
};
