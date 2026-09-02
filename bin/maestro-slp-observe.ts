#!/usr/bin/env bun

import { resolve } from "node:path";
import { runSlpObserve, sentinelPollMs, sentinelTickMs } from "../src/plugins/slp-observe.ts";

function option(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

const teamId = option("--team");
const generation = Number(option("--generation"));
const tickMs = Number(option("--tick-ms") ?? String(sentinelTickMs));
const pollMs = Number(option("--poll-ms") ?? String(sentinelPollMs));
const workspaceId = process.env.HERDR_WORKSPACE_ID;

if (!teamId || !/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(teamId)) {
  process.stderr.write("maestro-slp-observe: --team must be a normalized team id\n");
  process.exit(2);
}
if (!Number.isInteger(generation) || generation < 1) {
  process.stderr.write("maestro-slp-observe: --generation must be a positive integer\n");
  process.exit(2);
}
if (!Number.isFinite(tickMs) || tickMs < 25 || !Number.isFinite(pollMs) || pollMs < 25) {
  process.stderr.write("maestro-slp-observe: --tick-ms and --poll-ms must be at least 25\n");
  process.exit(2);
}
if (!workspaceId) {
  process.stderr.write("maestro-slp-observe: HERDR_WORKSPACE_ID is required\n");
  process.exit(2);
}

try {
  process.exitCode = await runSlpObserve({
    generation,
    pollMs,
    projectPath: resolve(process.cwd()),
    teamId,
    tickMs,
    workspaceId,
  });
} catch (error) {
  process.stderr.write(
    `maestro-slp-observe: ${error instanceof Error ? error.message : String(error)}\n`,
  );
  process.exitCode = 1;
}
