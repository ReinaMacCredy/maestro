import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { CliError, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import { readInstallStamp } from "./install-stamp.ts";

interface PackageJson {
  version: string;
}

export const versionPlugin: BuiltInPlugin = {
  name: "version",
  apply(context) {
    context.effect(() =>
      context.cli.register(
        "version",
        async (): Promise<CliResult> => {
          const root = resolve(import.meta.dir, "..", "..");
          const [packageText, stampRead] = await Promise.all([
            readFile(join(root, "package.json"), "utf8"),
            readInstallStamp(root),
          ]);
          if (stampRead.status === "invalid") {
            throw new CliError(
              "INVALID_INSTALL_STAMP",
              "runtime install stamp is invalid; run maestro install from the Maestro source checkout",
            );
          }
          const packageJson = JSON.parse(packageText) as PackageJson;
          const stamp = stampRead.status === "valid" ? stampRead.stamp : null;
          return {
            data: stamp ?? { version: packageJson.version, source: "dev" },
            text: stamp
              ? `maestro ${packageJson.version}\ncommit ${stamp.commit}\ninstalled ${stamp.installedAt}`
              : `maestro ${packageJson.version} (source/dev)`,
          };
        },
        { description: "Show the installed or source Maestro version." },
      ),
    );
  },
};
