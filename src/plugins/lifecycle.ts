import { resolve } from "node:path";
import { Cli, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import { uninstallRepo } from "./install.ts";

async function uninstall(): Promise<CliResult> {
  const repo = resolve(process.cwd());
  const removed = await uninstallRepo(repo);
  return {
    data: { removed },
    text: removed.length > 0
      ? `maestro uninstall removed ${removed.length} managed item${removed.length === 1 ? "" : "s"}`
      : "maestro uninstall: no changes",
  };
}

function registerLifecycle(cli: Cli): void {
  cli.register("uninstall", uninstall, {
    description: "Remove Maestro-managed wiring from the current repository.",
  });
}

export async function runLifecycleCommand(args: string[]): Promise<number | null> {
  if (args[0] !== "uninstall") return null;
  const cli = new Cli();
  registerLifecycle(cli);
  return cli.dispatch(args);
}

export const lifecyclePlugin: BuiltInPlugin = {
  name: "lifecycle",
  apply(context) {
    context.effect(() =>
      context.cli.register("uninstall", uninstall, {
        description: "Remove Maestro-managed wiring from the current repository.",
      }),
    );
  },
};
