#!/usr/bin/env bun

import { run } from "../src/kernel/index.ts";
import { runLifecycleCommand } from "../src/plugins/lifecycle.ts";

const args = process.argv.slice(2);
if (args[0] === "--version" || args[0] === "-v") args[0] = "version";
process.exitCode = (await runLifecycleCommand(args)) ?? (await run(args));
