/**
 * Filesystem-backed reply store.
 *
 * Layout:
 *   .maestro/replies/<feature-id>.yaml     -- the reply itself
 *   .maestro/replies/<feature-id>.ingested -- sidecar marker (empty file)
 *
 * Writes are atomic (write-then-rename). Reads are tolerant: malformed YAML
 * or missing required fields are logged and skipped so one bad reply does
 * not poison the inbox.
 */
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { FEATURE_ID_PATTERN } from "@/features/mission/index.js";
import { ensureDir, readText, removeIfExists, writeText } from "@/shared/lib/fs.js";
import { parseYaml, stringifyYaml } from "@/shared/lib/yaml.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import type { ReplyStorePort } from "../ports/reply-store.port.js";
import type { WorkerReply } from "../domain/reply-types.js";
import { validateWorkerReply } from "../domain/reply-validators.js";

const REPLIES_DIR = "replies";

export class FsReplyStoreAdapter implements ReplyStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, REPLIES_DIR);
  }

  private replyPath(featureId: string): string {
    assertSafeSegment(featureId, "feature ID", FEATURE_ID_PATTERN, "letters, numbers, dashes, and underscores");
    return resolveWithin(this.dir(), `${featureId}.yaml`, "Reply path");
  }

  private ingestedMarkerPath(featureId: string): string {
    assertSafeSegment(featureId, "feature ID", FEATURE_ID_PATTERN, "letters, numbers, dashes, and underscores");
    return resolveWithin(this.dir(), `${featureId}.ingested`, "Reply ingested marker");
  }

  async get(featureId: string): Promise<WorkerReply | undefined> {
    const raw = await readText(this.replyPath(featureId));
    if (raw === undefined) return undefined;
    return parseReplyText(raw);
  }

  async list(): Promise<readonly WorkerReply[]> {
    const ids = await listReplyIds(this.dir());
    const replies: WorkerReply[] = [];
    for (const id of ids) {
      const reply = await this.get(id);
      if (reply) replies.push(reply);
    }
    return replies.sort((a, b) => a.writtenAt.localeCompare(b.writtenAt));
  }

  async listSince(isoTimestamp: string): Promise<readonly WorkerReply[]> {
    const all = await this.list();
    return all.filter((r) => r.writtenAt >= isoTimestamp);
  }

  async write(reply: WorkerReply): Promise<void> {
    const validated = validateWorkerReply(reply);
    await ensureDir(this.dir());
    await writeText(this.replyPath(validated.featureId), stringifyYaml(validated));
  }

  async isIngested(featureId: string): Promise<boolean> {
    const text = await readText(this.ingestedMarkerPath(featureId));
    return text !== undefined;
  }

  async markIngested(featureId: string): Promise<void> {
    await ensureDir(this.dir());
    await writeText(this.ingestedMarkerPath(featureId), new Date().toISOString() + "\n");
  }
}

async function listReplyIds(dir: string): Promise<string[]> {
  try {
    const entries = await readdir(dir);
    return entries
      .filter((entry) => entry.endsWith(".yaml"))
      .map((entry) => entry.replace(/\.yaml$/, ""))
      .filter((id) => FEATURE_ID_PATTERN.test(id));
  } catch {
    return [];
  }
}

function parseReplyText(raw: string): WorkerReply | undefined {
  try {
    const parsed = parseYaml<unknown>(raw);
    return validateWorkerReply(parsed);
  } catch {
    return undefined;
  }
}

/** Exposed for testing only. Removes the sidecar marker. */
export async function clearIngestedMarker(baseDir: string, featureId: string): Promise<void> {
  assertSafeSegment(featureId, "feature ID", FEATURE_ID_PATTERN, "letters, numbers, dashes, and underscores");
  const markerPath = resolveWithin(
    join(baseDir, MAESTRO_DIR, REPLIES_DIR),
    `${featureId}.ingested`,
    "Reply ingested marker",
  );
  await removeIfExists(markerPath);
}
