#!/usr/bin/env bun

import { failureEnvelope } from "../src/kernel/cli.ts";
import { run } from "../src/kernel/index.ts";
import { runLifecycleCommand } from "../src/plugins/lifecycle.ts";
import { observerMode } from "../src/plugins/observer-mode.ts";
import { slpV2CliOptions } from "../src/plugins/slp-v2.ts";

const args = process.argv.slice(2);
// A reader that stops early (head, rg -m1, a narrow pane) closes the pipe; the
// output already reached it, so the write that failed is not an error of ours.
process.stdout.on("error", (error: NodeJS.ErrnoException) => {
  if (error.code === "EPIPE") process.exit(0);
  throw error;
});
if (args[0] === "--version" || args[0] === "-v") args[0] = "version";
const observer = observerMode();
const slp = slpV2CliOptions();
const cli = {
  ...observer.cli,
  async beforeInvoke(command: string, mutates: boolean) {
    await observer.cli.beforeInvoke?.(command, mutates);
    await slp.beforeInvoke?.(command, mutates);
  },
  async beforeUnknown(unknown: readonly string[]) {
    await slp.beforeUnknown?.(unknown);
    await observer.cli.beforeUnknown?.(unknown);
  },
};
try {
  process.exitCode =
    (await runLifecycleCommand(args, cli)) ??
    (await run(args, {
      allowBuiltIn: (plugin) => observer.allowBuiltIn(plugin.name),
      cli,
      readOnly: observer.enabled,
    }));
} catch (error) {
  process.stderr.write(`${JSON.stringify(failureEnvelope(error))}\n`);
  process.exitCode = 1;
}
