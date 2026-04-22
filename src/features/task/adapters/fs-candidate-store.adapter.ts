/**
 * Filesystem adapter for task candidates.
 *
 * Storage layout: `.maestro/tasks/candidates/<id>.json`
 * One file per candidate, hand-editable, trivially git-friendly.
 * Read-many / write-rarely pattern — candidates are written once on
 * `task close` and then read on every `task ready` query, so the
 * per-file layout beats a single JSONL append in terms of clarity
 * without hurting performance at maestro's scale.
 */

import { join } from "node:path";
import { readdir } from "node:fs/promises";
import type { TaskCandidate } from "../domain/task-candidate.js";
import type {
  CandidateStorePort,
  CreateCandidateInput,
} from "../ports/candidate-store.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readJson, removeIfExists, writeJson } from "@/shared/lib/fs.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import { validateTaskCandidate } from "../domain/task-candidate.js";

// Defense in depth for the write path. `validateTaskCandidate` enforces the
// stricter TASK_ID_PATTERN on read; this keeps the adapter symmetric with
// the launch/feature/reply stores, which all use assertSafeSegment.
const SAFE_CANDIDATE_ID = /^[A-Za-z0-9._-]+$/;

export class FsCandidateStoreAdapter implements CandidateStorePort {
  constructor(private readonly baseDir: string) {}

  private candidatesDir(): string {
    return join(this.baseDir, MAESTRO_DIR, "tasks", "candidates");
  }

  private candidatePath(id: string): string {
    assertSafeSegment(id, "candidate id", SAFE_CANDIDATE_ID, "alphanumerics, '.', '_', or '-'");
    return resolveWithin(this.candidatesDir(), `${id}.json`, "Candidate storage path");
  }

  async create(input: CreateCandidateInput): Promise<TaskCandidate> {
    await ensureDir(this.candidatesDir());
    const candidate: TaskCandidate = {
      id: input.id,
      sourceTaskId: input.sourceTaskId,
      sourceType: "task-close",
      title: input.title,
      reason: input.reason,
      keywords: input.keywords,
      capturedAt: new Date().toISOString(),
    };
    await writeJson(this.candidatePath(input.id), candidate);
    return candidate;
  }

  async all(): Promise<readonly TaskCandidate[]> {
      let entries;
      try {
        entries = await readdir(this.candidatesDir(), { encoding: "utf8", withFileTypes: true });
      } catch (err: unknown) {
        if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
        throw err;
      }

    const candidateReads = entries
      .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
      .map(async (entry) => {
        try {
          return await readJson<unknown>(join(this.candidatesDir(), entry.name));
        } catch {
          return undefined;
        }
      });

    const loaded = await Promise.all(candidateReads);
    return loaded.flatMap((raw) => {
      const validated = validateTaskCandidate(raw);
      return validated ? [validated] : [];
    });
  }

  async delete(id: string): Promise<void> {
    await removeIfExists(this.candidatePath(id));
  }
}
