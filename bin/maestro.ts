#!/usr/bin/env bun

import { run } from "../src/kernel/index.ts";
import { runLifecycleCommand } from "../src/plugins/lifecycle.ts";
import { observerMode } from "../src/plugins/observer-mode.ts";
import { pluginTrustPredicate } from "../src/plugins/plugin-trust.ts";

const args = process.argv.slice(2);
if (args[0] === "--version" || args[0] === "-v") args[0] = "version";
const observer = observerMode();
process.exitCode =
  (await runLifecycleCommand(args, observer.cli)) ??
  (await run(args, {
    allowBuiltIn: (plugin) => observer.allowBuiltIn(plugin.name),
    cli: observer.cli,
    loadExternalPlugins: observer.loadExternalPlugins,
    readOnly: observer.enabled,
    trustExternalPlugin: pluginTrustPredicate(process.env.HOME ?? process.cwd()),
  }));
