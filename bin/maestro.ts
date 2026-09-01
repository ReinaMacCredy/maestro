#!/usr/bin/env bun

import { failureEnvelope } from "../src/kernel/cli.ts";
import { run } from "../src/kernel/index.ts";
import { runLifecycleCommand } from "../src/plugins/lifecycle.ts";
import { observerMode } from "../src/plugins/observer-mode.ts";
import { pluginTrustPredicate } from "../src/plugins/plugin-trust.ts";
import { slpV2CliOptions } from "../src/plugins/slp-v2.ts";

const args = process.argv.slice(2);
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
      loadExternalPlugins: observer.loadExternalPlugins,
      readOnly: observer.enabled,
      trustExternalPlugin: pluginTrustPredicate(process.env.HOME ?? process.cwd()),
    }));
} catch (error) {
  process.stderr.write(`${JSON.stringify(failureEnvelope(error))}\n`);
  process.exitCode = 1;
}
