/**
 * Aggregate every artifact that belongs to a mission into an in-memory
 * set of bundle files plus summary stats. Pure enough that the export
 * usecase can delegate writing to the archive adapter and the test suite
 * can assert the shape of the collection in isolation.
 */
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import type {
  Assertion,
  AssertionStorePort,
  CheckpointStorePort,
  Feature,
  FeatureStorePort,
  Mission,
  MissionStorePort,
} from "@/features/mission/index.js";
import { MISSION_ID_PATTERN } from "@/features/mission/index.js";
import {
  isHandoffInProject,
  type HandoffStorePort,
} from "@/features/handoff/index.js";
import type { ReplyStorePort } from "@/features/reply/index.js";
import { MaestroError } from "@/shared/errors.js";
import { readText, dirExists } from "@/shared/lib/fs.js";
import { MAESTRO_DIR, MEMORY_DIR } from "@/shared/domain/defaults.js";
import { assertSafeSegment } from "@/shared/lib/path-safety.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import type {
  BundleFile,
  BundleMemoryStats,
  BundleOptions,
  BundleRedactScope,
  BundleStats,
} from "../domain/bundle-types.js";

export interface CollectBundleSourcesInput {
  readonly missionId: string;
  readonly projectDir: string;
  readonly options: BundleOptions;
}

export interface CollectBundleSourcesDeps {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly checkpointStore: CheckpointStorePort;
  readonly replyStore: ReplyStorePort;
  readonly handoffStore: HandoffStorePort;
}

export interface BundleSources {
  readonly mission: Mission;
  readonly features: readonly Feature[];
  readonly assertions: readonly Assertion[];
  readonly files: readonly BundleFile[];
  readonly stats: BundleStats;
}

const BUNDLE_ROOT = (missionId: string): string => `${missionId}.mission`;

export async function collectBundleSources(
  deps: CollectBundleSourcesDeps,
  input: CollectBundleSourcesInput,
): Promise<BundleSources> {
  const { missionId, projectDir, options } = input;
  assertSafeSegment(
    missionId,
    "mission ID",
    MISSION_ID_PATTERN,
    "YYYY-MM-DD-NNN",
  );

  const redact = new Set<BundleRedactScope>(options.redact);
  const root = BUNDLE_ROOT(missionId);
  const files: BundleFile[] = [];
  const projectRoot = resolveMaestroProjectRoot(projectDir);

  const mission = await deps.missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      "Check that the mission ID is correct",
    ]);
  }

  // mission.json
  files.push({
    path: `${root}/mission/mission.json`,
    content: stringifyJson(mission),
  });

  // features/*.json
  const features = await deps.featureStore.list(missionId);
  for (const feature of features) {
    files.push({
      path: `${root}/mission/features/${feature.id}.json`,
      content: stringifyJson(feature),
    });
  }

  // assertions.json (aggregate list)
  const assertions = await deps.assertionStore.list(missionId);
  files.push({
    path: `${root}/mission/assertions.json`,
    content: stringifyJson(assertions),
  });

  // agents/{featureId}/*
  const missionDir = join(projectDir, MAESTRO_DIR, "missions", missionId);
  const agentsDir = join(missionDir, "agents");
  const agentFeatureIds = await listSubdirectories(agentsDir);
  const redactPrompts = redact.has("prompts");
  for (const featureId of agentFeatureIds) {
    const featureDir = join(agentsDir, featureId);
    const entries = await safeReaddir(featureDir);
    for (const entry of entries) {
      if (redactPrompts && entry.endsWith(".md")) continue;
      const text = await readText(join(featureDir, entry));
      if (text === undefined) continue;
      files.push({
        path: `${root}/mission/agents/${featureId}/${entry}`,
        content: text,
      });
    }
  }

  // checkpoints/*.json (via store to tolerate malformed files)
  const checkpoints = await deps.checkpointStore.list(missionId);
  for (const checkpoint of checkpoints) {
    files.push({
      path: `${root}/mission/checkpoints/${checkpoint.id}.json`,
      content: stringifyJson(checkpoint),
    });
  }

  // replies
  let replyCount = 0;
  if (!redact.has("replies")) {
    for (const feature of features) {
      const reply = await deps.replyStore.get(missionId, feature.id);
      if (!reply) continue;
      const yaml = await readText(
        join(projectDir, MAESTRO_DIR, "replies", missionId, `${feature.id}.yaml`),
      );
      if (yaml === undefined) continue;
      replyCount++;
      files.push({
        path: `${root}/replies/${feature.id}.yaml`,
        content: yaml,
      });
    }
  }

  // handoff packets that reference this mission id
  const allHandoffs = await deps.handoffStore.list();
  const missionHandoffs = allHandoffs.filter(
    (handoff) => handoff.refs.missionId === missionId && isHandoffInProject(handoff, projectRoot),
  );
  const missionHandoffIds = new Set(missionHandoffs.map((handoff) => handoff.id));
  for (const handoff of missionHandoffs) {
    files.push({
      path: `${root}/handoffs/${handoff.id}.json`,
      content: stringifyJson(handoff),
    });
  }

  // principles snapshot -- global files filtered where possible
  const principlesPath = join(projectDir, MAESTRO_DIR, "principles.jsonl");
  const principlesRaw = await readText(principlesPath);
  const principlesContent = principlesRaw ?? "";
  files.push({
    path: `${root}/principles/principles.jsonl`,
    content: principlesContent,
  });
  const principlesSnapshot = countJsonlLines(principlesContent);

  const outcomesPath = join(projectDir, MAESTRO_DIR, "principles", "outcomes.jsonl");
  const outcomesRaw = await readText(outcomesPath);
  const filteredOutcomes = filterOutcomesForHandoffs(
    outcomesRaw ?? "",
    missionHandoffIds,
  );
  files.push({
    path: `${root}/principles/outcomes.jsonl`,
    content: filteredOutcomes,
  });
  const outcomesSnapshot = countJsonlLines(filteredOutcomes);

  // memory snapshot (corrections + learnings)
  let memorySnapshot: BundleMemoryStats | null = null;
  if (!redact.has("memory")) {
    const memoryStats = await collectMemoryFiles(projectDir, files, root);
    memorySnapshot = memoryStats;
  }

  const stats: BundleStats = {
    features: features.length,
    milestones: mission.milestones.length,
    assertions: assertions.length,
    agents: agentFeatureIds.length,
    replies: replyCount,
    handoffs: missionHandoffs.length,
    checkpoints: checkpoints.length,
    principlesSnapshot,
    outcomesSnapshot,
    memorySnapshot,
  };

  return {
    mission,
    features,
    assertions,
    files,
    stats,
  };
}

