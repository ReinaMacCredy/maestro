import { createHash, randomUUID } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { CliError } from "../../kernel/cli.ts";
import { resolveHomeDirectory } from "../home.ts";
import { resolveSeat, seatDigest, seatFileDirectories, seatNames, type Seat } from "./seats.ts";
import {
  buildSlpTeamPlan,
  type SeatLaunch,
  type SlpRole,
  type SlpRoleContract,
  type SlpTeamPlan,
} from "./runtime-plan.ts";

export interface PackModels {
  lead?: string;
  peer?: string;
}

// profileDigests keeps its stored name so generations started before d906 still read.
export interface PackConfiguration {
  models?: PackModels;
  profileDigests: Record<string, string>;
}

function packModels(value: unknown): PackModels | undefined {
  if (!value || typeof value !== "object") return undefined;
  const raw = value as Record<string, unknown>;
  const models: PackModels = {};
  for (const seat of seatNames) {
    const model = raw[seat];
    if (typeof model === "string" && model.trim() !== "") models[seat] = model;
  }
  return Object.keys(models).length > 0 ? models : undefined;
}

export function packConfiguration(json: string): PackConfiguration {
  const parsed = JSON.parse(json) as Partial<PackConfiguration>;
  const models = packModels(parsed.models);
  return {
    ...(models ? { models } : {}),
    profileDigests: parsed.profileDigests ?? {},
  };
}

function seatLaunch(seat: Seat, model: string | undefined): SeatLaunch {
  return { autocompact: seat.config.autocompact ?? null, harness: seat.config.harness, model: model ?? null };
}

export async function resolveSeats(projectPath: string, models: PackModels = {}): Promise<{ lead: SeatLaunch; profileDigests: Record<string, string> }> {
  const home = resolveHomeDirectory();
  const lead = await resolveSeat("lead", projectPath, home);
  const peer = await resolveSeat("peer", projectPath, home);
  return { lead: seatLaunch(lead, models.lead), profileDigests: { lead: seatDigest(lead), peer: seatDigest(peer) } };
}

// A seat file that no longer parses must not stop a team from being inspected or closed.
export async function planForTeam(team: {
  configuration_json: string;
  generation: number;
  project_path: string;
  team_id: string;
}): Promise<SlpTeamPlan> {
  const { models = {} } = packConfiguration(team.configuration_json);
  const lead = await resolveSeat("lead", team.project_path, resolveHomeDirectory())
    .then((seat) => seatLaunch(seat, models.lead))
    .catch((): SeatLaunch => ({ autocompact: null, harness: "codex", model: models.lead ?? null }));
  return buildSlpTeamPlan({
    generation: team.generation,
    lead,
    projectPath: team.project_path,
    teamId: team.team_id,
  });
}

// A pin under a name that is no longer a seat (a peer variant from before d906) has nothing left to compare.
export async function requireSeatsUnchanged(
  configuration: PackConfiguration,
  team: { generation: number; project_path: string; team_id: string },
): Promise<void> {
  for (const name of seatNames) {
    const digest = configuration.profileDigests[name];
    if (digest === undefined) continue;
    const seat = await resolveSeat(name, team.project_path, resolveHomeDirectory());
    const actual = seatDigest(seat);
    if (actual === digest) continue;
    const where = seat.paths.length > 1 ? `${seat.paths[0]} (frontmatter) and ${seat.paths[1]} (mandate)` : seat.paths[0];
    throw new CliError(
      "SLP_SNAPSHOT_CHANGED",
      `running generation ${team.team_id}:g${team.generation} must keep its pinned ${name} seat file, whose bytes are decided by ${where}; put those bytes back as they were pinned, or stop the generation with maestro team stop ${team.team_id}`,
      { actual, deciding: seat.paths, expected: digest, searched: seatFileDirectories(team.project_path, resolveHomeDirectory()), seat: name },
    );
  }
}

export function packVersion(pack: string): string {
  const version = /<!-- slp:version=([^\s]+) -->/.exec(pack)?.[1];
  if (!version) throw new CliError("INVALID_SLP_PACK", "SLP.md is missing its version marker");
  return version;
}

// d90: the mandate lives in the seat file, so the post-open prompt carries only
// team, generation, instance and the ready challenge.
export function roleContracts(
  teamId: string,
  generation: number,
  packDigest: string,
  existingInstances: Partial<Record<SlpRole, string>> = {},
): Map<SlpRole, SlpRoleContract> {
  const contract = (role: SlpRole): SlpRoleContract => {
    const instanceId = existingInstances[role] || randomUUID();
    const readyChallenge = randomUUID().replaceAll("-", "");
    const body = `slp team ${teamId} generation ${generation} instance ${instanceId}; reply ${readyChallenge}`;
    const briefDigest = createHash("sha256").update(body).digest("hex");
    const acknowledgement = [
      "SLP_ROLE_READY",
      `team=${teamId}`,
      `generation=${generation}`,
      `role=${role}`,
      `challenge=${readyChallenge}`,
    ].join(" ");
    return { acknowledgement, body, briefDigest, instanceId, packDigest, readyChallenge };
  };
  return new Map<SlpRole, SlpRoleContract>([
    ["lead", contract("lead")],
    ["peer", contract("peer")],
  ]);
}

export async function archivePack(roomRoot: string, bytes: Uint8Array, digest: string): Promise<string> {
  const directory = join(roomRoot, ".maestro", "packs");
  const path = join(directory, `${digest}.md`);
  await mkdir(directory, { recursive: true });
  if (!existsSync(path)) {
    const temporary = join(directory, `.${digest}.${randomUUID()}.tmp`);
    try {
      await writeFile(temporary, bytes, { flag: "wx" });
      await rename(temporary, path);
    } finally {
      await rm(temporary, { force: true });
    }
  }
  const archived = new Uint8Array(await readFile(path));
  const archivedDigest = createHash("sha256").update(archived).digest("hex");
  if (archivedDigest !== digest) {
    throw new CliError(
      "SLP_PACK_ARCHIVE_CORRUPT",
      `archived Workspace Pack ${path} does not match digest ${digest}`,
      { actual: archivedDigest, expected: digest, path },
    );
  }
  return path;
}
