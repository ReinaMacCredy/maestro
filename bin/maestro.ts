#!/usr/bin/env bun

import { failureEnvelope } from "../src/kernel/cli.ts";
import { run } from "../src/kernel/index.ts";
import { observerMode } from "../src/plugins/observer-mode.ts";

const args = process.argv.slice(2);
// A reader that stops early (head, rg -m1, a narrow pane) closes the pipe; the
// output already reached it, so the write that failed is not an error of ours.
process.stdout.on("error", (error: NodeJS.ErrnoException) => {
  if (error.code === "EPIPE") process.exit(0);
  throw error;
});
if (args[0] === "--version" || args[0] === "-v") args[0] = "version";
const observer = observerMode();
try {
  process.exitCode = await run(args, {
    allowBuiltIn: (plugin) => observer.allowBuiltIn(plugin.name),
    cli: observer.cli,
    readOnly: observer.enabled,
    // doctor diagnoses the store, update swaps the runtime under it, uninstall must not create one; none of them may be stopped by a broken config.
    storeInMemory: ["doctor", "uninstall", "update"].includes(args[0] ?? ""),
    tolerateConfig: ["doctor", "uninstall", "update"].includes(args[0] ?? ""),
  });
} catch (error) {
  process.stderr.write(`${JSON.stringify(failureEnvelope(error))}\n`);
  process.exitCode = 1;
}
