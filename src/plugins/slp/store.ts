import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { rm, writeFile } from "node:fs/promises";
import { basename } from "node:path";
import { CliError } from "../../kernel/cli.ts";
import { Store, resolveStoreLocation } from "../../kernel/store.ts";

export type WorkState = "OPEN" | "ACTIVE" | "RETURNED" | "DONE";

interface SlpWorkRecord {
  assignedTo: string;
  createdAt: string;
  createdBy: string;
  generation: number;
  id: string;
  objective: string;
  owner: string | null;
  state: WorkState;
  teamId: string;
  updatedAt: string;
}

export function canonicalCheckoutRoot(cwd: string): string {
  return realpathSync.native(resolveStoreLocation(cwd).root);
}

function slug(value: string): string {
  const normalized = value.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replaceAll(/^-|-$/g, "");
  if (!normalized) throw new CliError("INVALID_TEAM_ID", "project basename cannot form a team id");
  return normalized;
}

export function teamIdForProject(projectPath: string): string {
  const readable = slug(basename(projectPath)).slice(0, 10).replace(/-$/, "");
  const canonicalPath = realpathSync.native(projectPath);
  const identity = createHash("sha256").update(canonicalPath).digest("hex").slice(0, 10);
  return `${readable}-${identity}`;
}

export function toWork(row: {
  assigned_to: string;
  created_at: string;
  created_by: string;
  generation: number;
  id: string;
  objective: string;
  owner: string | null;
  state: WorkState;
  team_id: string;
  updated_at: string;
}): SlpWorkRecord {
  return {
    assignedTo: row.assigned_to,
    createdAt: row.created_at,
    createdBy: row.created_by,
    generation: row.generation,
    id: row.id,
    objective: row.objective,
    owner: row.owner,
    state: row.state,
    teamId: row.team_id,
    updatedAt: row.updated_at,
  };
}

export async function restoreSnapshot(path: string, previous: Uint8Array | null): Promise<void> {
  if (previous === null) {
    await rm(path, { force: true });
  } else {
    await writeFile(path, previous);
  }
}

export function withImmediateTransaction<T>(store: Store, action: () => T): T {
  store.database.exec("BEGIN IMMEDIATE");
  try {
    const result = action();
    store.database.exec("COMMIT");
    return result;
  } catch (error) {
    try {
      store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
}
