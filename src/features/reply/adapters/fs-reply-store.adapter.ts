/**
 * Filesystem-backed reply store.
 *
 * Layout:
 *   .maestro/replies/<mission-id>/<feature-id>.yaml     -- the reply itself
 *   .maestro/replies/<mission-id>/<feature-id>.ingested -- sidecar marker
 *
 * Writes are atomic (write-then-rename). Reads are tolerant: malformed YAML
 * or missing required fields are logged and skipped so one bad reply does
 * not poison the inbox.
 */
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { FEATURE_ID_PATTERN, MISSION_ID_PATTERN } from "@/features/mission";
import { ensureDir, readText, removeIfExists, writeText } from "@/shared/lib/fs.js";
import { parseYaml, stringifyYaml } from "@/shared/lib/yaml.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import type { ReplyStorePort } from "../ports/reply-store.port.js";
import type { AgentReply } from "../domain/types.js";
import { validateAgentReply } from "../domain/validators.js";

const REPLIES_DIR = "replies";

export class FsReplyStoreAdapter implements ReplyStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, REPLIES_DIR);
  }

  private missionDir(missionId: string): string {
    assertSafeSegment(missionId, "mission ID", MISSION_ID_PATTERN, "YYYY-MM-DD-NNN");
    return resolveWithin(this.dir(), missionId, "Replies mission directory");
  }

  private replyPath(missionId: string, featureId: string): string {
    assertSafeSegment(featureId, "feature ID", FEATURE_ID_PATTERN, "letters, numbers, dashes, and underscores");
    return resolveWithin(this.missionDir(missionId), `${featureId}.yaml`, "Reply path");
  }

  private ingestedMarkerPath(missionId: string, featureId: string): string {
    assertSafeSegment(featureId, "feature ID", FEATURE_ID_PATTERN, "letters, numbers, dashes, and underscores");
    return resolveWithin(this.missionDir(missionId), `${featureId}.ingested`, "Reply ingested marker");
  }

  async get(missionId: string, featureId: string): Promise<AgentReply | undefined> {
    const raw = await readText(this.replyPath(missionId, featureId));
    if (raw === undefined) return undefined;
    return parseReplyText(raw, missionId, featureId);
  }

  async list(): Promise<readonly AgentReply[]> {
    const refs = await listReplyRefs(this.dir());
    // Each reply is its own file; reads are I/O-independent.
    const settled = await Promise.all(
      refs.map((ref) => this.get(ref.missionId, ref.featureId)),
    );
    const replies = settled.filter((r): r is AgentReply => r !== undefined);
    return replies.sort((a, b) => a.writtenAt.localeCompare(b.writtenAt));
  }

  async listSince(isoTimestamp: string): Promise<readonly AgentReply[]> {
    const all = await this.list();
    return all.filter((r) => r.writtenAt >= isoTimestamp);
  }

  async write(reply: AgentReply): Promise<void> {
    const validated = validateAgentReply(reply);
    await ensureDir(this.missionDir(validated.missionId));
    await writeText(this.replyPath(validated.missionId, validated.featureId), stringifyYaml(validated));
    // Overwriting a reply invalidates any prior ingestion marker so the
    // next snapshot poll re-runs ingest against the fresh content.
    await removeIfExists(this.ingestedMarkerPath(validated.missionId, validated.featureId));
  }

  async isIngested(missionId: string, featureId: string): Promise<boolean> {
    const text = await readText(this.ingestedMarkerPath(missionId, featureId));
    return text !== undefined;
  }

  async markIngested(missionId: string, featureId: string): Promise<void> {
    await ensureDir(this.missionDir(missionId));
    await writeText(this.ingestedMarkerPath(missionId, featureId), new Date().toISOString() + "\n");
  }
}

interface ReplyRef {
  readonly missionId: string;
  readonly featureId: string;
}

async function listReplyRefs(dir: string): Promise<readonly ReplyRef[]> {
  try {
    const missionEntries = await readdir(dir, { withFileTypes: true });
    const refs: ReplyRef[] = [];
    for (const missionEntry of missionEntries) {
      if (!missionEntry.isDirectory() || !MISSION_ID_PATTERN.test(missionEntry.name)) {
        continue;
      }
      const missionId = missionEntry.name;
      const missionDir = join(dir, missionId);
      const replyEntries = await readdir(missionDir);
      for (const entry of replyEntries) {
        if (!entry.endsWith(".yaml")) {
          continue;
        }
        const featureId = entry.replace(/\.yaml$/, "");
        if (!FEATURE_ID_PATTERN.test(featureId)) {
          continue;
        }
        refs.push({ missionId, featureId });
      }
    }
    return refs;
  } catch {
    return [];
  }
}

function parseReplyText(
  raw: string,
  expectedMissionId: string,
  expectedFeatureId: string,
): AgentReply | undefined {
  try {
    const parsed = parseYaml<unknown>(raw);
    const reply = validateAgentReply(parsed);
    if (
      reply.missionId !== expectedMissionId
      || reply.featureId !== expectedFeatureId
    ) {
      return undefined;
    }
    return reply;
  } catch {
    return undefined;
  }
}
