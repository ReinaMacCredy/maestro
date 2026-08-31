#!/usr/bin/env bun

import { resolve } from "node:path";
import { runSlpWatch } from "../src/plugins/slp-watch.ts";

function option(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

const teamId = option("--team");
const generation = Number(option("--generation"));
const intervalMs = Number(option("--interval-ms") ?? "1000");
const workspaceId = process.env.HERDR_WORKSPACE_ID;

if (!teamId || !/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(teamId)) {
  process.stderr.write("maestro-slp-watch: --team must be a normalized team id\n");
  process.exit(2);
}
if (!Number.isInteger(generation) || generation < 1) {
  process.stderr.write("maestro-slp-watch: --generation must be a positive integer\n");
  process.exit(2);
}
if (!Number.isFinite(intervalMs) || intervalMs < 25) {
  process.stderr.write("maestro-slp-watch: --interval-ms must be at least 25\n");
  process.exit(2);
}
if (!workspaceId) {
  process.stderr.write("maestro-slp-watch: HERDR_WORKSPACE_ID is required\n");
  process.exit(2);
}

try {
  process.exitCode = await runSlpWatch({
    generation,
    intervalMs,
    projectPath: resolve(process.cwd()),
    teamId,
    workspaceId,
  });
} catch (error) {
  process.stderr.write(
    `maestro-slp-watch: ${error instanceof Error ? error.message : String(error)}\n`,
  );
  process.exitCode = 1;
}