async function collectMemoryFiles(
  projectDir: string,
  files: BundleFile[],
  root: string,
): Promise<BundleMemoryStats> {
  const memoryDir = join(projectDir, MAESTRO_DIR, MEMORY_DIR);
  const correctionsDir = join(memoryDir, "corrections");
  const learningsCompiled = join(memoryDir, "learnings", "_compiled.json");

  let corrections = 0;
  for (const entry of await safeReaddir(correctionsDir)) {
    if (!entry.endsWith(".json")) continue;
    const text = await readText(join(correctionsDir, entry));
    if (text === undefined) continue;
    corrections++;
    files.push({
      path: `${root}/memory/corrections/${entry}`,
      content: text,
    });
  }

  let learnings = 0;
  const compiledText = await readText(learningsCompiled);
  if (compiledText !== undefined) {
    files.push({
      path: `${root}/memory/learnings/_compiled.json`,
      content: compiledText,
    });
    learnings = countLearnings(compiledText);
  }

  return { corrections, learnings };
}

async function listSubdirectories(dir: string): Promise<string[]> {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries
      .filter((e) => e.isDirectory())
      .map((e) => e.name)
      .sort();
  } catch {
    return [];
  }
}

async function safeReaddir(dir: string): Promise<string[]> {
  if (!(await dirExists(dir))) return [];
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries.filter((e) => e.isFile()).map((e) => e.name).sort();
  } catch {
    return [];
  }
}

function stringifyJson(value: unknown): string {
  return JSON.stringify(value, null, 2) + "\n";
}

function countJsonlLines(content: string): number {
  if (!content) return 0;
  return content.split("\n").filter((line) => line.trim().length > 0).length;
}

function countLearnings(compiledText: string): number {
  try {
    const parsed = JSON.parse(compiledText) as unknown;
    if (Array.isArray(parsed)) return parsed.length;
    if (
      parsed
      && typeof parsed === "object"
      && Array.isArray((parsed as { learnings?: unknown[] }).learnings)
    ) {
      return (parsed as { learnings: unknown[] }).learnings.length;
    }
    return 0;
  } catch {
    return 0;
  }
}

function filterOutcomesForHandoffs(
  raw: string,
  handoffIds: ReadonlySet<string>,
): string {
  if (!raw) return "";
  const kept: string[] = [];
  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      const parsed = JSON.parse(trimmed) as { handoffId?: string };
      if (parsed.handoffId && handoffIds.has(parsed.handoffId)) {
        kept.push(trimmed);
      }
    } catch {
      // skip malformed lines
    }
  }
  return kept.length > 0 ? kept.join("\n") + "\n" : "";
}
