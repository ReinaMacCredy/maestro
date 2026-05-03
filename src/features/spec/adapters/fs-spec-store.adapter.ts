/**
 * Filesystem-backed spec store.
 *
 * Layout: `.maestro/specs/<mission-id>.json` — one file per Mission Spec.
 *
 * Writes are atomic via `writeJson` (write-tmp-then-rename). Reads are
 * tolerant: malformed JSON or specs with a non-current `schema_version` are
 * silently skipped so a single bad file does not poison `list()`.
 *
 * Specs are committed (not gitignored) so the team shares them.
 */
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readJson, writeJson } from "@/shared/lib/fs.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import type { Spec } from "../domain/types.js";
import type { SpecStorePort } from "../ports/storage.js";

const SPECS_DIR = "specs";
const CURRENT_SCHEMA_VERSION = 1;

/**
 * Segment pattern for mission IDs.
 * Mission IDs look like "2026-05-04-001" — date + 3-digit sequence.
 */
const MISSION_ID_PATTERN = /^[\w][\w.-]*$/;

export class FsSpecStoreAdapter implements SpecStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, SPECS_DIR);
  }

  private specPath(missionId: string): string {
    assertSafeSegment(missionId, "mission ID", MISSION_ID_PATTERN, "word characters, hyphens, or dots");
    return resolveWithin(this.dir(), `${missionId}.json`, "Spec path");
  }

  async write(spec: Spec): Promise<void> {
    await ensureDir(this.dir());
    await writeJson(this.specPath(spec.mission_id), spec);
  }

  async read(missionId: string): Promise<Spec | undefined> {
    const path = this.specPath(missionId);
    let raw: unknown;
    try {
      raw = await readJson<unknown>(path);
    } catch {
      return undefined;
    }
    if (!isSpec(raw)) return undefined;
    return raw;
  }

  async list(): Promise<Spec[]> {
    let entries;
    try {
      entries = await readdir(this.dir(), { withFileTypes: true });
    } catch {
      return [];
    }
    const specs: Spec[] = [];
    for (const entry of entries) {
      if (!entry.isFile() || !entry.name.endsWith(".json")) continue;
      const missionId = entry.name.slice(0, -".json".length);
      const raw = await readJson<unknown>(join(this.dir(), entry.name)).catch(() => undefined);
      if (!isSpec(raw) || raw.mission_id !== missionId) continue;
      specs.push(raw);
    }
    return specs.sort((a, b) => a.mission_id.localeCompare(b.mission_id));
  }
}

function isSpec(value: unknown): value is Spec {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    v["schema_version"] === CURRENT_SCHEMA_VERSION
    && typeof v["mission_id"] === "string"
    && Array.isArray(v["acceptance_criteria"])
    && Array.isArray(v["non_goals"])
    && Array.isArray(v["runtime_signals"])
    && typeof v["created_at"] === "string"
    && typeof v["updated_at"] === "string"
  );
}
