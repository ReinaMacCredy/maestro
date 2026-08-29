#!/usr/bin/env bun

import { runTeamSensor } from "../src/plugins/team-sensor.ts";

function option(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

const teamId = option("--team");
const generationText = option("--generation");
const observerName = option("--observer");
const workspaceId = process.env.HERDR_WORKSPACE_ID;
const generation = Number(generationText);

if (!teamId || !/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(teamId)) {
  process.stderr.write("maestro-team-sensor: --team must be a normalized team id\n");
  process.exit(2);
}
if (!Number.isInteger(generation) || generation < 1) {
  process.stderr.write("maestro-team-sensor: --generation must be a positive integer\n");
  process.exit(2);
}
if (observerName !== `observer-${teamId}`) {
  process.stderr.write(`maestro-team-sensor: --observer must be observer-${teamId}\n`);
  process.exit(2);
}
if (!workspaceId) {
  process.stderr.write("maestro-team-sensor: HERDR_WORKSPACE_ID is required\n");
  process.exit(2);
}

process.exitCode = await runTeamSensor({
  generation,
  observerName,
  repoPath: process.cwd(),
  teamId,
  workspaceId,
});
